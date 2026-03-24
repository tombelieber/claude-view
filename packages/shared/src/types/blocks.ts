// packages/shared/src/types/blocks.ts
// ConversationBlock view models — hand-written by design (no Rust equivalent).
// Combine data from multiple ServerEvent types into semantic blocks for rendering.

import type { ActionCategory } from './generated/ActionCategory'
import type { ProgressData } from './generated/ProgressData'
import type { ProgressVariant } from './generated/ProgressVariant'

import type {
  AskQuestion,
  AssistantError,
  AuthStatus,
  CommandOutput,
  ContextCompacted,
  Elicitation,
  ElicitationComplete,
  ErrorEvent,
  FilesSaved,
  HookEvent,
  ModelUsageInfo,
  AiTitle,
  FileHistorySnapshot,
  Informational,
  LastPrompt,
  LocalCommand,
  PermissionRequest,
  PlanApproval,
  PromptSuggestion,
  QueueOperation,
  RateLimit,
  SessionClosed,
  SessionInit,
  SessionStatus,
  StreamDelta,
  TaskNotification,
  TaskProgressEvent,
  TaskStarted,
  UnknownSdkEvent,
} from './sidecar-protocol'

export type { ActionCategory } from './generated/ActionCategory'

// ── UserBlock ───────────────────────────────────────────────────────────────

export type UserBlock = {
  type: 'user'
  id: string
  text: string
  timestamp: number
  status?: 'optimistic' | 'sending' | 'sent' | 'failed'
  localId?: string
  parentUuid?: string | null
  rawJson?: Record<string, unknown> | null
}

// ── AssistantBlock ──────────────────────────────────────────────────────────

export type AssistantBlock = {
  type: 'assistant'
  id: string // messageId from first event in this block
  segments: AssistantSegment[]
  thinking?: string
  streaming: boolean
  /** Unix seconds — populated from JSONL timestamp or Date.now() for live blocks */
  timestamp?: number
  parentUuid?: string | null
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
  category?: ActionCategory
  liveOutput?: string
  duration?: number
}

// ── InteractionBlock ────────────────────────────────────────────────────────

export type InteractionBlock = {
  type: 'interaction'
  id: string
  variant: 'permission' | 'question' | 'plan' | 'elicitation'
  requestId: string
  resolved: boolean
  data: PermissionRequest | AskQuestion | PlanApproval | Elicitation
}

// ── TurnBoundaryBlock ───────────────────────────────────────────────────────

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

// ── NoticeBlock ─────────────────────────────────────────────────────────────

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

// ── SystemBlock ─────────────────────────────────────────────────────────────

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
    | 'local_command'
    | 'queue_operation'
    | 'file_history_snapshot'
    | 'ai_title'
    | 'last_prompt'
    | 'informational'
    | 'unknown'
  data:
    | SessionInit
    | SessionStatus
    | ElicitationComplete
    | HookEvent
    | TaskStarted
    | TaskProgressEvent
    | TaskNotification
    | FilesSaved
    | CommandOutput
    | StreamDelta
    | LocalCommand
    | QueueOperation
    | FileHistorySnapshot
    | AiTitle
    | LastPrompt
    | Informational
    | UnknownSdkEvent
  rawJson?: Record<string, unknown> | null
}

// ── ProgressBlock ──────────────────────────────────────────────────────────

export type ProgressBlock = {
  type: 'progress'
  id: string
  variant: ProgressVariant
  category: ActionCategory
  data: ProgressData
  ts: number
  parentToolUseId?: string
}

// ── ConversationBlock union ─────────────────────────────────────────────────

export type ConversationBlock =
  | UserBlock
  | AssistantBlock
  | InteractionBlock
  | TurnBoundaryBlock
  | NoticeBlock
  | SystemBlock
  | ProgressBlock

// ── Type guards ────────────────────────────────────────────────────────────

export function isProgressBlock(block: ConversationBlock): block is ProgressBlock {
  return block.type === 'progress'
}
