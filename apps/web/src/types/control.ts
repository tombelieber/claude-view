// apps/web/src/types/control.ts
// Connection-level types for the sidecar control protocol.
// Full event types live in @claude-view/shared (sidecar-protocol.ts).

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

/** Connection health state for the connection banner. */
export type ConnectionHealth = 'ok' | 'degraded' | 'lost'

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

export type PermissionMode = 'default' | 'acceptEdits' | 'bypassPermissions' | 'plan' | 'dontAsk'

// Control session info for takeover flow
export interface ControlSessionInfo {
  sessionId: string
  status: 'idle' | 'running' | 'completed'
  origin: 'claude-view' | 'external'
}
