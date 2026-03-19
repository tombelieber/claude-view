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
  SequencedEvent,
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

export type ConversationBlock =
  | UserBlock
  | AssistantBlock
  | InteractionBlock
  | TurnBoundaryBlock
  | NoticeBlock
  | SystemBlock

// ── StreamAccumulator ───────────────────────────────────────────────────

let _idCounter = 0
function genId(): string {
  return `block-${++_idCounter}`
}

export class StreamAccumulator {
  private blocks: ConversationBlock[] = []
  private currentAssistant: AssistantBlock | null = null
  private lastProcessedSeq = -1
  private initialized = false
  private buffer: SequencedEvent[] = []

  push(event: SequencedEvent): void {
    // Dedup: drop events already processed (reconnect replay)
    if (event.seq <= this.lastProcessedSeq) return

    // user_message_echo bypasses init gate — render immediately
    if (event.type === 'user_message_echo') {
      this.lastProcessedSeq = event.seq
      this.handleEvent(event)
      return
    }

    // Buffer events before session_init
    if (!this.initialized && event.type !== 'session_init') {
      this.buffer.push(event)
      return
    }

    this.lastProcessedSeq = event.seq
    this.handleEvent(event)
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

  private handleEvent(event: SequencedEvent): void {
    switch (event.type) {
      case 'user_message_echo':
        this.handleUserMessageEcho(event as UserMessageEcho & { seq: number })
        break
      case 'session_init':
        this.handleSessionInit(event as SessionInit & { seq: number })
        break
      case 'assistant_text':
        this.handleAssistantText(event as AssistantText & { seq: number })
        break
      case 'assistant_thinking':
        this.handleAssistantThinking(event as AssistantThinking & { seq: number })
        break
      case 'assistant_error':
        this.handleAssistantError(event as AssistantError & { seq: number })
        break
      case 'tool_use_start':
        this.handleToolUseStart(event as ToolUseStart & { seq: number })
        break
      case 'tool_use_result':
        this.handleToolUseResult(event as ToolUseResult & { seq: number })
        break
      case 'tool_progress':
        this.handleToolProgress(event as ToolProgress & { seq: number })
        break
      case 'tool_summary':
        this.handleToolSummary(event as ToolSummary & { seq: number })
        break
      case 'turn_complete':
        this.handleTurnComplete(event as TurnComplete & { seq: number })
        break
      case 'turn_error':
        this.handleTurnError(event as TurnError & { seq: number })
        break
      case 'permission_request':
        this.pushInteraction(
          'permission',
          (event as PermissionRequest & { seq: number }).requestId,
          event as PermissionRequest,
        )
        break
      case 'ask_question':
        this.pushInteraction(
          'question',
          (event as AskQuestion & { seq: number }).requestId,
          event as AskQuestion,
        )
        break
      case 'plan_approval':
        this.pushInteraction(
          'plan',
          (event as PlanApproval & { seq: number }).requestId,
          event as PlanApproval,
        )
        break
      case 'elicitation':
        this.pushInteraction(
          'elicitation',
          (event as Elicitation & { seq: number }).requestId,
          event as Elicitation,
        )
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
      // stream_delta is filtered out in emitSequenced — never reaches accumulator.
      // Frontend gets stream_delta via WS for live rendering; blocks only need final text
      // from assistant_text. Removing this handler prevents doubled text.
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

  private handleUserMessageEcho(event: UserMessageEcho & { seq: number }): void {
    this.finalizeCurrentAssistant()
    const block: UserBlock = {
      type: 'user',
      id: `user-${event.seq}`,
      text: event.content,
      timestamp: event.timestamp,
    }
    this.blocks.push(block)
  }

  private handleSessionInit(event: SessionInit & { seq: number }): void {
    this.initialized = true
    this.pushSystem('session_init', event)
    // Flush buffered events
    const buffered = this.buffer.splice(0)
    for (const e of buffered) {
      this.lastProcessedSeq = e.seq
      this.handleEvent(e)
    }
  }

  private handleAssistantText(event: AssistantText & { seq: number }): void {
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

  private handleAssistantThinking(event: AssistantThinking & { seq: number }): void {
    const assistant = this.ensureAssistant(event.messageId)
    assistant.thinking = (assistant.thinking ?? '') + event.thinking
  }

  private handleAssistantError(event: AssistantError & { seq: number }): void {
    this.finalizeCurrentAssistant()
    this.pushNotice('assistant_error', event)
  }

  private handleToolUseStart(event: ToolUseStart & { seq: number }): void {
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

  private handleToolUseResult(event: ToolUseResult & { seq: number }): void {
    const execution = this.findToolExecution(event.toolUseId)
    if (execution) {
      execution.result = { output: event.output, isError: event.isError, isReplay: event.isReplay }
      execution.status = event.isError ? 'error' : 'complete'
    }
  }

  private handleToolProgress(event: ToolProgress & { seq: number }): void {
    const execution = this.findToolExecution(event.toolUseId)
    if (execution) {
      execution.progress = { elapsedSeconds: event.elapsedSeconds }
    }
  }

  private handleToolSummary(event: ToolSummary & { seq: number }): void {
    for (const toolUseId of event.precedingToolUseIds) {
      const execution = this.findToolExecution(toolUseId)
      if (execution) {
        execution.summary = event.summary
      }
    }
  }

  private handleTurnComplete(event: TurnComplete & { seq: number }): void {
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

  private handleTurnError(event: TurnError & { seq: number }): void {
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

  // handleStreamDelta removed — stream_delta events are filtered out in
  // SessionRegistry.emitSequenced() and never reach the accumulator.
  // See doubled-text-regression.test.ts for the root cause explanation.

  private ensureAssistant(messageId: string): AssistantBlock {
    if (!this.currentAssistant) {
      this.currentAssistant = {
        type: 'assistant',
        id: messageId,
        segments: [],
        streaming: true,
        timestamp: Date.now() / 1000,
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
    }
    this.blocks.push(block)
  }
}
