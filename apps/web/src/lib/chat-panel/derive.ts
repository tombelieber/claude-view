import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import type { ChatPanelStore, InputBarState, ViewMode } from './types'

export type ConnectionStatusInfo = {
  message: string
  kind: 'loading' | 'error'
}

// ─── Helpers ───────────────────────────────────────────────

function getBaseBlocks(store: ChatPanelStore): ConversationBlock[] {
  const { panel } = store
  switch (panel.phase) {
    case 'empty':
      return []
    case 'nobody':
      return panel.sub.sub === 'ready' ? panel.sub.blocks : []
    case 'cc_cli':
      return panel.blocks
    case 'acquiring':
      return panel.historyBlocks
    case 'sdk_owned': {
      // Strip `streaming` from server blocks — we manage the streaming cursor
      // exclusively via the synthetic __pending__ block below.  Without this,
      // a BLOCKS_UPDATE that includes `streaming: true` on the last assistant
      // block produces a SECOND pulsing cursor above the real one.
      const blocks = panel.pendingText
        ? panel.blocks.map((b) =>
            b.type === 'assistant' && b.streaming ? { ...b, streaming: false } : b,
          )
        : [...panel.blocks]

      if (panel.pendingText) {
        blocks.push({
          type: 'assistant',
          id: '__pending__',
          segments: [{ kind: 'text', text: panel.pendingText }],
          streaming: true,
          timestamp: Date.now(),
        })
      }
      return blocks
    }
    case 'recovering':
      return panel.blocks
    case 'closed':
      return panel.blocks
  }
}

function reconcileOutbox(blocks: ConversationBlock[], store: ChatPanelStore): ConversationBlock[] {
  const { outbox } = store
  if (outbox.messages.length === 0) return blocks

  const existingTexts = new Set(
    blocks
      .filter((b): b is ConversationBlock & { type: 'user' } => b.type === 'user')
      .map((b) => b.text),
  )

  const synthetic: ConversationBlock[] = []
  for (const entry of outbox.messages) {
    if (!existingTexts.has(entry.text)) {
      synthetic.push({
        type: 'user',
        id: `outbox-${entry.localId}`,
        text: entry.text,
        timestamp: Date.now(),
        localId: entry.localId,
        status: entry.status === 'failed' ? 'failed' : 'optimistic',
      } as ConversationBlock)
    }
  }

  return synthetic.length > 0 ? [...blocks, ...synthetic] : blocks
}

// ─── Public API ────────────────────────────────────────────

export function deriveBlocks(store: ChatPanelStore): ConversationBlock[] {
  const blocks = getBaseBlocks(store)
  return reconcileOutbox(blocks, store)
}

export function deriveCanSend(store: ChatPanelStore): boolean {
  const { panel } = store
  switch (panel.phase) {
    case 'empty':
      return true
    case 'nobody':
      return panel.sub.sub === 'ready'
    case 'cc_cli':
      return panel.sub.sub === 'watching'
    case 'acquiring':
      return false
    case 'sdk_owned':
      return panel.turn.turn === 'idle'
    case 'recovering':
      return true
    case 'closed':
      return !panel.ephemeral
  }
}

export function deriveCanFork(store: ChatPanelStore): boolean {
  const { panel } = store
  switch (panel.phase) {
    case 'nobody':
      return panel.sub.sub === 'ready' && panel.sub.blocks.length > 0
    case 'sdk_owned':
      return panel.blocks.length > 0
    default:
      return false
  }
}

export function deriveInputBar(store: ChatPanelStore): InputBarState {
  const { panel } = store
  switch (panel.phase) {
    case 'empty':
      return 'dormant'
    case 'nobody':
      return 'active'
    case 'cc_cli':
      return 'controlled_elsewhere'
    case 'acquiring':
      return 'connecting'
    case 'sdk_owned': {
      if (panel.conn.health === 'reconnecting') return 'reconnecting'
      switch (panel.turn.turn) {
        case 'idle':
          return 'active'
        case 'pending':
        case 'streaming':
          return 'streaming'
        case 'awaiting':
          return 'waiting_permission'
        case 'compacting':
          return 'streaming'
      }
      break
    }
    case 'recovering':
      return 'active'
    case 'closed':
      return 'completed'
  }
}

/**
 * Structured status indicator shown in the thread during
 * acquiring/recovering phases so users know what's happening.
 * Returns null when no status indicator is needed.
 * `kind` distinguishes loading (spinner) from error (static icon).
 */
export function deriveConnectionStatus(store: ChatPanelStore): ConnectionStatusInfo | null {
  const { panel } = store
  switch (panel.phase) {
    case 'acquiring': {
      const verb =
        panel.action === 'create' ? 'Creating' : panel.action === 'fork' ? 'Forking' : 'Resuming'
      switch (panel.step.step) {
        case 'posting':
          return { message: `${verb} session...`, kind: 'loading' }
        case 'ws_connecting':
          return { message: 'Connecting to session...', kind: 'loading' }
        case 'ws_initializing':
          return { message: 'Initializing...', kind: 'loading' }
      }
      return { message: `${verb} session...`, kind: 'loading' }
    }
    case 'recovering':
      if (panel.recovering.kind === 'ws_fatal')
        return { message: 'Connection lost. Reconnecting...', kind: 'loading' }
      if (panel.recovering.kind === 'replaced')
        return { message: 'Session taken over by another client', kind: 'error' }
      return { message: `Resume failed: ${panel.recovering.error}`, kind: 'error' }
    case 'sdk_owned':
      if (panel.conn.health === 'reconnecting') {
        return { message: `Reconnecting... (attempt ${panel.conn.attempt})`, kind: 'loading' }
      }
      return null
    default:
      return null
  }
}

export function deriveHistoryPagination(store: ChatPanelStore): {
  hasOlderMessages: boolean
  isFetchingOlder: boolean
} {
  const pg = store.historyPagination
  if (!pg) return { hasOlderMessages: false, isFetchingOlder: false }
  return {
    hasOlderMessages: pg.offset > 0,
    isFetchingOlder: pg.fetchingOlder,
  }
}

// ─── Thinking state (spinner verbs indicator) ─────────────
export type ThinkingPhase = 'loading' | 'connecting' | 'thinking'

/**
 * Derives when the inline thinking indicator should show.
 * Covers the gap: session loading / acquiring / agent thinking before first output.
 *
 *  - 'loading'    — fetching session history (nobody.loading)
 *  - 'connecting' — acquiring phase (POST / WS connect / WS init)
 *  - 'thinking'   — sdk_owned streaming with no text output yet
 *
 * NOTE: We intentionally do NOT check outbox entries here. Outbox entries
 * persist with status 'sent' even after TURN_COMPLETE, which would keep
 * the indicator visible after the response is fully rendered. The ~0.5s gap
 * between send (sdk_owned.idle) and first STREAM_DELTA is acceptable.
 */
export function deriveThinkingState(store: ChatPanelStore): ThinkingPhase | null {
  const { panel } = store

  // History loading
  if (panel.phase === 'nobody' && panel.sub.sub === 'loading') return 'loading'

  // Acquiring (creating / resuming / forking session)
  if (panel.phase === 'acquiring') return 'connecting'

  // SDK owned: message sent, waiting for first server response
  if (panel.phase === 'sdk_owned' && panel.turn.turn === 'pending') return 'thinking'

  // SDK owned, streaming turn, but agent hasn't produced text yet
  if (panel.phase === 'sdk_owned' && panel.turn.turn === 'streaming' && !panel.pendingText) {
    return 'thinking'
  }

  return null
}

export function deriveViewMode(store: ChatPanelStore): ViewMode {
  const { panel } = store
  switch (panel.phase) {
    case 'empty':
      return 'blank'
    case 'nobody':
      return panel.sub.sub === 'loading' ? 'loading' : 'history'
    case 'cc_cli':
      return 'watching'
    case 'acquiring':
      return 'connecting'
    case 'sdk_owned':
      return 'active'
    case 'recovering':
      return 'error'
    case 'closed':
      return 'closed'
  }
}
