import type {
  AssistantBlock,
  ConversationBlock,
  InteractionBlock,
  NoticeBlock,
  SystemBlock,
  ToolExecution,
  TurnBoundaryBlock,
} from '../types/blocks'
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
} from '../types/sidecar-protocol'

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

  private handleEvent(event: SequencedEvent): void {
    switch (event.type) {
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
      case 'stream_delta':
        this.pushSystem('stream_delta', event as StreamDelta)
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

  private ensureAssistant(messageId: string): AssistantBlock {
    if (!this.currentAssistant) {
      this.currentAssistant = {
        type: 'assistant',
        id: messageId,
        segments: [],
        streaming: true,
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
    // Search in committed blocks first, then current assistant
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
