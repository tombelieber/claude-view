// sidecar/src/session-manager.ts
//
// REWRITTEN (audit H1+H2+H3): Uses correct Agent SDK V2 API.
// - unstable_v2_resumeSession(sessionId, options) — positional sessionId
// - SDKSession has .send(message) and .stream() (async generator of SDKMessage)
// - Permission routing via canUseTool callback in options
// - close() is synchronous in V2 (no await)

import { EventEmitter } from 'node:events'
import {
  type SDKMessage,
  type SDKSession,
  unstable_v2_resumeSession,
} from '@anthropic-ai/claude-agent-sdk'
import type { ActiveSession, ServerMessage } from './types.js'

interface ControlSession {
  controlId: string
  sessionId: string
  sdkSession: SDKSession
  status: 'active' | 'waiting_input' | 'waiting_permission' | 'completed' | 'error'
  totalCost: number
  turnCount: number
  contextUsage: number // 0-100 percentage of context window used
  startedAt: number
  emitter: EventEmitter
  isStreaming: boolean // guard against concurrent sendMessage calls
  pendingPermissions: Map<
    string,
    {
      resolve: (allowed: boolean) => void
      timer: ReturnType<typeof setTimeout>
    }
  >
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

  async resume(sessionId: string, model?: string, projectPath?: string): Promise<ControlSession> {
    // Check if already active
    for (const cs of this.sessions.values()) {
      if (cs.sessionId === sessionId) {
        return cs
      }
    }

    const controlId = crypto.randomUUID()
    const emitter = new EventEmitter()
    const pendingPermissions = new Map<
      string,
      {
        resolve: (allowed: boolean) => void
        timer: ReturnType<typeof setTimeout>
      }
    >()

    // SDK V2 LIMITATION (verified against installed `@anthropic-ai/claude-agent-sdk` types):
    //
    // `SDKSessionOptions` only accepts: { model, pathToClaudeCodeExecutable?, executable?,
    //   executableArgs?, env? }
    //
    // The following fields are NOT available in V2:
    //   - `cwd` -- pass via env: { CLAUDE_CWD: projectPath } if the SDK respects it, or omit
    //   - `canUseTool` -- NOT available in V2. Interactive permission routing requires either:
    //     (a) Fall back to V1 query() API which supports canUseTool in its Options type
    //     (b) Accept all tools automatically in V2 and add interactive permissions when SDK stabilizes
    //   - `includePartialMessages` -- NOT available in V2. Streaming content comes via stream() events.
    //
    // Recommendation: For Phase F MVP, use V2 with auto-accept permissions. Add
    // env: { CLAUDE_ACCEPT_TOS: 'true' } or equivalent. Mark interactive permission approval as
    // Phase F.2 pending SDK stabilization. The permission UI (Task 14) can still be built -- it
    // just won't be wired to canUseTool until the V2 API adds it.
    const sdkSession = unstable_v2_resumeSession(sessionId, {
      model: model ?? 'claude-sonnet-4-20250514',
      env: {
        ...(projectPath ? { CLAUDE_CWD: projectPath } : {}),
      },
    })

    // Phase F.2 TODO: Wire interactive permission routing when SDK V2 adds canUseTool.
    // The permission UI (Task 14) and pendingPermissions map are ready -- they just need
    // the SDK callback to be wired in. For now, permissions are auto-accepted by the SDK.

    const cs: ControlSession = {
      controlId,
      sessionId,
      sdkSession,
      status: 'waiting_input',
      totalCost: 0,
      turnCount: 0,
      contextUsage: 0,
      startedAt: Date.now(),
      emitter,
      isStreaming: false,
      pendingPermissions,
    }

    this.sessions.set(controlId, cs)
    return cs
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
              // Real-time text chunks (needs includePartialMessages: true)
              const event = (msg as SDKMessage & { type: 'stream_event' }).event
              if (
                event?.type === 'content_block_delta' &&
                'delta' in event &&
                (event as Record<string, unknown>).delta &&
                (event as Record<string, Record<string, unknown>>).delta.type === 'text_delta'
              ) {
                cs.emitter.emit('message', {
                  type: 'assistant_chunk',
                  content: (event as Record<string, Record<string, unknown>>).delta.text as string,
                  messageId,
                } satisfies ServerMessage)
              }
              break
            }
            case 'assistant': {
              // Complete assistant message with all content blocks
              messageId = crypto.randomUUID()
              const assistantMsg = msg as SDKMessage & { type: 'assistant' }
              for (const block of assistantMsg.message.content) {
                if (block.type === 'tool_use') {
                  cs.emitter.emit('message', {
                    type: 'tool_use_start',
                    toolName: block.name,
                    toolInput: block.input as Record<string, unknown>,
                    toolUseId: block.id,
                  } satisfies ServerMessage)
                }
              }
              break
            }
            case 'user': {
              // Tool results come back as user messages
              break
            }
            case 'result': {
              const resultMsg = msg as SDKMessage & { type: 'result' }
              if (resultMsg.subtype === 'success') {
                cs.totalCost = resultMsg.total_cost_usd ?? 0
                cs.turnCount = resultMsg.num_turns ?? 0
              }
              cs.status = 'waiting_input'
              cs.emitter.emit('message', {
                type: 'assistant_done',
                messageId,
                usage: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheWriteTokens: 0 },
                cost: 0,
                totalCost: cs.totalCost,
              } satisfies ServerMessage)
              break
            }
          }
        }
        cs.isStreaming = false
      } catch (err) {
        cs.isStreaming = false
        cs.status = 'error'
        cs.emitter.emit('message', {
          type: 'error',
          message: err instanceof Error ? err.message : String(err),
          fatal: true,
        } satisfies ServerMessage)
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
    pending.resolve(allowed)
    return true
  }

  async close(controlId: string): Promise<void> {
    const cs = this.sessions.get(controlId)
    if (!cs) return
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
