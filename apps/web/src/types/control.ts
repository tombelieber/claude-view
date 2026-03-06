// apps/web/src/types/control.ts
// Mirrors sidecar/src/types.ts for frontend consumption

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

export interface ResumeResponse {
  controlId: string
  status: 'active' | 'already_active'
  sessionId: string
  error?: string
}

// WebSocket message types (sidecar → frontend)

export interface AssistantChunk {
  type: 'assistant_chunk'
  content: string
  messageId: string
}

export interface AssistantDone {
  type: 'assistant_done'
  messageId: string
  usage: {
    inputTokens: number
    outputTokens: number
    cacheReadTokens: number
    cacheWriteTokens: number
  }
  cost: number | null
  totalCost: number | null
}

export interface ToolUseStartMsg {
  type: 'tool_use_start'
  toolName: string
  toolInput: Record<string, unknown>
  toolUseId: string
}

export interface ToolUseResultMsg {
  type: 'tool_use_result'
  toolUseId: string
  output: string
  isError: boolean
}

export interface PermissionRequestMsg {
  type: 'permission_request'
  requestId: string
  toolName: string
  toolInput: Record<string, unknown>
  description: string
  timeoutMs: number
}

export interface SessionStatusMsg {
  type: 'session_status'
  status: 'active' | 'waiting_input' | 'waiting_permission' | 'completed' | 'error'
  contextUsage: number
  turnCount: number
}

export interface ErrorMsg {
  type: 'error'
  message: string
  fatal: boolean
}

export interface PongMsg {
  type: 'pong'
}

// Interactive card messages — sidecar → frontend
export interface AskUserQuestionMsg {
  type: 'ask_user_question'
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
  prompt: string
}

export type ServerMessage =
  | AssistantChunk
  | AssistantDone
  | ToolUseStartMsg
  | ToolUseResultMsg
  | PermissionRequestMsg
  | AskUserQuestionMsg
  | PlanApprovalMsg
  | ElicitationMsg
  | SessionStatusMsg
  | ErrorMsg
  | PongMsg

// Control session info for takeover flow
export interface ControlSessionInfo {
  sessionId: string
  controlId: string
  status: 'idle' | 'running' | 'completed'
  origin: 'claude-view' | 'external'
}

// Chat message for display
export interface ChatMessage {
  role: 'user' | 'assistant' | 'tool_use' | 'tool_result'
  content?: string
  messageId?: string
  toolName?: string
  toolInput?: Record<string, unknown>
  toolUseId?: string
  output?: string
  isError?: boolean
  usage?: AssistantDone['usage']
}

/** Message lifecycle status for optimistic rendering */
export type MessageStatus = 'optimistic' | 'sending' | 'sent' | 'failed'

/** ChatMessage with lifecycle tracking — content is required (user messages always have text) */
export interface ChatMessageWithStatus extends ChatMessage {
  /** Required content (narrows the optional inherited field) */
  content: string
  /** Unique ID for this message instance (used for retry/status updates) */
  localId: string
  /** Lifecycle status */
  status: MessageStatus
  /** Timestamp when the message was created locally */
  createdAt: number
}
