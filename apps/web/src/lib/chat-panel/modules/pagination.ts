import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { BLOCK_PAGE_SIZE } from '../../block-pagination'
import { blockTimestamp } from '../hook-events'
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

// ── Merge older blocks into existing, interleaving misplaced hooks ──
// When FETCH_HOOK_EVENTS merges live-session hooks into existing blocks,
// hook events from early turns (low timestamps) land at the front of
// existing. On prepend, they'd sit between older and newer blocks
// instead of interleaved at their correct timestamp positions.
//
// Fix: extract hooks that belong in the older range, interleave them
// into older by timestamp, then concat with remaining existing.
// Non-timestamped blocks (TurnBoundary, Notice) are never moved.
function mergeOlderBlocks(
  older: ConversationBlock[],
  existing: ConversationBlock[],
): ConversationBlock[] {
  if (older.length === 0 || existing.length === 0) return [...older, ...existing]

  const lastOlderTs = blockTimestamp(older[older.length - 1])

  // Partition existing: hooks that predate the older range vs everything else
  const belongInOlder: ConversationBlock[] = []
  const stayInExisting: ConversationBlock[] = []
  for (const block of existing) {
    const ts = blockTimestamp(block)
    if (ts > 0 && ts < lastOlderTs) {
      belongInOlder.push(block)
    } else {
      stayInExisting.push(block)
    }
  }

  if (belongInOlder.length === 0) return [...older, ...stayInExisting]

  // Interleave hooks into older by timestamp.
  // Non-timestamped blocks (ts=0) in older keep their position.
  belongInOlder.sort((a, b) => blockTimestamp(a) - blockTimestamp(b))
  const merged: ConversationBlock[] = []
  let hi = 0
  for (const block of older) {
    const ts = blockTimestamp(block)
    if (ts > 0) {
      while (hi < belongInOlder.length && blockTimestamp(belongInOlder[hi]) <= ts) {
        merged.push(belongInOlder[hi++])
      }
    }
    merged.push(block)
  }
  while (hi < belongInOlder.length) merged.push(belongInOlder[hi++])

  return [...merged, ...stayInExisting]
}

// ── Prepend blocks to the phase-correct location ─────────────
function prependBlocks(panel: PanelState, older: ConversationBlock[]): PanelState {
  switch (panel.phase) {
    case 'nobody': {
      if (panel.sub.sub !== 'ready') return panel
      return {
        ...panel,
        sub: { sub: 'ready', blocks: mergeOlderBlocks(older, panel.sub.blocks) },
      }
    }
    case 'cc_cli':
    case 'sdk_owned':
    case 'closed':
      return { ...panel, blocks: mergeOlderBlocks(older, panel.blocks) }
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
