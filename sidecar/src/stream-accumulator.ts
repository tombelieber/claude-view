// sidecar/src/stream-accumulator.ts
// Server-side accumulator: converts sequenced protocol events into ConversationBlock state.
// Copied from packages/shared/src/lib/stream-accumulator.ts with local type imports.

import type {
  AskQuestion,
  AssistantError,
  AssistantText,
  AssistantThinking,
  AuthStatus,
  CommandOutput,
  ContextCompacted,
  Elicitation,
  ElicitationComplete,
  ErrorEvent,
  FilesSaved,
  HookEvent,
  ModelUsageInfo,
  PermissionRequest,
  PlanApproval,
  PromptSuggestion,
  RateLimit,
  ServerEvent,
  SessionClosed,
  SessionInit,
  StreamDelta,
  TaskNotification,
  TaskProgressEvent,
  TaskStarted,
  ToolProgress,
  ToolSummary,
  ToolUseResult,
  ToolUseStart,
  TurnComplete,
  TurnError,
  UnknownSdkEvent,
  UserMessageEcho,
} from './protocol.js'

// ── Progress types — mirrored from packages/shared/src/types/generated/ ─
// These MUST match the frontend's ProgressData discriminated union.
// If Rust types change (ts-rs regenerates), update these to match.
// Without typed data, the frontend casts blocks blindly → runtime crashes.
type ProgressVariant = 'bash' | 'agent' | 'mcp' | 'hook' | 'task_queue' | 'search' | 'query'
type ActionCategory = 'builtin' | 'mcp' | 'agent' | 'hook'
type BashProgressData = {
  type: 'bash'
  output: string
  fullOutput: string
  elapsedTimeSeconds: number
  totalLines: number
  totalBytes: number
  taskId?: string | null
}
type AgentProgressData = {
  type: 'agent'
  prompt: string
  agentId?: string
  message?: string
}
type McpProgressData = { type: 'mcp'; serverName: string; toolName: string }
type HookProgressData = {
  type: 'hook'
  hookName: string
  hookType: string
  statusMessage?: string
}
type TaskQueueProgressData = { type: 'task_queue'; queueSize: number }
type SearchProgressData = { type: 'search'; query: string }
type QueryProgressData = { type: 'query'; query: string }
type ProgressData =
  | BashProgressData
  | AgentProgressData
  | McpProgressData
  | HookProgressData
  | TaskQueueProgressData
  | SearchProgressData
  | QueryProgressData

// ── Block types (inlined for sidecar isolation) ─────────────────────────

export type UserBlock = {
  type: 'user'
  id: string
  text: string
  timestamp: number
  status?: 'optimistic' | 'sending' | 'sent' | 'failed'
  localId?: string
  rawJson?: Record<string, unknown> | null
}

export type AssistantBlock = {
  type: 'assistant'
  id: string
  segments: AssistantSegment[]
  thinking?: string
  streaming: boolean
  timestamp?: number
  rawJson?: Record<string, unknown> | null
}

export type AssistantSegment =
  | { kind: 'text'; text: string; parentToolUseId?: string | null }
  | { kind: 'tool'; execution: ToolExecution }

export type ToolExecution = {
  toolName: string
  toolInput: Record<string, unknown>
  toolUseId: string
  parentToolUseId?: string | null
  result?: { output: string; isError: boolean; isReplay: boolean }
  progress?: { elapsedSeconds: number }
  summary?: string
  status: 'running' | 'complete' | 'error'
}

export type InteractionBlock = {
  type: 'interaction'
  id: string
  variant: 'permission' | 'question' | 'plan' | 'elicitation'
  requestId: string
  resolved: boolean
  data: PermissionRequest | AskQuestion | PlanApproval | Elicitation
}

export type TurnBoundaryBlock = {
  type: 'turn_boundary'
  id: string
  success: boolean
  totalCostUsd: number
  numTurns: number
  durationMs: number
  durationApiMs?: number
  usage: Record<string, number>
  modelUsage: Record<string, ModelUsageInfo>
  permissionDenials: { toolName: string; toolUseId: string; toolInput: Record<string, unknown> }[]
  result?: string
  structuredOutput?: unknown
  stopReason: string | null
  fastModeState?: 'off' | 'cooldown' | 'on'
  error?: {
    subtype:
      | 'error_during_execution'
      | 'error_max_turns'
      | 'error_max_budget_usd'
      | 'error_max_structured_output_retries'
    messages: string[]
  }
}

export type NoticeBlock = {
  type: 'notice'
  id: string
  variant:
    | 'assistant_error'
    | 'rate_limit'
    | 'context_compacted'
    | 'auth_status'
    | 'session_closed'
    | 'error'
    | 'prompt_suggestion'
    | 'session_resumed'
  data:
    | AssistantError
    | RateLimit
    | ContextCompacted
    | AuthStatus
    | SessionClosed
    | ErrorEvent
    | PromptSuggestion
    | null
}

export type SystemBlock = {
  type: 'system'
  id: string
  variant:
    | 'session_init'
    | 'session_status'
    | 'elicitation_complete'
    | 'hook_event'
    | 'task_started'
    | 'task_progress'
    | 'task_notification'
    | 'files_saved'
    | 'command_output'
    | 'stream_delta'
    | 'unknown'
  data:
    | SessionInit
    | { type: 'session_status'; status: 'compacting' | null; permissionMode?: string }
    | ElicitationComplete
    | HookEvent
    | TaskStarted
    | TaskProgressEvent
    | TaskNotification
    | FilesSaved
    | CommandOutput
    | StreamDelta
    | UnknownSdkEvent
  rawJson?: Record<string, unknown> | null
}

export type ProgressBlock = {
  type: 'progress'
  id: string
  variant: ProgressVariant
  category: ActionCategory
  data: ProgressData
  ts: number
  parentToolUseId?: string
}

export type ConversationBlock =
  | UserBlock
  | AssistantBlock
  | InteractionBlock
  | TurnBoundaryBlock
  | NoticeBlock
  | SystemBlock
  | ProgressBlock

// ── StreamAccumulator ───────────────────────────────────────────────────

let _idCounter = 0
function genId(): string {
  return `block-${++_idCounter}`
}

export class StreamAccumulator {
  private blocks: ConversationBlock[] = []
  private currentAssistant: AssistantBlock | null = null
  private pushCounter = 0
  private initialized = false
  private buffer: { event: ServerEvent; raw?: Record<string, unknown> }[] = []
  /** Raw SDK message for the current push — available during handleEvent. */
  private currentRaw: Record<string, unknown> | undefined = undefined

  push(event: ServerEvent, rawSdkMessage?: Record<string, unknown>): void {
    this.pushCounter++

    // user_message_echo bypasses init gate — render immediately
    if (event.type === 'user_message_echo') {
      this.currentRaw = rawSdkMessage
      this.handleEvent(event)
      this.currentRaw = undefined
      return
    }

    // Buffer events before session_init
    if (!this.initialized && event.type !== 'session_init') {
      this.buffer.push({ event, raw: rawSdkMessage })
      return
    }

    this.currentRaw = rawSdkMessage
    this.handleEvent(event)
    this.currentRaw = undefined
  }

  getBlocks(): ConversationBlock[] {
    return [...this.blocks, ...(this.currentAssistant ? [this.currentAssistant] : [])]
  }

  finalize(): ConversationBlock[] {
    if (this.currentAssistant) {
      this.currentAssistant.streaming = false
      this.blocks.push(this.currentAssistant)
      this.currentAssistant = null
    }
    return this.getBlocks()
  }

  /** Reset all accumulated blocks for a new turn cycle.
   *  Preserves lastProcessedSeq for reconnect dedup safety. */
  reset(): void {
    this.blocks = []
    this.currentAssistant = null
    this.initialized = false
    this.buffer = []
  }

  private handleEvent(event: ServerEvent): void {
    switch (event.type) {
      case 'user_message_echo':
        this.handleUserMessageEcho(event as UserMessageEcho)
        break
      case 'session_init':
        this.handleSessionInit(event as SessionInit)
        break
      case 'assistant_text':
        this.handleAssistantText(event as AssistantText)
        break
      case 'assistant_thinking':
        this.handleAssistantThinking(event as AssistantThinking)
        break
      case 'assistant_error':
        this.handleAssistantError(event as AssistantError)
        break
      case 'tool_use_start':
        this.handleToolUseStart(event as ToolUseStart)
        break
      case 'tool_use_result':
        this.handleToolUseResult(event as ToolUseResult)
        break
      case 'tool_progress':
        this.handleToolProgress(event as ToolProgress)
        break
      case 'tool_summary':
        this.handleToolSummary(event as ToolSummary)
        break
      case 'turn_complete':
        this.handleTurnComplete(event as TurnComplete)
        break
      case 'turn_error':
        this.handleTurnError(event as TurnError)
        break
      case 'permission_request':
        this.pushInteraction(
          'permission',
          (event as PermissionRequest).requestId,
          event as PermissionRequest,
        )
        break
      case 'ask_question':
        this.pushInteraction('question', (event as AskQuestion).requestId, event as AskQuestion)
        break
      case 'plan_approval':
        this.pushInteraction('plan', (event as PlanApproval).requestId, event as PlanApproval)
        break
      case 'elicitation':
        this.pushInteraction('elicitation', (event as Elicitation).requestId, event as Elicitation)
        break
      case 'rate_limit':
        this.pushNotice('rate_limit', event as RateLimit)
        break
      case 'context_compacted':
        this.pushNotice('context_compacted', event as ContextCompacted)
        break
      case 'auth_status':
        this.pushNotice('auth_status', event as AuthStatus)
        break
      case 'session_closed':
        this.pushNotice('session_closed', event as SessionClosed)
        break
      case 'error':
        this.pushNotice('error', event as ErrorEvent)
        break
      case 'prompt_suggestion':
        this.pushNotice('prompt_suggestion', event as PromptSuggestion)
        break
      case 'elicitation_complete':
        this.pushSystem('elicitation_complete', event as ElicitationComplete)
        break
      case 'hook_event':
        this.pushSystem('hook_event', event as HookEvent)
        break
      case 'task_started':
        this.pushSystem('task_started', event as TaskStarted)
        break
      case 'task_progress':
        this.pushSystem('task_progress', event as TaskProgressEvent)
        // Also emit an agent ProgressBlock for Developer mode progress cards
        this.pushProgress(
          'agent',
          'agent',
          {
            type: 'agent',
            prompt: (event as TaskProgressEvent).description,
            agentId: (event as TaskProgressEvent).taskId,
            message: (event as TaskProgressEvent).summary ?? undefined,
          },
          (event as TaskProgressEvent).toolUseId,
        )
        break
      case 'task_notification':
        this.pushSystem('task_notification', event as TaskNotification)
        break
      case 'files_saved':
        this.pushSystem('files_saved', event as FilesSaved)
        break
      case 'command_output':
        this.pushSystem('command_output', event as CommandOutput)
        break
      case 'stream_delta':
        // Only structural events (content_block_start/stop, message_stop) reach here.
        // content_block_delta is filtered in emitSequenced to prevent doubled text.
        this.handleStreamDelta(event as StreamDelta)
        break
      case 'unknown_sdk_event':
        this.pushSystem('unknown', event as UnknownSdkEvent)
        break
      case 'session_status':
        this.pushSystem(
          'session_status',
          event as { type: 'session_status'; status: 'compacting' | null; permissionMode?: string },
        )
        break
      case 'pong':
        // Infrastructure event — ignored, no block created
        break
      default:
        // Unknown events silently ignored for forward compatibility
        break
    }
  }

  private handleUserMessageEcho(event: UserMessageEcho): void {
    this.finalizeCurrentAssistant()
    const block: UserBlock = {
      type: 'user',
      id: `user-${this.pushCounter}`,
      text: event.content,
      timestamp: event.timestamp,
      rawJson: this.extractRawJson(),
    }
    this.blocks.push(block)
  }

  private handleSessionInit(event: SessionInit): void {
    this.initialized = true
    this.pushSystem('session_init', event)
    // Flush buffered events (with their raw SDK messages)
    const buffered = this.buffer.splice(0)
    for (const { event: e, raw } of buffered) {
      this.currentRaw = raw
      this.handleEvent(e)
      this.currentRaw = undefined
    }
  }

  private handleAssistantText(event: AssistantText): void {
    const assistant = this.ensureAssistant(event.messageId)
    const lastSeg = assistant.segments.at(-1)
    if (lastSeg?.kind === 'text' && lastSeg.parentToolUseId === event.parentToolUseId) {
      lastSeg.text += event.text
    } else {
      assistant.segments.push({
        kind: 'text',
        text: event.text,
        parentToolUseId: event.parentToolUseId,
      })
    }
  }

  private handleAssistantThinking(event: AssistantThinking): void {
    const assistant = this.ensureAssistant(event.messageId)
    assistant.thinking = (assistant.thinking ?? '') + event.thinking
  }

  private handleAssistantError(event: AssistantError): void {
    this.finalizeCurrentAssistant()
    this.pushNotice('assistant_error', event)
  }

  private handleToolUseStart(event: ToolUseStart): void {
    const assistant = this.ensureAssistant(event.messageId)
    const execution: ToolExecution = {
      toolName: event.toolName,
      toolInput: event.toolInput,
      toolUseId: event.toolUseId,
      parentToolUseId: event.parentToolUseId,
      status: 'running',
    }
    assistant.segments.push({ kind: 'tool', execution })
  }

  private handleToolUseResult(event: ToolUseResult): void {
    const execution = this.findToolExecution(event.toolUseId)
    if (execution) {
      execution.result = { output: event.output, isError: event.isError, isReplay: event.isReplay }
      execution.status = event.isError ? 'error' : 'complete'
    }
  }

  private handleToolProgress(event: ToolProgress): void {
    const execution = this.findToolExecution(event.toolUseId)
    if (execution) {
      execution.progress = { elapsedSeconds: event.elapsedSeconds }
    }
    // Emit a ProgressBlock so Developer mode shows live progress cards
    this.pushProgress(
      'bash',
      'builtin',
      {
        type: 'bash',
        output: '',
        fullOutput: '',
        elapsedTimeSeconds: event.elapsedSeconds ?? 0,
        totalLines: 0,
        totalBytes: 0,
        taskId: event.taskId ?? null,
      },
      event.parentToolUseId ?? undefined,
    )
  }

  private handleToolSummary(event: ToolSummary): void {
    for (const toolUseId of event.precedingToolUseIds) {
      const execution = this.findToolExecution(toolUseId)
      if (execution) {
        execution.summary = event.summary
      }
    }
  }

  private handleTurnComplete(event: TurnComplete): void {
    this.finalizeCurrentAssistant()
    const boundary: TurnBoundaryBlock = {
      type: 'turn_boundary',
      id: genId(),
      success: true,
      totalCostUsd: event.totalCostUsd,
      numTurns: event.numTurns,
      durationMs: event.durationMs,
      durationApiMs: event.durationApiMs,
      usage: event.usage,
      modelUsage: event.modelUsage,
      permissionDenials: event.permissionDenials,
      result: event.result,
      structuredOutput: event.structuredOutput,
      stopReason: event.stopReason,
      fastModeState: event.fastModeState,
    }
    this.blocks.push(boundary)
  }

  private handleTurnError(event: TurnError): void {
    this.finalizeCurrentAssistant()
    const boundary: TurnBoundaryBlock = {
      type: 'turn_boundary',
      id: genId(),
      success: false,
      totalCostUsd: event.totalCostUsd,
      numTurns: event.numTurns,
      durationMs: event.durationMs,
      usage: event.usage,
      modelUsage: event.modelUsage,
      permissionDenials: event.permissionDenials,
      stopReason: event.stopReason,
      fastModeState: event.fastModeState,
      error: {
        subtype: event.subtype,
        messages: event.errors,
      },
    }
    this.blocks.push(boundary)
  }

  /** Handle structural stream events (content_block_start/stop, message_stop).
   *  content_block_delta is filtered in emitSequenced — never reaches here. */
  private handleStreamDelta(event: StreamDelta): void {
    switch (event.deltaType) {
      case 'content_block_start': {
        const assistant = this.ensureAssistant(event.messageId)
        assistant.segments.push({ kind: 'text', text: '', parentToolUseId: null })
        break
      }
      case 'content_block_delta': {
        // Dead code — filtered in emitSequenced. Kept for safety/forward compat.
        if (event.textDelta) {
          const assistant = this.ensureAssistant(event.messageId)
          const lastSeg = assistant.segments.at(-1)
          if (lastSeg?.kind === 'text') {
            lastSeg.text += event.textDelta
          }
        } else if (event.thinkingDelta) {
          const assistant = this.ensureAssistant(event.messageId)
          assistant.thinking = (assistant.thinking ?? '') + event.thinkingDelta
        }
        break
      }
      case 'message_stop': {
        this.finalizeCurrentAssistant()
        break
      }
      default: {
        this.pushSystem('stream_delta', event)
        break
      }
    }
  }

  private ensureAssistant(messageId: string): AssistantBlock {
    if (!this.currentAssistant) {
      this.currentAssistant = {
        type: 'assistant',
        id: messageId,
        segments: [],
        streaming: true,
        timestamp: Date.now() / 1000,
        rawJson: this.extractRawJson(),
      }
    }
    return this.currentAssistant
  }

  private finalizeCurrentAssistant(): void {
    if (this.currentAssistant) {
      this.currentAssistant.streaming = false
      this.blocks.push(this.currentAssistant)
      this.currentAssistant = null
    }
  }

  private findToolExecution(toolUseId: string): ToolExecution | undefined {
    const allBlocks = this.getBlocks()
    for (const block of allBlocks) {
      if (block.type !== 'assistant') continue
      for (const seg of block.segments) {
        if (seg.kind === 'tool' && seg.execution.toolUseId === toolUseId) {
          return seg.execution
        }
      }
    }
    return undefined
  }

  private pushInteraction(
    variant: InteractionBlock['variant'],
    requestId: string,
    data: InteractionBlock['data'],
  ): void {
    const block: InteractionBlock = {
      type: 'interaction',
      id: genId(),
      variant,
      requestId,
      resolved: false,
      data,
    }
    this.blocks.push(block)
  }

  private pushNotice(variant: NoticeBlock['variant'], data: NoticeBlock['data']): void {
    const block: NoticeBlock = {
      type: 'notice',
      id: genId(),
      variant,
      data,
    }
    this.blocks.push(block)
  }

  private pushSystem(variant: SystemBlock['variant'], data: SystemBlock['data']): void {
    const block: SystemBlock = {
      type: 'system',
      id: genId(),
      variant,
      data,
      rawJson: this.extractRawJson(),
    }
    this.blocks.push(block)
  }

  private pushProgress(
    variant: ProgressVariant,
    category: ActionCategory,
    data: ProgressData,
    parentToolUseId?: string,
  ): void {
    const block: ProgressBlock = {
      type: 'progress',
      id: genId(),
      variant,
      category,
      data,
      ts: Date.now() / 1000,
      parentToolUseId,
    }
    this.blocks.push(block)
  }

  /** Extract raw SDK message envelope, omitting the large `message.content`
   *  payload that is already parsed into structured block fields. */
  private extractRawJson(): Record<string, unknown> | undefined {
    if (!this.currentRaw) return undefined
    const { message, ...envelope } = this.currentRaw
    // Keep envelope metadata, drop parsed content to avoid duplication
    return Object.keys(envelope).length > 0 ? envelope : undefined
  }
}
