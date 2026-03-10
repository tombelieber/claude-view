// sidecar/src/types.ts
// IPC message protocol: Frontend <-> Axum <-> Sidecar

// ── Frontend → Sidecar (via Axum WS proxy) ──

export interface UserMessage {
  type: 'user_message'
  content: string
}

export interface PermissionResponse {
  type: 'permission_response'
  requestId: string
  allowed: boolean
}

export interface PingMessage {
  type: 'ping'
}

export interface QuestionResponse {
  type: 'question_response'
  requestId: string
  answers: Record<string, string> // { "question text": "selected option label" }
}

export interface PlanResponse {
  type: 'plan_response'
  requestId: string
  approved: boolean
  feedback?: string
}

export interface ElicitationResponse {
  type: 'elicitation_response'
  requestId: string
  response: string
}

export interface ResumeMsg {
  type: 'resume'
  lastSeq: number
}

export interface SetModeMessage {
  type: 'set_mode'
  mode: 'default' | 'acceptEdits' | 'bypassPermissions' | 'plan' | 'dontAsk'
}

export type ClientMessage =
  | UserMessage
  | PermissionResponse
  | QuestionResponse
  | PlanResponse
  | ElicitationResponse
  | PingMessage
  | ResumeMsg
  | SetModeMessage

// ── Sidecar → Frontend (via Axum WS proxy) ──

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

export interface ToolUseStart {
  type: 'tool_use_start'
  toolName: string
  toolInput: Record<string, unknown>
  toolUseId: string
}

export interface ToolUseResult {
  type: 'tool_use_result'
  toolUseId: string
  output: string
  isError: boolean
}

export interface ThinkingMessage {
  type: 'thinking'
  content: string
  messageId: string
}

export interface PermissionRequest {
  type: 'permission_request'
  requestId: string
  toolName: string
  toolInput: Record<string, unknown>
  description: string
  timeoutMs: number
}

export interface SessionStatusMessage {
  type: 'session_status'
  status: 'active' | 'waiting_input' | 'waiting_permission' | 'completed' | 'error'
  contextUsage: number
  turnCount: number
  tokenUsage?: {
    input: number
    output: number
    cacheRead: number
    cacheCreation: number
  }
  costUsd?: number
  model?: string
  contextWindow?: number
}

export interface ErrorMessage {
  type: 'error'
  message: string
  fatal: boolean
}

export interface PongMessage {
  type: 'pong'
}

// Interactive card messages — sidecar → frontend
// Emitted when canUseTool intercepts AskUserQuestion, ExitPlanMode, or Elicitation

export interface AskUserQuestionMessage {
  type: 'ask_user_question'
  requestId: string
  questions: {
    question: string
    header: string
    options: { label: string; description: string; markdown?: string }[]
    multiSelect: boolean
  }[]
}

export interface PlanApprovalMessage {
  type: 'plan_approval'
  requestId: string
  planData: Record<string, unknown> // ExitPlanMode tool input (allowedPrompts, etc.)
}

export interface ElicitationMessage {
  type: 'elicitation'
  requestId: string
  prompt: string
}

export interface HeartbeatConfig {
  type: 'heartbeat_config'
  intervalMs: number
  // NOTE: seq is NOT baked in. Heartbeat_config is a connection-scoped setup
  // message sent directly via ws.send() — NOT through emitSequenced.
  // It's re-sent on each WS open, so replay is unnecessary.
}

export type ServerMessage =
  | AssistantChunk
  | AssistantDone
  | ToolUseStart
  | ToolUseResult
  | ThinkingMessage
  | PermissionRequest
  | AskUserQuestionMessage
  | PlanApprovalMessage
  | ElicitationMessage
  | SessionStatusMessage
  | ErrorMessage
  | PongMessage
  | HeartbeatConfig

// Wrapper type for sequenced messages:
export type SequencedServerMessage = ServerMessage & { seq: number }

// ── HTTP request/response types ──

export interface ResumeRequest {
  sessionId: string
  model?: string
  projectPath?: string
}

export interface ResumeResponse {
  controlId: string
  status: 'active' | 'already_active'
  sessionId: string
}

export interface SendRequest {
  controlId: string
  message: string
}

export interface ActiveSession {
  controlId: string
  sessionId: string
  status: string
  turnCount: number
  totalCost: number | null
  startedAt: number
}

export interface HealthResponse {
  status: 'ok'
  activeSessions: number
  uptime: number
}
