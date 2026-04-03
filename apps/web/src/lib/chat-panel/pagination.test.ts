import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { describe, expect, it } from 'vitest'
import { BLOCK_PAGE_SIZE } from '../block-pagination'
import { coordinate } from './coordinator'
import { deriveHistoryPagination } from './derive'
import type { ChatPanelStore, PanelState, RawEvent } from './types'

// ── Helpers ──────────────────────────────────────────────────

const emptyOutbox = { messages: [] }

function makeStore(panel: PanelState, overrides?: Partial<ChatPanelStore>): ChatPanelStore {
  return {
    panel,
    outbox: emptyOutbox,
    meta: null,
    projectPath: null,
    lastModel: null,
    lastPermissionMode: null,
    historyPagination: null,
    ...overrides,
  }
}

const fakeBlocks = (n: number): ConversationBlock[] =>
  Array.from({ length: n }, (_, i) => ({
    type: 'assistant',
    id: `msg-${i}`,
  })) as unknown as ConversationBlock[]

function step(store: ChatPanelStore, event: RawEvent): ChatPanelStore {
  const [next] = coordinate(store, event)
  return next
}

function stepWithCmds(store: ChatPanelStore, event: RawEvent) {
  return coordinate(store, event)
}

// ── deriveHistoryPagination ──────────────────────────────────

describe('deriveHistoryPagination', () => {
  it('returns hasOlderMessages: false when pagination is null', () => {
    const store = makeStore({ phase: 'empty' })
    expect(deriveHistoryPagination(store)).toEqual({
      hasOlderMessages: false,
      isFetchingOlder: false,
    })
  })

  it('returns hasOlderMessages: false when offset is 0', () => {
    const store = makeStore(
      { phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: [] } },
      { historyPagination: { total: 50, offset: 0, fetchingOlder: false } },
    )
    expect(deriveHistoryPagination(store)).toEqual({
      hasOlderMessages: false,
      isFetchingOlder: false,
    })
  })

  it('returns hasOlderMessages: true when offset > 0', () => {
    const store = makeStore(
      { phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: [] } },
      { historyPagination: { total: 126, offset: 76, fetchingOlder: false } },
    )
    expect(deriveHistoryPagination(store)).toEqual({
      hasOlderMessages: true,
      isFetchingOlder: false,
    })
  })

  it('returns isFetchingOlder: true when fetch in progress', () => {
    const store = makeStore(
      { phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: [] } },
      { historyPagination: { total: 126, offset: 76, fetchingOlder: true } },
    )
    expect(deriveHistoryPagination(store)).toEqual({
      hasOlderMessages: true,
      isFetchingOlder: true,
    })
  })
})

// ── HISTORY_OK populates pagination ──────────────────────────

describe('HISTORY_OK → historyPagination', () => {
  it('populates pagination from HISTORY_OK in nobody phase', () => {
    const store = makeStore({
      phase: 'nobody',
      sessionId: 's1',
      sub: { sub: 'loading' },
    })
    const next = step(store, {
      type: 'HISTORY_OK',
      blocks: fakeBlocks(50),
      total: 126,
      offset: 76,
    } as RawEvent)

    expect(next.historyPagination).toEqual({
      total: 126,
      offset: 76,
      fetchingOlder: false,
    })
  })

  it('populates pagination from HISTORY_OK in cc_cli phase (race)', () => {
    const store = makeStore({
      phase: 'cc_cli',
      sessionId: 's1',
      blocks: [],
      sub: { sub: 'watching' },
    } as PanelState)
    const next = step(store, {
      type: 'HISTORY_OK',
      blocks: fakeBlocks(50),
      total: 126,
      offset: 76,
    } as RawEvent)

    expect(next.historyPagination).toEqual({
      total: 126,
      offset: 76,
      fetchingOlder: false,
    })
  })

  it('leaves pagination null when HISTORY_OK lacks total/offset', () => {
    const store = makeStore({
      phase: 'nobody',
      sessionId: 's1',
      sub: { sub: 'loading' },
    })
    const next = step(store, {
      type: 'HISTORY_OK',
      blocks: fakeBlocks(10),
    } as RawEvent)

    expect(next.historyPagination).toBeNull()
  })
})

// ── LOAD_OLDER_HISTORY ───────────────────────────────────────

describe('LOAD_OLDER_HISTORY', () => {
  const readyStore = (offset: number, total = 126) =>
    makeStore(
      { phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: fakeBlocks(50) } },
      { historyPagination: { total, offset, fetchingOlder: false } },
    )

  it('emits FETCH_OLDER_HISTORY with correct offset and limit', () => {
    const [next, cmds] = stepWithCmds(readyStore(76), { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next.historyPagination?.fetchingOlder).toBe(true)
    expect(cmds).toHaveLength(1)
    expect(cmds[0]).toEqual({
      cmd: 'FETCH_OLDER_HISTORY',
      sessionId: 's1',
      offset: 26, // max(0, 76 - 50)
      limit: 50, // 76 - 26
    })
  })

  it('clamps offset to 0 for partial last page', () => {
    const [, cmds] = stepWithCmds(readyStore(30), { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(cmds[0]).toEqual({
      cmd: 'FETCH_OLDER_HISTORY',
      sessionId: 's1',
      offset: 0,
      limit: 30,
    })
  })

  it('no-ops when offset is 0 (all loaded)', () => {
    const store = readyStore(0)
    const [next, cmds] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next).toBe(store)
    expect(cmds).toHaveLength(0)
  })

  it('no-ops when already fetching', () => {
    const store = makeStore(
      { phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: fakeBlocks(50) } },
      { historyPagination: { total: 126, offset: 76, fetchingOlder: true } },
    )
    const [next, cmds] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next).toBe(store)
    expect(cmds).toHaveLength(0)
  })

  it('no-ops when pagination is null', () => {
    const store = makeStore({
      phase: 'nobody',
      sessionId: 's1',
      sub: { sub: 'ready', blocks: fakeBlocks(50) },
    })
    const [next, cmds] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next).toBe(store)
    expect(cmds).toHaveLength(0)
  })

  it('no-ops when sub is not ready', () => {
    const store = makeStore(
      { phase: 'nobody', sessionId: 's1', sub: { sub: 'loading' } },
      { historyPagination: { total: 126, offset: 76, fetchingOlder: false } },
    )
    const [next, cmds] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next).toBe(store)
    expect(cmds).toHaveLength(0)
  })

  it('works in cc_cli phase too', () => {
    const store = makeStore(
      {
        phase: 'cc_cli',
        sessionId: 's1',
        blocks: fakeBlocks(50),
        sub: { sub: 'watching' },
      } as PanelState,
      { historyPagination: { total: 126, offset: 76, fetchingOlder: false } },
    )
    const [next, cmds] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next.historyPagination?.fetchingOlder).toBe(true)
    expect(cmds).toHaveLength(1)
    expect(cmds[0]).toMatchObject({ cmd: 'FETCH_OLDER_HISTORY', offset: 26, limit: 50 })
  })
})

// ── OLDER_HISTORY_OK ─────────────────────────────────────────

describe('OLDER_HISTORY_OK', () => {
  it('prepends blocks and updates offset in nobody phase', () => {
    const existing = fakeBlocks(50)
    const older = fakeBlocks(50).map((b) => ({ ...b, id: `old-${b.id}` }))
    const store = makeStore(
      { phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: existing } },
      { historyPagination: { total: 126, offset: 76, fetchingOlder: true } },
    )

    const next = step(store, {
      type: 'OLDER_HISTORY_OK',
      blocks: older,
      offset: 26,
    } as RawEvent)

    // Blocks prepended
    expect(next.panel.phase).toBe('nobody')
    if (next.panel.phase === 'nobody' && next.panel.sub.sub === 'ready') {
      expect(next.panel.sub.blocks).toHaveLength(100)
      expect(next.panel.sub.blocks[0].id).toBe('old-msg-0')
    }

    // Pagination updated
    expect(next.historyPagination).toEqual({
      total: 126,
      offset: 26,
      fetchingOlder: false,
    })
  })

  it('prepends blocks and updates offset in cc_cli phase', () => {
    const existing = fakeBlocks(50)
    const older = fakeBlocks(50).map((b) => ({ ...b, id: `old-${b.id}` }))
    const store = makeStore(
      {
        phase: 'cc_cli',
        sessionId: 's1',
        blocks: existing,
        sub: { sub: 'watching' },
      } as PanelState,
      { historyPagination: { total: 126, offset: 76, fetchingOlder: true } },
    )

    const next = step(store, {
      type: 'OLDER_HISTORY_OK',
      blocks: older,
      offset: 26,
    } as RawEvent)

    if (next.panel.phase === 'cc_cli') {
      expect(next.panel.blocks).toHaveLength(100)
    }
    expect(next.historyPagination?.offset).toBe(26)
    expect(next.historyPagination?.fetchingOlder).toBe(false)
  })
})

// ── Full pagination chain ────────────────────────────────────

describe('full pagination chain: load all 126 blocks in 3 steps', () => {
  it('SELECT → HISTORY_OK → LOAD_OLDER → OK → LOAD_OLDER → OK → all loaded', () => {
    // Step 1: SELECT_SESSION
    let store = step(makeStore({ phase: 'empty' }), {
      type: 'SELECT_SESSION',
      sessionId: 's1',
    } as RawEvent)
    expect(store.panel.phase).toBe('nobody')
    expect(store.historyPagination).toBeNull()

    // Step 2: HISTORY_OK (initial 50 blocks, offset=76)
    store = step(store, {
      type: 'HISTORY_OK',
      blocks: fakeBlocks(50),
      total: 126,
      offset: 76,
    } as RawEvent)
    expect(deriveHistoryPagination(store).hasOlderMessages).toBe(true)

    // Step 3: LOAD_OLDER_HISTORY (76 → 26)
    const [s3, cmds3] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)
    expect(s3.historyPagination?.fetchingOlder).toBe(true)
    expect(cmds3[0]).toMatchObject({ offset: 26, limit: BLOCK_PAGE_SIZE })

    // Step 4: OLDER_HISTORY_OK (50 more blocks, offset=26)
    store = step(s3, {
      type: 'OLDER_HISTORY_OK',
      blocks: fakeBlocks(50),
      offset: 26,
    } as RawEvent)
    expect(deriveHistoryPagination(store).hasOlderMessages).toBe(true)

    // Step 5: LOAD_OLDER_HISTORY (26 → 0)
    const [s5, cmds5] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)
    expect(cmds5[0]).toMatchObject({ offset: 0, limit: 26 })

    // Step 6: OLDER_HISTORY_OK (last 26 blocks, offset=0)
    store = step(s5, {
      type: 'OLDER_HISTORY_OK',
      blocks: fakeBlocks(26),
      offset: 0,
    } as RawEvent)
    expect(deriveHistoryPagination(store).hasOlderMessages).toBe(false)
    expect(deriveHistoryPagination(store).isFetchingOlder).toBe(false)

    // Step 7: LOAD_OLDER_HISTORY → no-op (offset=0)
    const [final, finalCmds] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)
    expect(final).toBe(store)
    expect(finalCmds).toHaveLength(0)
  })
})

// ── Pagination preserved across phase transitions ────────────

describe('historyPagination preserved across phase transitions', () => {
  const paginatedStore = (phase: 'nobody' | 'cc_cli') => {
    const panel: PanelState =
      phase === 'nobody'
        ? { phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: fakeBlocks(50) } }
        : ({
            phase: 'cc_cli',
            sessionId: 's1',
            blocks: fakeBlocks(50),
            sub: { sub: 'watching' },
          } as PanelState)
    return makeStore(panel, {
      historyPagination: { total: 126, offset: 26, fetchingOlder: false },
    })
  }

  it('nobody → cc_cli preserves pagination', () => {
    const store = paginatedStore('nobody')
    const next = step(store, {
      type: 'LIVE_STATUS_CHANGED',
      status: 'cc_owned',
    } as RawEvent)

    expect(next.panel.phase).toBe('cc_cli')
    expect(next.historyPagination).toEqual({
      total: 126,
      offset: 26,
      fetchingOlder: false,
    })
  })

  it('cc_cli → nobody preserves pagination', () => {
    const store = paginatedStore('cc_cli')
    const next = step(store, {
      type: 'LIVE_STATUS_CHANGED',
      status: 'inactive',
    } as RawEvent)

    expect(next.panel.phase).toBe('nobody')
    expect(next.historyPagination).toEqual({
      total: 126,
      offset: 26,
      fetchingOlder: false,
    })
  })

  it('SELECT_SESSION resets pagination to null', () => {
    const store = paginatedStore('nobody')
    const next = step(store, {
      type: 'SELECT_SESSION',
      sessionId: 'different-session',
    } as RawEvent)

    expect(next.historyPagination).toBeNull()
  })
})

// ── LOAD_OLDER_HISTORY in sdk_owned phase ───────────────────

describe('LOAD_OLDER_HISTORY in sdk_owned phase', () => {
  const sdkOwnedStore = (offset: number, total = 126) =>
    makeStore(
      {
        phase: 'sdk_owned',
        sessionId: 's1',
        controlId: 'c1',
        blocks: fakeBlocks(50),
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'idle' },
        conn: { health: 'ok' },
      } as PanelState,
      { historyPagination: { total, offset, fetchingOlder: false } },
    )

  it('emits FETCH_OLDER_HISTORY with correct offset and limit', () => {
    const [next, cmds] = stepWithCmds(sdkOwnedStore(76), { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next.historyPagination?.fetchingOlder).toBe(true)
    expect(cmds).toHaveLength(1)
    expect(cmds[0]).toMatchObject({
      cmd: 'FETCH_OLDER_HISTORY',
      sessionId: 's1',
      offset: 26,
      limit: 50,
    })
  })

  it('no-ops when offset is 0', () => {
    const store = sdkOwnedStore(0)
    const [next, cmds] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next).toBe(store)
    expect(cmds).toHaveLength(0)
  })

  it('no-ops when already fetching', () => {
    const store = makeStore(
      {
        phase: 'sdk_owned',
        sessionId: 's1',
        controlId: 'c1',
        blocks: fakeBlocks(50),
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'idle' },
        conn: { health: 'ok' },
      } as PanelState,
      { historyPagination: { total: 126, offset: 76, fetchingOlder: true } },
    )
    const [next, cmds] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next).toBe(store)
    expect(cmds).toHaveLength(0)
  })

  it('no-ops when pagination is null', () => {
    const store = makeStore({
      phase: 'sdk_owned',
      sessionId: 's1',
      controlId: 'c1',
      blocks: fakeBlocks(50),
      pendingText: '',
      ephemeral: false,
      turn: { turn: 'idle' },
      conn: { health: 'ok' },
    } as PanelState)
    const [next, cmds] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next).toBe(store)
    expect(cmds).toHaveLength(0)
  })
})

// ── OLDER_HISTORY_OK in sdk_owned phase ─────────────────────

describe('OLDER_HISTORY_OK in sdk_owned phase', () => {
  it('prepends blocks and updates offset', () => {
    const existing = fakeBlocks(50)
    const older = fakeBlocks(50).map((b) => ({ ...b, id: `old-${b.id}` }))
    const store = makeStore(
      {
        phase: 'sdk_owned',
        sessionId: 's1',
        controlId: 'c1',
        blocks: existing,
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'idle' },
        conn: { health: 'ok' },
      } as PanelState,
      { historyPagination: { total: 126, offset: 76, fetchingOlder: true } },
    )

    const next = step(store, {
      type: 'OLDER_HISTORY_OK',
      blocks: older,
      offset: 26,
    } as RawEvent)

    if (next.panel.phase === 'sdk_owned') {
      expect(next.panel.blocks).toHaveLength(100)
      expect(next.panel.blocks[0].id).toBe('old-msg-0')
    }
    expect(next.historyPagination).toEqual({
      total: 126,
      offset: 26,
      fetchingOlder: false,
    })
  })

  it('no-ops when fetchingOlder is false (stale response)', () => {
    const store = makeStore(
      {
        phase: 'sdk_owned',
        sessionId: 's1',
        controlId: 'c1',
        blocks: fakeBlocks(50),
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'idle' },
        conn: { health: 'ok' },
      } as PanelState,
      { historyPagination: { total: 126, offset: 76, fetchingOlder: false } },
    )
    const [next, cmds] = stepWithCmds(store, {
      type: 'OLDER_HISTORY_OK',
      blocks: fakeBlocks(50),
      offset: 26,
    } as RawEvent)

    expect(next).toBe(store)
    expect(cmds).toHaveLength(0)
  })
})

// ── HISTORY_OK populates historyPagination in sdk_owned ─────

describe('HISTORY_OK populates historyPagination in sdk_owned phase', () => {
  it('sets pagination from HISTORY_OK when blocks are empty', () => {
    const store = makeStore({
      phase: 'sdk_owned',
      sessionId: 's1',
      controlId: 'c1',
      blocks: [],
      pendingText: '',
      ephemeral: false,
      turn: { turn: 'idle' },
      conn: { health: 'ok' },
    } as PanelState)

    const next = step(store, {
      type: 'HISTORY_OK',
      blocks: fakeBlocks(50),
      total: 126,
      offset: 76,
    } as RawEvent)

    expect(next.historyPagination).toEqual({
      total: 126,
      offset: 76,
      fetchingOlder: false,
    })
  })

  it('ignores pagination when blocks already exist', () => {
    const store = makeStore(
      {
        phase: 'sdk_owned',
        sessionId: 's1',
        controlId: 'c1',
        blocks: fakeBlocks(50),
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'idle' },
        conn: { health: 'ok' },
      } as PanelState,
      { historyPagination: { total: 50, offset: 0, fetchingOlder: false } },
    )

    const next = step(store, {
      type: 'HISTORY_OK',
      blocks: fakeBlocks(50),
      total: 126,
      offset: 76,
    } as RawEvent)

    // Blocks exist → HISTORY_OK ignored, pagination unchanged
    expect(next.historyPagination).toEqual({ total: 50, offset: 0, fetchingOlder: false })
  })
})

// ── LOAD_OLDER_HISTORY in closed phase ──────────────────────

describe('LOAD_OLDER_HISTORY in closed phase', () => {
  it('emits FETCH_OLDER_HISTORY in closed phase', () => {
    const store = makeStore(
      { phase: 'closed', sessionId: 's1', blocks: fakeBlocks(50), ephemeral: false } as PanelState,
      { historyPagination: { total: 126, offset: 76, fetchingOlder: false } },
    )
    const [next, cmds] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next.historyPagination?.fetchingOlder).toBe(true)
    expect(cmds).toHaveLength(1)
    expect(cmds[0]).toMatchObject({ cmd: 'FETCH_OLDER_HISTORY', sessionId: 's1' })
  })
})

// ── Pagination skipped during non-paginatable phases ────────

describe('pagination skipped during non-paginatable phases', () => {
  it('no-ops in acquiring phase', () => {
    const store = makeStore(
      {
        phase: 'acquiring',
        sessionId: 's1',
        targetSessionId: null,
        action: 'resume',
        historyBlocks: fakeBlocks(50),
        pendingMessage: null,
        step: { step: 'posting' },
      } as PanelState,
      { historyPagination: { total: 126, offset: 76, fetchingOlder: false } },
    )
    const [next, cmds] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next).toBe(store)
    expect(cmds).toHaveLength(0)
  })

  it('no-ops in nobody.loading sub-state', () => {
    const store = makeStore(
      { phase: 'nobody', sessionId: 's1', sub: { sub: 'loading' } },
      { historyPagination: { total: 126, offset: 76, fetchingOlder: false } },
    )
    const [next, cmds] = stepWithCmds(store, { type: 'LOAD_OLDER_HISTORY' } as RawEvent)

    expect(next).toBe(store)
    expect(cmds).toHaveLength(0)
  })
})
