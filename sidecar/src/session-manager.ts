// sidecar/src/session-manager.ts
//
// REWRITTEN (audit H1+H2+H3): Uses correct Agent SDK V2 API.
// - unstable_v2_resumeSession(sessionId, options) — positional sessionId
// - SDKSession has .send(message) and .stream() (async generator of SDKMessage)
// - Permission routing via canUseTool callback in options
// - close() is synchronous in V2 (no await)

import { EventEmitter } from 'node:events'
import {
  type PermissionResult,
  type SDKMessage,
  type SDKSession,
  unstable_v2_resumeSession,
} from '@anthropic-ai/claude-agent-sdk'
import { RingBuffer } from './ring-buffer.js'
import type {
  ActiveSession,
  AskUserQuestionMessage,
  SequencedServerMessage,
  ServerMessage,
  ThinkingMessage,
} from './types.js'

export interface ControlSession {
  controlId: string
  sessionId: string
  sdkSession: SDKSession
  status: 'active' | 'waiting_input' | 'waiting_permission' | 'completed' | 'error'
  totalCost: number | null
  turnCount: number
  contextUsage: number // 0-100 percentage of context window used
  totalInputTokens: number
  totalOutputTokens: number
  cacheReadTokens: number
  cacheCreationTokens: number
  lastTurnInputTokens: number
  permissionMode: import('@anthropic-ai/claude-agent-sdk').PermissionMode | null
  model: string | null
  sessionContextWindow: number | null
  startedAt: number
  emitter: EventEmitter
  isStreaming: boolean // guard against concurrent sendMessage calls
  eventBuffer: RingBuffer<{ seq: number; msg: SequencedServerMessage }>
  nextSeq: number
  pendingPermissions: Map<
    string,
    {
      resolve: (allowed: boolean) => void
      timer: ReturnType<typeof setTimeout>
    }
  >
  pendingQuestions: Map<string, { resolve: (answers: Record<string, string>) => void }>
  pendingPlans: Map<string, { resolve: (result: { approved: boolean; feedback?: string }) => void }>
  pendingElicitations: Map<string, { resolve: (response: string) => void }>
}

export class SessionManager {
  private sessions = new Map<string, ControlSession>()

  getActiveCount(): number {
    return this.sessions.size
  }

  getSession(controlId: string): ControlSession | undefined {
    return this.sessions.get(controlId)
  }

  listSessions(): ActiveSession[] {
    return Array.from(this.sessions.values()).map((cs) => ({
      controlId: cs.controlId,
      sessionId: cs.sessionId,
      status: cs.status,
      turnCount: cs.turnCount,
      totalCost: cs.totalCost,
      startedAt: cs.startedAt,
    }))
  }

  async resume(
    sessionId: string,
    model?: string,
    projectPath?: string,
    permissionMode?: import('@anthropic-ai/claude-agent-sdk').PermissionMode,
  ): Promise<ControlSession> {
    // Check if already active
    for (const cs of this.sessions.values()) {
      if (cs.sessionId === sessionId) {
        return cs
      }
    }

    const controlId = crypto.randomUUID()
    const emitter = new EventEmitter()

    // SDK v0.2.63: canUseTool available — permissions, AskUserQuestion, ExitPlanMode
    // routed through handleCanUseTool → pendingPermissions/Questions/Plans maps

    // canUseTool closure captures `cs` by reference — safe because
    // canUseTool fires asynchronously, never during synchronous init.
    // biome-ignore lint/style/useConst: definite assignment with deferred init — `let x!: T` then assign after sdkSession
    let cs!: ControlSession

    const sdkSession = unstable_v2_resumeSession(sessionId, {
      model: model ?? 'claude-sonnet-4-20250514',
      ...(projectPath ? { cwd: projectPath } : {}),
      ...(permissionMode ? { permissionMode } : {}),
      canUseTool: async (toolName, input, { signal }) => {
        return this.handleCanUseTool(cs, toolName, input, signal)
      },
    })

    cs = {
      controlId,
      sessionId,
      sdkSession,
      status: 'waiting_input',
      totalCost: null,
      turnCount: 0,
      contextUsage: 0,
      totalInputTokens: 0,
      totalOutputTokens: 0,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
      lastTurnInputTokens: 0,
      permissionMode: permissionMode ?? null,
      model: null,
      sessionContextWindow: null,
      startedAt: Date.now(),
      emitter,
      isStreaming: false,
      eventBuffer: new RingBuffer(200),
      nextSeq: 0,
      pendingPermissions: new Map(),
      pendingQuestions: new Map(),
      pendingPlans: new Map(),
      pendingElicitations: new Map(),
    }

    this.sessions.set(controlId, cs)
    return cs
  }

  /** Emit a message with seq number and buffer it for replay (internal). */
  private emitSequenced(cs: ControlSession, msg: ServerMessage): void {
    const seq = cs.nextSeq++
    const sequenced: SequencedServerMessage = { ...msg, seq }
    cs.eventBuffer.push({ seq, msg: sequenced })
    cs.emitter.emit('message', sequenced)
  }

  /** Public wrapper — ws-handler.ts calls this for the initial session_status. */
  emitSequencedById(controlId: string, msg: ServerMessage): void {
    const cs = this.sessions.get(controlId)
    if (!cs) return
    this.emitSequenced(cs, msg)
  }

  /**
   * Central callback for all tool permission decisions.
   * Routes to specific handlers based on toolName.
   */
  private async handleCanUseTool(
    cs: ControlSession,
    toolName: string,
    input: Record<string, unknown>,
    signal: AbortSignal,
  ): Promise<PermissionResult> {
    // --- AskUserQuestion: route to frontend for user selection ---
    if (toolName === 'AskUserQuestion') {
      const requestId = crypto.randomUUID()
      const questions = input.questions as AskUserQuestionMessage['questions']

      return new Promise((resolve) => {
        cs.pendingQuestions.set(requestId, {
          resolve: (answers: Record<string, string>) => {
            resolve({
              behavior: 'allow',
              updatedInput: { ...input, answers },
            })
          },
        })

        signal.addEventListener(
          'abort',
          () => {
            if (cs.pendingQuestions.has(requestId)) {
              cs.pendingQuestions.delete(requestId)
              resolve({ behavior: 'deny', message: 'Question aborted' })
            }
          },
          { once: true },
        )

        // Update status and emit to frontend
        cs.status = 'waiting_permission'
        this.emitSequenced(cs, {
          type: 'ask_user_question',
          requestId,
          questions,
        })
        this.emitSequenced(cs, {
          type: 'session_status',
          status: cs.status,
          contextUsage: cs.contextUsage,
          turnCount: cs.turnCount,
        })
      })
    }

    // --- ExitPlanMode: route to frontend for plan approval ---
    if (toolName === 'ExitPlanMode') {
      const requestId = crypto.randomUUID()

      return new Promise((resolve) => {
        cs.pendingPlans.set(requestId, {
          resolve: (result: { approved: boolean; feedback?: string }) => {
            resolve(
              result.approved
                ? { behavior: 'allow', updatedInput: input }
                : { behavior: 'deny', message: result.feedback ?? 'Plan rejected by user' },
            )
          },
        })

        signal.addEventListener(
          'abort',
          () => {
            if (cs.pendingPlans.has(requestId)) {
              cs.pendingPlans.delete(requestId)
              resolve({ behavior: 'deny', message: 'Plan approval aborted' })
            }
          },
          { once: true },
        )

        // Update status and emit plan data to frontend
        cs.status = 'waiting_permission'
        this.emitSequenced(cs, {
          type: 'plan_approval',
          requestId,
          planData: input,
        })
        this.emitSequenced(cs, {
          type: 'session_status',
          status: cs.status,
          contextUsage: cs.contextUsage,
          turnCount: cs.turnCount,
        })
      })
    }

    // --- Generic permission request (all other tools) ---
    const requestId = crypto.randomUUID()

    return new Promise((resolve) => {
      // Store resolve callback
      cs.pendingPermissions.set(requestId, {
        resolve: (allowed: boolean) => {
          resolve(
            allowed
              ? { behavior: 'allow', updatedInput: input }
              : { behavior: 'deny', message: `User denied ${toolName}` },
          )
        },
        timer: setTimeout(() => {
          // Auto-deny after 60s if frontend doesn't respond
          if (cs.pendingPermissions.has(requestId)) {
            cs.pendingPermissions.delete(requestId)
            resolve({ behavior: 'deny', message: 'Permission request timed out' })
          }
        }, 60_000),
      })

      // Handle SDK abort (session close, server shutdown)
      signal.addEventListener(
        'abort',
        () => {
          const pending = cs.pendingPermissions.get(requestId)
          if (pending) {
            clearTimeout(pending.timer)
            cs.pendingPermissions.delete(requestId)
            resolve({ behavior: 'deny', message: 'Request aborted' })
          }
        },
        { once: true },
      )

      // Emit to frontend via EventEmitter → WebSocket
      cs.status = 'waiting_permission'
      this.emitSequenced(cs, {
        type: 'permission_request',
        requestId,
        toolName,
        toolInput: input,
        description: `${toolName} requires permission`,
        timeoutMs: 60_000,
      })
      this.emitSequenced(cs, {
        type: 'session_status',
        status: cs.status,
        contextUsage: cs.contextUsage,
        turnCount: cs.turnCount,
      })
    })
  }

  async sendMessage(controlId: string, content: string): Promise<void> {
    const cs = this.sessions.get(controlId)
    if (!cs) throw new Error(`No session: ${controlId}`)
    if (cs.isStreaming) throw new Error('Session is already streaming')

    cs.isStreaming = true
    cs.status = 'active'
    let messageId = crypto.randomUUID()

    await cs.sdkSession.send(content)

    // Process stream in background
    ;(async () => {
      try {
        for await (const msg of cs.sdkSession.stream()) {
          switch (msg.type) {
            case 'stream_event': {
              // V2 SDK does NOT emit stream_event — kept for forward-compat
              // if includePartialMessages is added to SDKSessionOptions.
              const event = (msg as SDKMessage & { type: 'stream_event' }).event
              if (
                event?.type === 'content_block_delta' &&
                'delta' in event &&
                (event as Record<string, unknown>).delta &&
                (event as Record<string, Record<string, unknown>>).delta.type === 'text_delta'
              ) {
                this.emitSequenced(cs, {
                  type: 'assistant_chunk',
                  content: (event as Record<string, Record<string, unknown>>).delta.text as string,
                  messageId,
                })
              }
              break
            }
            case 'assistant': {
              // V2 SDK delivers complete assistant messages (not streaming chunks).
              // Each message contains content blocks: text, tool_use, thinking.
              // Emit text blocks as assistant_chunk so the client can render them.
              messageId = crypto.randomUUID()
              const assistantMsg = msg as SDKMessage & { type: 'assistant' }
              for (const block of assistantMsg.message.content) {
                if (block.type === 'thinking' && 'thinking' in block && block.thinking) {
                  this.emitSequenced(cs, {
                    type: 'thinking',
                    content: block.thinking as string,
                    messageId,
                  } satisfies ThinkingMessage)
                } else if (block.type === 'text' && block.text) {
                  this.emitSequenced(cs, {
                    type: 'assistant_chunk',
                    content: block.text,
                    messageId,
                  })
                } else if (block.type === 'tool_use') {
                  this.emitSequenced(cs, {
                    type: 'tool_use_start',
                    toolName: block.name,
                    toolInput: block.input as Record<string, unknown>,
                    toolUseId: block.id,
                  })
                }
              }
              // Accumulate per-turn tokens from BetaMessage.usage (BetaUsage uses snake_case)
              // Note: BetaUsage comes from @anthropic-ai/sdk (API types, snake_case),
              // NOT ModelUsage from @anthropic-ai/claude-agent-sdk (SDK types, camelCase).
              const usage = assistantMsg.message.usage
              if (usage) {
                cs.lastTurnInputTokens = usage.input_tokens ?? 0
                cs.totalInputTokens += usage.input_tokens ?? 0
                cs.totalOutputTokens += usage.output_tokens ?? 0
                cs.cacheReadTokens += usage.cache_read_input_tokens ?? 0
                cs.cacheCreationTokens += usage.cache_creation_input_tokens ?? 0
              }
              break
            }
            case 'user': {
              const userMsg = msg as SDKMessage & { type: 'user' }
              if (userMsg.message?.content && Array.isArray(userMsg.message.content)) {
                for (const block of userMsg.message.content) {
                  if (block.type === 'tool_result') {
                    this.emitSequenced(cs, {
                      type: 'tool_use_result',
                      toolUseId: block.tool_use_id ?? '',
                      output:
                        typeof block.content === 'string'
                          ? block.content
                          : JSON.stringify(block.content ?? ''),
                      isError: block.is_error ?? false,
                    })
                  }
                }
              }
              break
            }
            case 'result': {
              const resultMsg = msg as SDKMessage & { type: 'result' }
              if (resultMsg.subtype === 'success') {
                cs.totalCost = resultMsg.total_cost_usd ?? null
                cs.turnCount = resultMsg.num_turns ?? 0

                // Extract context window from modelUsage (SDK type: Record<string, ModelUsage>)
                if (resultMsg.modelUsage) {
                  for (const [model, mu] of Object.entries(resultMsg.modelUsage)) {
                    if (mu.contextWindow) {
                      cs.sessionContextWindow = mu.contextWindow
                      cs.model = model
                    }
                  }
                }

                // Override accumulated totals with authoritative result usage
                if (resultMsg.usage) {
                  cs.totalInputTokens = resultMsg.usage.input_tokens ?? cs.totalInputTokens
                  cs.totalOutputTokens = resultMsg.usage.output_tokens ?? cs.totalOutputTokens
                  cs.cacheReadTokens = resultMsg.usage.cache_read_input_tokens ?? cs.cacheReadTokens
                  cs.cacheCreationTokens =
                    resultMsg.usage.cache_creation_input_tokens ?? cs.cacheCreationTokens
                }
              }
              cs.status = 'waiting_input'

              const ctxWindow = cs.sessionContextWindow ?? 200_000
              cs.contextUsage = Math.round((cs.lastTurnInputTokens / ctxWindow) * 100)

              this.emitSequenced(cs, {
                type: 'assistant_done',
                messageId,
                usage: {
                  inputTokens: cs.totalInputTokens,
                  outputTokens: cs.totalOutputTokens,
                  cacheReadTokens: cs.cacheReadTokens,
                  cacheWriteTokens: cs.cacheCreationTokens,
                },
                cost: null,
                totalCost: cs.totalCost,
              })

              this.emitSequenced(cs, {
                type: 'session_status',
                status: cs.status,
                contextUsage: cs.contextUsage,
                turnCount: cs.turnCount,
                tokenUsage: {
                  input: cs.totalInputTokens,
                  output: cs.totalOutputTokens,
                  cacheRead: cs.cacheReadTokens,
                  cacheCreation: cs.cacheCreationTokens,
                },
                costUsd: cs.totalCost ?? undefined,
                model: cs.model ?? undefined,
                contextWindow: cs.sessionContextWindow ?? undefined,
              })
              break
            }
          }
        }
        cs.isStreaming = false
      } catch (err) {
        cs.isStreaming = false
        cs.status = 'error'
        this.emitSequenced(cs, {
          type: 'error',
          message: err instanceof Error ? err.message : String(err),
          fatal: true,
        })
      }
    })()
  }

  resolvePermission(controlId: string, requestId: string, allowed: boolean): boolean {
    const cs = this.sessions.get(controlId)
    if (!cs) return false
    const pending = cs.pendingPermissions.get(requestId)
    if (!pending) return false
    clearTimeout(pending.timer)
    cs.pendingPermissions.delete(requestId)
    cs.status = 'active'
    this.emitSequenced(cs, {
      type: 'session_status',
      status: cs.status,
      contextUsage: cs.contextUsage,
      turnCount: cs.turnCount,
    })
    pending.resolve(allowed)
    return true
  }

  resolveQuestion(controlId: string, requestId: string, answers: Record<string, string>): boolean {
    const cs = this.sessions.get(controlId)
    if (!cs) return false
    const pending = cs.pendingQuestions.get(requestId)
    if (!pending) return false
    cs.pendingQuestions.delete(requestId)
    cs.status = 'active'
    this.emitSequenced(cs, {
      type: 'session_status',
      status: cs.status,
      contextUsage: cs.contextUsage,
      turnCount: cs.turnCount,
    })
    pending.resolve(answers)
    return true
  }

  resolvePlan(controlId: string, requestId: string, approved: boolean, feedback?: string): boolean {
    const cs = this.sessions.get(controlId)
    if (!cs) return false
    const pending = cs.pendingPlans.get(requestId)
    if (!pending) return false
    cs.pendingPlans.delete(requestId)
    cs.status = 'active'
    this.emitSequenced(cs, {
      type: 'session_status',
      status: cs.status,
      contextUsage: cs.contextUsage,
      turnCount: cs.turnCount,
    })
    pending.resolve({ approved, feedback })
    return true
  }

  resolveElicitation(controlId: string, requestId: string, response: string): boolean {
    const cs = this.sessions.get(controlId)
    if (!cs) return false
    const pending = cs.pendingElicitations.get(requestId)
    if (!pending) return false
    cs.pendingElicitations.delete(requestId)
    cs.status = 'active'
    this.emitSequenced(cs, {
      type: 'session_status',
      status: cs.status,
      contextUsage: cs.contextUsage,
      turnCount: cs.turnCount,
    })
    pending.resolve(response)
    return true
  }

  async setMode(
    controlId: string,
    mode: import('@anthropic-ai/claude-agent-sdk').PermissionMode,
  ): Promise<boolean> {
    const cs = this.sessions.get(controlId)
    if (!cs) return false

    // CRITICAL: Only allow mode changes when NOT actively streaming.
    if (cs.isStreaming) {
      this.emitSequenced(cs, {
        type: 'error',
        message:
          'Cannot change mode while agent is processing. Wait for the current turn to complete.',
        fatal: false,
      })
      return false
    }

    try {
      // V2 SDK does NOT support mid-session setPermissionMode().
      // Strategy: close the current SDK session and re-resume with the new mode.
      cs.sdkSession.close()
      // unstable_v2_resumeSession is already imported at module top level
      cs.sdkSession = unstable_v2_resumeSession(cs.sessionId, {
        model: cs.model ?? 'claude-sonnet-4-20250514',
        permissionMode: mode,
        canUseTool: async (toolName, input, { signal }) => {
          return this.handleCanUseTool(cs, toolName, input, signal)
        },
      })
      cs.permissionMode = mode
      return true
    } catch (err) {
      this.emitSequenced(cs, {
        type: 'error',
        message: `Failed to set mode: ${err instanceof Error ? err.message : String(err)}`,
        fatal: false,
      })
      return false
    }
  }

  async close(controlId: string): Promise<void> {
    const cs = this.sessions.get(controlId)
    if (!cs) return

    // Drain all pending promises before closing (prevents leaked promises)
    for (const [, pending] of cs.pendingPermissions) {
      clearTimeout(pending.timer)
      pending.resolve(false) // deny
    }
    cs.pendingPermissions.clear()

    for (const [, pending] of cs.pendingQuestions) {
      pending.resolve({}) // allow with empty answers
    }
    cs.pendingQuestions.clear()

    for (const [, pending] of cs.pendingPlans) {
      pending.resolve({ approved: false }) // reject plan
    }
    cs.pendingPlans.clear()

    for (const [, pending] of cs.pendingElicitations) {
      pending.resolve('') // empty response
    }
    cs.pendingElicitations.clear()

    cs.sdkSession.close() // close() is synchronous in V2, no await
    this.sessions.delete(controlId)
  }

  hasSessionId(sessionId: string): boolean {
    return Array.from(this.sessions.values()).some((cs) => cs.sessionId === sessionId)
  }

  getBySessionId(sessionId: string): ControlSession | undefined {
    return Array.from(this.sessions.values()).find((cs) => cs.sessionId === sessionId)
  }

  async shutdownAll(): Promise<void> {
    await Promise.all(Array.from(this.sessions.keys()).map((id) => this.close(id)))
  }
}
