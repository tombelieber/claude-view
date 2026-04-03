import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { BLOCK_PAGE_SIZE } from '../../block-pagination'
import type { ChatPanelStore, PanelState, TransitionResult } from '../types'

// ── Phase allowlist: phases that support pagination ──────────
type PaginatablePhase = 'nobody' | 'cc_cli' | 'sdk_owned' | 'closed'

function isPaginatablePhase(phase: string): phase is PaginatablePhase {
  return phase === 'nobody' || phase === 'cc_cli' || phase === 'sdk_owned' || phase === 'closed'
}

// ── Phase readiness check ────────────────────────────────────
// nobody needs sub.sub === 'ready' to have blocks.
// All other paginatable phases always have blocks.
function isReadyForPagination(panel: PanelState): boolean {
  if (panel.phase === 'nobody') return panel.sub.sub === 'ready'
  return true
}

// ── Prepend blocks to the phase-correct location ─────────────
function prependBlocks(panel: PanelState, older: ConversationBlock[]): PanelState {
  switch (panel.phase) {
    case 'nobody': {
      if (panel.sub.sub !== 'ready') return panel
      return { ...panel, sub: { sub: 'ready', blocks: [...older, ...panel.sub.blocks] } }
    }
    case 'cc_cli':
    case 'sdk_owned':
    case 'closed':
      return { ...panel, blocks: [...older, ...panel.blocks] }
    default:
      return panel
  }
}

// ── LOAD_OLDER_HISTORY handler (phase-independent) ───────────
export function handleLoadOlder(store: ChatPanelStore): TransitionResult {
  const { panel } = store

  if (!isPaginatablePhase(panel.phase)) return [store, []]
  if (!isReadyForPagination(panel)) return [store, []]

  const pg = store.historyPagination
  if (!pg || pg.offset <= 0 || pg.fetchingOlder) return [store, []]

  // All paginatable phases have sessionId — TS can't narrow through the type guard
  const sessionId = 'sessionId' in panel ? panel.sessionId : ''
  const newOffset = Math.max(0, pg.offset - BLOCK_PAGE_SIZE)
  const limit = pg.offset - newOffset
  return [
    { ...store, historyPagination: { ...pg, fetchingOlder: true } },
    [{ cmd: 'FETCH_OLDER_HISTORY', sessionId, offset: newOffset, limit }],
  ]
}

// ── OLDER_HISTORY_OK handler (phase-independent) ─────────────
export function handleOlderHistoryOk(
  store: ChatPanelStore,
  event: { type: 'OLDER_HISTORY_OK'; blocks: ConversationBlock[]; offset: number },
): TransitionResult {
  const { panel } = store

  const pg = store.historyPagination
  if (!pg || !pg.fetchingOlder) return [store, []]

  if (!isPaginatablePhase(panel.phase)) return [store, []]
  if (!isReadyForPagination(panel)) return [store, []]

  const newPanel = prependBlocks(panel, event.blocks)
  return [
    {
      ...store,
      panel: newPanel,
      historyPagination: { ...pg, offset: event.offset, fetchingOlder: false },
    },
    [],
  ]
}
