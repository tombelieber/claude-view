/**
 * Relay protocol types — matches the Rust wire format exactly.
 *
 * The Mac's relay_client.rs sends `serde_json::to_vec(&session)` where
 * session is a `LiveSession` with `#[serde(rename_all = "camelCase")]`.
 * Every field name here must match the Rust camelCase serialization.
 */

// -- Agent state (matches crates/server/src/live/state.rs AgentState) --

export type AgentStateGroup = 'needs_you' | 'autonomous'

export interface AgentState {
  group: AgentStateGroup
  state: string
  label: string
  context?: unknown
}

// -- Session status (matches state.rs SessionStatus) --
// Rust enum: Working | Paused | Done, serialized as snake_case strings

export type SessionStatus = 'working' | 'paused' | 'done'

// -- Token usage (matches crates/core/src/pricing.rs TokenUsage) --

export interface TokenUsage {
  inputTokens: number
  outputTokens: number
  cacheReadTokens: number
  cacheCreationTokens: number
  cacheCreation5mTokens: number
  cacheCreation1hrTokens: number
  totalTokens: number
}

// -- Cost breakdown (matches crates/core/src/pricing.rs CostBreakdown) --

export interface CostBreakdown {
  totalUsd: number
  inputCostUsd: number
  outputCostUsd: number
  cacheReadCostUsd: number
  cacheCreationCostUsd: number
  cacheSavingsUsd: number
  hasUnpricedUsage: boolean
  unpricedInputTokens: number
  unpricedOutputTokens: number
  unpricedCacheReadTokens: number
  unpricedCacheCreationTokens: number
  pricedTokenCoverage: number
  totalCostSource: 'computed_priced_tokens_full' | 'computed_priced_tokens_partial'
}

// -- Cache status (matches pricing.rs CacheStatus) --

export type CacheStatus = 'warm' | 'cold' | 'unknown'

// -- Sub-agent info --

export interface SubAgentInfo {
  id: string
  name?: string
  description?: string
  agentType?: string
  status: string
  startedAt?: number
  completedAt?: number
}

// -- Progress item --

export interface ProgressItem {
  content: string
  status: string
  activeForm?: string
}

// -- Tool used --

export interface ToolUsed {
  name: string
  kind: string
}

// -- LiveSession (matches crates/server/src/live/state.rs LiveSession) --
// This is what the Mac sends per-session over the relay WebSocket.

export interface RelaySession {
  id: string
  project: string
  projectDisplayName: string
  projectPath: string
  filePath: string
  status: SessionStatus
  agentState: AgentState
  gitBranch: string | null
  pid: number | null
  title: string
  lastUserMessage: string
  lastUserFile: string | null
  currentActivity: string
  turnCount: number
  startedAt: number | null
  lastActivityAt: number
  model: string | null
  tokens: TokenUsage
  contextWindowTokens: number
  cost: CostBreakdown
  cacheStatus: CacheStatus
  currentTurnStartedAt: number | null
  lastTurnTaskSeconds: number | null
  subAgents?: SubAgentInfo[]
  progressItems?: ProgressItem[]
  toolsUsed?: ToolUsed[]
  lastCacheHitAt: number | null
}

// -- Session events (matches state.rs SessionEvent, tagged enum) --
// The Mac sends these as encrypted payloads. After decryption, each
// message is one of these event types.

export interface SessionDiscoveredEvent {
  type: 'session_discovered'
  session: RelaySession
}

export interface SessionUpdatedEvent {
  type: 'session_updated'
  session: RelaySession
}

export interface SessionCompletedEvent {
  type: 'session_completed'
  sessionId: string
}

/** Mac → Phone: live output stream */
export interface RelayOutputStream {
  type: 'output'
  sessionId: string
  chunks: RelayOutputChunk[]
}

export interface RelayOutputChunk {
  role: 'assistant' | 'tool' | 'user'
  text?: string
  name?: string
  path?: string
}

/** Phone → Mac: command */
export interface RelayCommand {
  type: 'command'
  action: string
  sessionId?: string
  [key: string]: unknown
}

/**
 * Union of all relay message types.
 *
 * NOTE: The Mac's relay_client.rs sends individual LiveSession objects
 * (not wrapped in an event envelope) for the initial snapshot. For
 * broadcast events, it sends the tagged SessionEvent format. The phone's
 * relay hook must handle both:
 *   1. Raw RelaySession (no `type` field) — initial snapshot item
 *   2. Tagged events with `type` field — ongoing updates
 */
export type RelayMessage =
  | SessionDiscoveredEvent
  | SessionUpdatedEvent
  | SessionCompletedEvent
  | RelayOutputStream
  | RelayCommand
