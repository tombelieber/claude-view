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

export type ClientMessage = UserMessage | PermissionResponse | PingMessage

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
  cost: number
  totalCost: number
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
}

export interface ErrorMessage {
  type: 'error'
  message: string
  fatal: boolean
}

export interface PongMessage {
  type: 'pong'
}

export type ServerMessage =
  | AssistantChunk
  | AssistantDone
  | ToolUseStart
  | ToolUseResult
  | PermissionRequest
  | SessionStatusMessage
  | ErrorMessage
  | PongMessage

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
  totalCost: number
  startedAt: number
}

export interface HealthResponse {
  status: 'ok'
  activeSessions: number
  uptime: number
}
