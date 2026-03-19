// sidecar/src/protocol.ts
// Complete protocol: sidecar ↔ frontend over WebSocket
// Every SDK message type maps to one or more of these events.

// ─── Server → Client Events ───────────────────────────────────────

export interface AssistantText {
  type: 'assistant_text'
  text: string
  messageId: string // SDK msg.uuid
  parentToolUseId: string | null
}

export interface AssistantThinking {
  type: 'assistant_thinking'
  thinking: string
  messageId: string
  parentToolUseId: string | null
}

export interface AssistantError {
  type: 'assistant_error'
  error:
    | 'authentication_failed'
    | 'billing_error'
    | 'rate_limit'
    | 'invalid_request'
    | 'server_error'
    | 'unknown'
    | 'max_output_tokens'
  messageId: string
}

export interface StreamDelta {
  type: 'stream_delta'
  event: unknown // Raw BetaRawMessageStreamEvent
  messageId: string // UUID — groups deltas to their parent message
  deltaType: string // 'content_block_start' | 'content_block_delta' | etc.
  textDelta?: string // Pre-extracted text for rendering
  thinkingDelta?: string // Pre-extracted thinking text
  toolInputDelta?: string // Pre-extracted tool input JSON fragment
}

export interface ToolUseStart {
  type: 'tool_use_start'
  toolName: string
  toolInput: Record<string, unknown>
  toolUseId: string
  messageId: string
  parentToolUseId: string | null
}

export interface ToolUseResult {
  type: 'tool_use_result'
  toolUseId: string
  output: string
  isError: boolean
  isReplay: boolean
}

export interface ToolProgress {
  type: 'tool_progress'
  toolUseId: string
  toolName: string
  elapsedSeconds: number
  parentToolUseId: string | null
  taskId?: string
}

export interface ToolSummary {
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

export interface TurnComplete {
  type: 'turn_complete'
  totalCostUsd: number
  numTurns: number
  durationMs: number
  durationApiMs: number
  usage: Record<string, number> // NonNullableUsage flattened
  modelUsage: Record<string, ModelUsageInfo>
  permissionDenials: { toolName: string; toolUseId: string; toolInput: Record<string, unknown> }[]
  result: string
  structuredOutput?: unknown
  stopReason: string | null
  fastModeState?: 'off' | 'cooldown' | 'on'
}

export interface TurnError {
  type: 'turn_error'
  subtype:
    | 'error_during_execution'
    | 'error_max_turns'
    | 'error_max_budget_usd'
    | 'error_max_structured_output_retries'
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

export interface SessionInit {
  type: 'session_init'
  sessionId?: string // NEW — optional during transition; populated from SDK message session_id
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
  capabilities?: string[]
}

export interface SessionStatus {
  type: 'session_status'
  status: 'compacting' | null
  permissionMode?: string
}

export interface ContextCompacted {
  type: 'context_compacted'
  trigger: 'manual' | 'auto'
  preTokens: number
}

export interface ElicitationComplete {
  type: 'elicitation_complete'
  mcpServerName: string
  elicitationId: string
}

export interface RateLimit {
  type: 'rate_limit'
  status: 'allowed' | 'allowed_warning' | 'rejected'
  resetsAt?: number
  utilization?: number
  rateLimitType?: string
  overageStatus?: string
  isUsingOverage?: boolean
}

export interface TaskStarted {
  type: 'task_started'
  taskId: string
  toolUseId?: string
  description: string
  taskType?: string
  prompt?: string
}

export interface TaskProgressEvent {
  type: 'task_progress'
  taskId: string
  toolUseId?: string
  description: string
  lastToolName?: string
  summary?: string
  usage: { totalTokens: number; toolUses: number; durationMs: number }
}

export interface TaskNotification {
  type: 'task_notification'
  taskId: string
  toolUseId?: string
  status: 'completed' | 'failed' | 'stopped'
  outputFile: string
  summary: string
  usage?: { totalTokens: number; toolUses: number; durationMs: number }
}

export interface HookEvent {
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

export interface AuthStatus {
  type: 'auth_status'
  isAuthenticating: boolean
  output: string[]
  error?: string
}

export interface FilesSaved {
  type: 'files_saved'
  files: { filename: string; fileId: string }[]
  failed: { filename: string; error: string }[]
  processedAt: string
}

export interface PromptSuggestion {
  type: 'prompt_suggestion'
  suggestion: string
}

export interface CommandOutput {
  type: 'command_output'
  content: string
}

export interface UnknownSdkEvent {
  type: 'unknown_sdk_event'
  sdkType: string
  raw: unknown
}

export interface UserMessageEcho {
  type: 'user_message_echo'
  content: string
  timestamp: number
}

export interface SessionClosed {
  type: 'session_closed'
  reason: string
}

// ─── Interactive Cards (from canUseTool) ──────────────────────────

export interface PermissionRequest {
  type: 'permission_request'
  requestId: string
  toolName: string
  toolInput: Record<string, unknown>
  toolUseID: string
  suggestions?: unknown[] // PermissionUpdate[] from SDK
  decisionReason?: string
  blockedPath?: string
  agentID?: string
  timeoutMs: number
}

export interface AskQuestion {
  type: 'ask_question'
  requestId: string
  questions: {
    question: string
    header: string
    options: { label: string; description: string; markdown?: string }[]
    multiSelect: boolean
  }[]
}

export interface PlanApproval {
  type: 'plan_approval'
  requestId: string
  planData: Record<string, unknown>
}

export interface Elicitation {
  type: 'elicitation'
  requestId: string
  toolName: string
  toolInput: Record<string, unknown>
  prompt: string // extracted from toolInput.prompt — frontend renders this
}

// ─── Infrastructure ───────────────────────────────────────────────

export interface ErrorEvent {
  type: 'error'
  message: string
  fatal: boolean
}

export interface PongEvent {
  type: 'pong'
}

export interface HeartbeatConfig {
  type: 'heartbeat_config'
  intervalMs: number
}

// ─── Union Types ──────────────────────────────────────────────────

export type ServerEvent =
  // Assistant output
  | AssistantText
  | AssistantThinking
  | AssistantError
  | StreamDelta
  // Tool execution
  | ToolUseStart
  | ToolUseResult
  | ToolProgress
  | ToolSummary
  // Turn lifecycle
  | TurnComplete
  | TurnError
  // Session
  | SessionInit
  | SessionStatus
  | SessionClosed
  // Context
  | ContextCompacted
  | ElicitationComplete
  | RateLimit
  // Tasks (agent teams)
  | TaskStarted
  | TaskProgressEvent
  | TaskNotification
  // System
  | HookEvent
  | AuthStatus
  | FilesSaved
  | CommandOutput
  | PromptSuggestion
  // Interactive cards
  | PermissionRequest
  | AskQuestion
  | PlanApproval
  | Elicitation
  // Infrastructure
  | ErrorEvent
  | PongEvent
  | UnknownSdkEvent
  | UserMessageEcho
  // Blocks (server-driven message store)
  | { type: 'blocks_snapshot'; blocks: unknown[]; lastSeq: number }
  | { type: 'blocks_update'; blocks: unknown[] }

export type SequencedEvent = ServerEvent & { seq: number }

// ─── Client → Server Messages ─────────────────────────────────────

export interface UserMessage {
  type: 'user_message'
  content: string
}

export interface PermissionResponse {
  type: 'permission_response'
  requestId: string
  allowed: boolean
  updatedPermissions?: unknown[] // echo back suggestions for "always allow"
}

export interface QuestionResponse {
  type: 'question_response'
  requestId: string
  answers: Record<string, string>
}

export interface PlanResponseMsg {
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

export interface PingMsg {
  type: 'ping'
}

export interface SetModeMsg {
  type: 'set_mode'
  mode: 'default' | 'acceptEdits' | 'bypassPermissions' | 'plan' | 'dontAsk'
}

// Session control
export interface InterruptMsg {
  type: 'interrupt'
}
export interface SetModelMsg {
  type: 'set_model'
  model: string
}
export interface SetMaxThinkingTokensMsg {
  type: 'set_max_thinking_tokens'
  maxThinkingTokens: number | null
}
export interface StopTaskMsg {
  type: 'stop_task'
  taskId: string
}

// Info queries
export interface QueryModelsMsg {
  type: 'query_models'
  requestId?: string
}
export interface QueryCommandsMsg {
  type: 'query_commands'
  requestId?: string
}
export interface QueryAgentsMsg {
  type: 'query_agents'
  requestId?: string
}
export interface QueryMcpStatusMsg {
  type: 'query_mcp_status'
  requestId?: string
}
export interface QueryAccountInfoMsg {
  type: 'query_account_info'
  requestId?: string
}

// MCP management
export interface ReconnectMcpMsg {
  type: 'reconnect_mcp'
  serverName: string
}
export interface ToggleMcpMsg {
  type: 'toggle_mcp'
  serverName: string
  enabled: boolean
}
export interface SetMcpServersMsg {
  type: 'set_mcp_servers'
  servers: Record<string, unknown>
  requestId?: string
}

// File management
export interface RewindFilesMsg {
  type: 'rewind_files'
  userMessageId: string
  dryRun?: boolean
  requestId?: string
}

export type ClientMessage =
  | UserMessage
  | PermissionResponse
  | QuestionResponse
  | PlanResponseMsg
  | ElicitationResponse
  | ResumeMsg
  | PingMsg
  | SetModeMsg
  // Session control
  | InterruptMsg
  | SetModelMsg
  | SetMaxThinkingTokensMsg
  | StopTaskMsg
  // Info queries
  | QueryModelsMsg
  | QueryCommandsMsg
  | QueryAgentsMsg
  | QueryMcpStatusMsg
  | QueryAccountInfoMsg
  // MCP management
  | ReconnectMcpMsg
  | ToggleMcpMsg
  | SetMcpServersMsg
  // File management
  | RewindFilesMsg

// ─── Direct WS Reply Types (NOT in ServerEvent union) ────────────

export interface QueryResult {
  type: 'query_result'
  queryType: string
  data: unknown
  requestId?: string
}
export interface RewindResult {
  type: 'rewind_result'
  result: unknown
  requestId?: string
}
export interface McpSetResult {
  type: 'mcp_set_result'
  result: unknown
  requestId?: string
}

// ─── HTTP Request/Response Types ──────────────────────────────────

export interface CreateSessionRequest {
  model: string
  permissionMode?: string
  allowedTools?: string[]
  disallowedTools?: string[]
  projectPath?: string
  initialMessage?: string
}

export interface ResumeSessionRequest {
  sessionId: string
  model?: string
  permissionMode?: string
  projectPath?: string
}

export interface ForkSessionRequest {
  sessionId: string
  model?: string
  permissionMode?: string
  projectPath?: string
}

export interface PromptRequest {
  message: string
  model: string
  permissionMode?: string
}

export interface SessionResponse {
  controlId: string
  sessionId: string
  status: 'created' | 'resumed' | 'already_active'
}

export interface AvailableSession {
  sessionId: string
  summary: string
  lastModified: number
  fileSize: number
  customTitle?: string
  firstPrompt?: string
  gitBranch?: string
  cwd?: string
}

export interface ActiveSession {
  controlId: string
  sessionId: string
  state: string
  turnCount: number
  totalCostUsd: number | null
  startedAt: number
}

export interface HealthResponse {
  status: 'ok'
  activeSessions: number
  uptime: number
}
