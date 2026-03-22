import type { ConversationBlock } from '@claude-view/shared/types/blocks'

// ─── Leaf FSMs (sdk_owned children only) ─────────────────────
export type TurnState =
  | { turn: 'idle' }
  | { turn: 'streaming' }
  | {
      turn: 'awaiting'
      kind: 'permission' | 'question' | 'plan' | 'elicitation'
      requestId: string
    }
  | { turn: 'compacting' }

export type ConnHealth = { health: 'ok' } | { health: 'reconnecting'; attempt: number }

// ─── Sub-state types ─────────────────────────────────────────
export type NobodySub =
  | { sub: 'loading'; pendingLive?: 'cc_owned' }
  | { sub: 'ready'; blocks: ConversationBlock[] }

export type CcCliSub = { sub: 'watching' } | { sub: 'takeover_killing' }

export type AcquireAction = 'create' | 'resume' | 'fork'

export type AcquiringStep =
  | { step: 'posting' }
  | { step: 'ws_connecting'; controlId: string }
  | { step: 'ws_initializing'; controlId: string }

export type RecoveringKind =
  | { kind: 'action_failed'; error: string }
  | { kind: 'ws_fatal'; error: string }
  | { kind: 'replaced' }

// ─── Panel State (hierarchical tree) ─────────────────────────
export type PanelState =
  | { phase: 'empty' }
  | { phase: 'nobody'; sessionId: string; sub: NobodySub }
  | { phase: 'cc_cli'; sessionId: string; blocks: ConversationBlock[]; sub: CcCliSub }
  | {
      phase: 'acquiring'
      sessionId: string
      targetSessionId: string | null
      action: AcquireAction
      historyBlocks: ConversationBlock[]
      pendingMessage: string | null
      step: AcquiringStep
    }
  | {
      phase: 'sdk_owned'
      sessionId: string
      controlId: string
      blocks: ConversationBlock[]
      pendingText: string
      ephemeral: boolean
      turn: TurnState
      conn: ConnHealth
    }
  | {
      phase: 'recovering'
      sessionId: string
      blocks: ConversationBlock[]
      recovering: RecoveringKind
    }
  | { phase: 'closed'; sessionId: string; blocks: ConversationBlock[]; ephemeral: boolean }

// ─── Session Metadata (orthogonal, survives reconnect) ───────
export interface SessionMeta {
  model: string
  permissionMode: string
  slashCommands: string[]
  mcpServers: { name: string; status: string }[]
  skills: string[]
  agents: string[]
  capabilities: string[]
  totalInputTokens: number
  contextWindowSize: number
}

// ─── Outbox (orthogonal, text-based reconciliation) ──────────
export interface OutboxEntry {
  localId: string
  text: string
  status: 'queued' | 'sent' | 'failed'
  sentAt?: number
}

export interface OutboxState {
  messages: OutboxEntry[]
}

// ─── History pagination (orthogonal, survives phase transitions) ─
export interface HistoryPagination {
  total: number
  offset: number
  fetchingOlder: boolean
}

// ─── Composed Store ──────────────────────────────────────────
export interface ChatPanelStore {
  panel: PanelState
  outbox: OutboxState
  meta: SessionMeta | null
  /** Decoded project path for the current session — threaded through from LiveSession
   *  or sidebar data so the sidecar can pass the correct `cwd` to the SDK on resume/fork. */
  projectPath: string | null
  /** Last model used in a SEND_MESSAGE — fallback for retries from recovering/closed. */
  lastModel: string | null
  /** Last permissionMode used in a SEND_MESSAGE — fallback for retries from recovering/closed. */
  lastPermissionMode: string | null
  /** Pagination state for history blocks — tracks offset/total for infinite scroll. */
  historyPagination: HistoryPagination | null
}

// ─── Events ──────────────────────────────────────────────────
export type RawEvent =
  // Navigation
  | { type: 'SELECT_SESSION'; sessionId: string; projectPath?: string }
  | { type: 'DESELECT' }
  // History
  | { type: 'HISTORY_OK'; blocks: ConversationBlock[]; total?: number; offset?: number }
  | { type: 'HISTORY_FAILED'; error: string }
  // Pagination (scroll-up infinite load)
  | { type: 'LOAD_OLDER_HISTORY' }
  | { type: 'OLDER_HISTORY_OK'; blocks: ConversationBlock[]; offset: number }
  // Active check
  | { type: 'SIDECAR_HAS_SESSION'; controlId: string; sessionState?: string }
  | { type: 'SIDECAR_NO_SESSION' }
  // User actions (E-B1: localId at call site for StrictMode safety)
  | { type: 'SEND_MESSAGE'; text: string; localId: string; model?: string; permissionMode?: string }
  | { type: 'FORK_SESSION'; message?: string }
  | { type: 'TAKEOVER_CLI' }
  | { type: 'RESUME_WITH_OPTIONS'; permissionMode?: string; model?: string }
  | {
      type: 'RESPOND_PERMISSION'
      requestId: string
      allowed: boolean
      updatedPermissions?: unknown[]
    }
  | { type: 'ANSWER_QUESTION'; requestId: string; answers: Record<string, string> }
  | { type: 'APPROVE_PLAN'; requestId: string; approved: boolean; feedback?: string }
  | { type: 'SUBMIT_ELICITATION'; requestId: string; response: string }
  | { type: 'INTERRUPT' }
  | { type: 'RETRY_MESSAGE'; localId: string } // E-B3
  // Acquire lifecycle
  | { type: 'ACQUIRE_OK'; controlId: string; sessionId?: string }
  | { type: 'ACQUIRE_FAILED'; error: string }
  // WS lifecycle
  | { type: 'WS_OPEN' }
  | { type: 'WS_CLOSE'; code: number; recoverable: boolean }
  // Sidecar protocol
  | {
      type: 'SESSION_INIT'
      model: string
      permissionMode: string
      slashCommands: string[]
      mcpServers: { name: string; status: string }[]
      skills: string[]
      agents: string[]
      capabilities: string[]
    }
  | { type: 'BLOCKS_SNAPSHOT'; blocks: ConversationBlock[] }
  | { type: 'BLOCKS_UPDATE'; blocks: ConversationBlock[] }
  | { type: 'STREAM_DELTA'; text: string }
  | {
      type: 'TURN_COMPLETE'
      blocks: ConversationBlock[]
      totalInputTokens: number
      contextWindowSize: number
    }
  | {
      type: 'TURN_ERROR'
      blocks: ConversationBlock[]
      totalInputTokens: number
      contextWindowSize: number
    }
  | {
      type: 'PERMISSION_REQUEST'
      kind: 'permission' | 'question' | 'plan' | 'elicitation'
      requestId: string
    }
  | { type: 'SESSION_COMPACTING' }
  | { type: 'COMPACT_DONE' }
  | { type: 'SESSION_CLOSED' }
  // Meta updates (E-M5: split MODE_CHANGED into server/user events)
  | { type: 'SERVER_MODE_CONFIRMED'; mode: string }
  | { type: 'SERVER_MODE_REJECTED'; mode: string; reason?: string }
  | { type: 'SET_PERMISSION_MODE'; mode: string }
  | { type: 'COMMANDS_UPDATED'; commands: string[] }
  | { type: 'AGENTS_UPDATED'; agents: string[] }
  // SSE (pre-computed prop)
  | {
      type: 'LIVE_STATUS_CHANGED'
      status: 'cc_owned' | 'cc_agent_sdk_owned' | 'inactive'
      projectPath?: string
    }
  // Terminal WS block stream (watching mode)
  | { type: 'TERMINAL_BLOCK'; block: ConversationBlock }
  | { type: 'TERMINAL_CONNECTED' }
  // Takeover lifecycle
  | { type: 'KILL_CLI_OK' }
  | { type: 'KILL_CLI_FAILED'; error: string }
  | { type: 'TAKEOVER_TIMEOUT' }
  // Timers
  | { type: 'INIT_TIMEOUT' }
  | { type: 'RECONNECT_ATTEMPT' }
  | { type: 'FAIL_TIMER_FIRED'; localId: string }

// ─── Commands (side effects as data) ─────────────────────────
export type Command =
  | { cmd: 'FETCH_HISTORY'; sessionId: string; limit?: number; offset?: number }
  | { cmd: 'FETCH_OLDER_HISTORY'; sessionId: string; offset: number; limit: number }
  | { cmd: 'CHECK_SIDECAR_ACTIVE'; sessionId: string }
  | {
      cmd: 'POST_CREATE'
      model: string
      message?: string
      permissionMode?: string
      persistSession?: boolean
      projectPath?: string
    }
  | {
      cmd: 'POST_RESUME'
      sessionId: string
      permissionMode?: string
      model?: string
      resumeAtMessageId?: string
      message?: string
      projectPath?: string
    }
  | { cmd: 'POST_FORK'; sessionId: string; message?: string; projectPath?: string }
  | { cmd: 'OPEN_SIDECAR_WS'; sessionId: string }
  | { cmd: 'CLOSE_SIDECAR_WS' }
  | { cmd: 'OPEN_TERMINAL_WS'; sessionId: string }
  | { cmd: 'CLOSE_TERMINAL_WS' }
  | { cmd: 'WS_SEND'; message: Record<string, unknown> }
  | { cmd: 'INVALIDATE_HISTORY'; sessionId: string }
  | { cmd: 'INVALIDATE_SIDEBAR' }
  | { cmd: 'KILL_CLI_SESSION'; sessionId: string }
  | { cmd: 'START_TIMER'; id: string; delayMs: number; event: RawEvent }
  | { cmd: 'CANCEL_TIMER'; id: string }
  | { cmd: 'TOAST'; message: string; variant: 'error' | 'info' | 'success' }
  | { cmd: 'NAVIGATE'; path: string }
  | { cmd: 'TRACK_EVENT'; name: string }

// ─── Coordinator return type ─────────────────────────────────
export type TransitionResult = [ChatPanelStore, Command[]]

// ─── View derivation types ───────────────────────────────────
export type InputBarState =
  | 'dormant' // E-M3: empty panel, no session
  | 'active'
  | 'connecting'
  | 'streaming'
  | 'waiting_permission'
  | 'reconnecting'
  | 'completed'
  | 'controlled_elsewhere'

export type ViewMode =
  | 'blank'
  | 'loading'
  | 'history'
  | 'watching'
  | 'connecting'
  | 'active'
  | 'error'
  | 'closed'
