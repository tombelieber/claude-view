// apps/web/src/types/control.ts
// Frontend mirror of sidecar/src/protocol.ts — server events + client messages

export interface CostEstimate {
  session_id: string
  history_tokens: number
  cache_warm: boolean
  first_message_cost: number | null
  per_message_cost: number | null
  has_pricing: boolean
  model: string
  explanation: string
  session_title: string | null
  project_name: string | null
  turn_count: number
  files_edited: number
  last_active_secs_ago: number
}

// --- Server -> Client Events (sidecar -> frontend) ----------------

export interface AssistantTextMsg {
  type: 'assistant_text'
  text: string
  messageId: string
  parentToolUseId: string | null
}

export interface AssistantThinkingMsg {
  type: 'assistant_thinking'
  thinking: string
  messageId: string
  parentToolUseId: string | null
}

export interface AssistantErrorMsg {
  type: 'assistant_error'
  error: string
  messageId: string
}

export interface StreamDeltaMsg {
  type: 'stream_delta'
  event: unknown
  messageId: string
}

export interface ToolUseStartMsg {
  type: 'tool_use_start'
  toolName: string
  toolInput: Record<string, unknown>
  toolUseId: string
  messageId: string
  parentToolUseId: string | null
}

export interface ToolUseResultMsg {
  type: 'tool_use_result'
  toolUseId: string
  output: string
  isError: boolean
  isReplay: boolean
}

export interface ToolProgressMsg {
  type: 'tool_progress'
  toolUseId: string
  toolName: string
  elapsedSeconds: number
  parentToolUseId: string | null
  taskId?: string
}

export interface ToolSummaryMsg {
  type: 'tool_summary'
  summary: string
  precedingToolUseIds: string[]
}

export interface ModelUsageInfo {
  inputTokens: number
  outputTokens: number
  cacheReadInputTokens: number
  cacheCreationInputTokens: number
  webSearchRequests: number
  costUSD: number
  contextWindow: number
  maxOutputTokens: number
}

export interface TurnCompleteMsg {
  type: 'turn_complete'
  totalCostUsd: number
  numTurns: number
  durationMs: number
  durationApiMs: number
  usage: Record<string, number>
  modelUsage: Record<string, ModelUsageInfo>
  permissionDenials: { toolName: string; toolUseId: string; toolInput: Record<string, unknown> }[]
  result: string
  structuredOutput?: unknown
  stopReason: string | null
  fastModeState?: 'off' | 'cooldown' | 'on'
}

export interface TurnErrorMsg {
  type: 'turn_error'
  subtype: string
  errors: string[]
  permissionDenials: { toolName: string; toolUseId: string; toolInput: Record<string, unknown> }[]
  totalCostUsd: number
  numTurns: number
  durationMs: number
  usage: Record<string, number>
  modelUsage: Record<string, ModelUsageInfo>
  stopReason: string | null
  fastModeState?: 'off' | 'cooldown' | 'on'
}

export interface SessionInitMsg {
  type: 'session_init'
  tools: string[]
  model: string
  mcpServers: { name: string; status: string }[]
  permissionMode: string
  slashCommands: string[]
  claudeCodeVersion: string
  cwd: string
  agents: string[]
  skills: string[]
  outputStyle: string
}

export interface SessionStatusMsg {
  type: 'session_status'
  status: 'compacting' | null
  permissionMode?: string
}

export interface SessionClosedMsg {
  type: 'session_closed'
  reason: string
}

export interface ContextCompactedMsg {
  type: 'context_compacted'
  trigger: 'manual' | 'auto'
  preTokens: number
}

export interface RateLimitMsg {
  type: 'rate_limit'
  status: 'allowed' | 'allowed_warning' | 'rejected'
  resetsAt?: number
  utilization?: number
  rateLimitType?: string
  overageStatus?: string
  isUsingOverage?: boolean
}

export interface TaskStartedMsg {
  type: 'task_started'
  taskId: string
  toolUseId?: string
  description: string
  taskType?: string
  prompt?: string
}

export interface TaskProgressMsg {
  type: 'task_progress'
  taskId: string
  toolUseId?: string
  description: string
  lastToolName?: string
  summary?: string
  usage: { totalTokens: number; toolUses: number; durationMs: number }
}

export interface TaskNotificationMsg {
  type: 'task_notification'
  taskId: string
  toolUseId?: string
  status: 'completed' | 'failed' | 'stopped'
  outputFile: string
  summary: string
  usage?: { totalTokens: number; toolUses: number; durationMs: number }
}

export interface HookEventMsg {
  type: 'hook_event'
  phase: 'started' | 'progress' | 'response'
  hookId: string
  hookName: string
  hookEventName: string
  stdout?: string
  stderr?: string
  output?: string
  exitCode?: number
  outcome?: 'success' | 'error' | 'cancelled'
}

export interface AuthStatusMsg {
  type: 'auth_status'
  isAuthenticating: boolean
  output: string[]
  error?: string
}

export interface FilesSavedMsg {
  type: 'files_saved'
  files: { filename: string; fileId: string }[]
  failed: { filename: string; error: string }[]
  processedAt: string
}

export interface CommandOutputMsg {
  type: 'command_output'
  content: string
}

export interface PromptSuggestionMsg {
  type: 'prompt_suggestion'
  suggestion: string
}

export interface UnknownSdkEventMsg {
  type: 'unknown_sdk_event'
  sdkType: string
  raw: unknown
}

// --- Interactive Cards --------------------------------------------

export interface PermissionRequestMsg {
  type: 'permission_request'
  requestId: string
  toolName: string
  toolInput: Record<string, unknown>
  toolUseID: string
  suggestions?: unknown[]
  decisionReason?: string
  blockedPath?: string
  agentID?: string
  timeoutMs: number
}

export interface AskUserQuestionMsg {
  type: 'ask_question'
  requestId: string
  questions: {
    question: string
    header: string
    options: { label: string; description: string; markdown?: string }[]
    multiSelect: boolean
  }[]
}

export interface PlanApprovalMsg {
  type: 'plan_approval'
  requestId: string
  planData: Record<string, unknown>
}

export interface ElicitationMsg {
  type: 'elicitation'
  requestId: string
  toolName: string
  toolInput: Record<string, unknown>
  prompt: string
}

export interface ElicitationCompleteMsg {
  type: 'elicitation_complete'
  mcpServerName: string
  elicitationId: string
}

// --- Infrastructure -----------------------------------------------

export interface ErrorMsg {
  type: 'error'
  message: string
  fatal: boolean
}

export interface PongMsg {
  type: 'pong'
}

// Heartbeat message - NO seq field.
export interface HeartbeatConfigMsg {
  type: 'heartbeat_config'
  intervalMs: number
}

// --- Union Type ---------------------------------------------------

export type ServerMessage =
  // Assistant output
  | AssistantTextMsg
  | AssistantThinkingMsg
  | AssistantErrorMsg
  | StreamDeltaMsg
  // Tool execution
  | ToolUseStartMsg
  | ToolUseResultMsg
  | ToolProgressMsg
  | ToolSummaryMsg
  // Turn lifecycle
  | TurnCompleteMsg
  | TurnErrorMsg
  // Session
  | SessionInitMsg
  | SessionStatusMsg
  | SessionClosedMsg
  // Context
  | ContextCompactedMsg
  | RateLimitMsg
  // Tasks (agent teams)
  | TaskStartedMsg
  | TaskProgressMsg
  | TaskNotificationMsg
  // System
  | HookEventMsg
  | AuthStatusMsg
  | FilesSavedMsg
  | CommandOutputMsg
  | PromptSuggestionMsg
  // Interactive cards
  | PermissionRequestMsg
  | AskUserQuestionMsg
  | PlanApprovalMsg
  | ElicitationMsg
  | ElicitationCompleteMsg
  // Infrastructure
  | ErrorMsg
  | PongMsg
  | HeartbeatConfigMsg
  | UnknownSdkEventMsg

// --- Close codes --------------------------------------------------

export const CLOSE_CODES = {
  NORMAL: 1000,
  SESSION_NOT_FOUND: 4004,
  SIDECAR_UNAVAILABLE: 4100,
  SIDECAR_WS_FAILED: 4101,
  SIDECAR_STREAM_ENDED: 4102,
  HEARTBEAT_TIMEOUT: 4200,
  SERVER_SHUTDOWN: 4500,
} as const

export const NON_RECOVERABLE_CODES = new Set([
  CLOSE_CODES.SESSION_NOT_FOUND,
  CLOSE_CODES.SIDECAR_UNAVAILABLE,
  CLOSE_CODES.SIDECAR_WS_FAILED,
  CLOSE_CODES.SERVER_SHUTDOWN,
])

// --- Client -> Server messages ------------------------------------

export interface ResumeClientMsg {
  type: 'resume'
  lastSeq: number
}

export type PermissionMode = 'default' | 'acceptEdits' | 'bypassPermissions' | 'plan' | 'dontAsk'

export interface SetModeClientMsg {
  type: 'set_mode'
  mode: PermissionMode
}

// Control session info for takeover flow
export interface ControlSessionInfo {
  sessionId: string
  status: 'idle' | 'running' | 'completed'
  origin: 'claude-view' | 'external'
}

// --- Chat display types -------------------------------------------

export interface ChatMessage {
  role: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'thinking'
  content?: string
  messageId?: string
  toolName?: string
  toolInput?: Record<string, unknown>
  toolUseId?: string
  output?: string
  isError?: boolean
}

/** Message lifecycle status for optimistic rendering */
export type MessageStatus = 'optimistic' | 'sending' | 'sent' | 'failed'

/** ChatMessage with lifecycle tracking */
export interface ChatMessageWithStatus extends ChatMessage {
  content: string
  localId: string
  status: MessageStatus
  createdAt: number
}
