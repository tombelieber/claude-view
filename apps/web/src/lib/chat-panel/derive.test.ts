import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { describe, expect, test } from 'vitest'
import {
  deriveBlocks,
  deriveCanFork,
  deriveCanSend,
  deriveInputBar,
  deriveViewMode,
} from './derive'
import type { ChatPanelStore, PanelState } from './types'

const emptyOutbox = { messages: [] }
const mockBlocks: ConversationBlock[] = [
  { type: 'user', id: '1', text: 'hi', timestamp: 1 },
] as ConversationBlock[]

function makeStore(panel: PanelState, overrides?: Partial<ChatPanelStore>): ChatPanelStore {
  return { panel, outbox: emptyOutbox, meta: null, ...overrides }
}

// ─── deriveBlocks ──────────────────────────────────────────

describe('deriveBlocks', () => {
  test('empty → []', () => {
    expect(deriveBlocks(makeStore({ phase: 'empty' }))).toEqual([])
  })

  test('nobody.loading → []', () => {
    expect(
      deriveBlocks(makeStore({ phase: 'nobody', sessionId: 's1', sub: { sub: 'loading' } })),
    ).toEqual([])
  })

  test('nobody.ready → history blocks', () => {
    expect(
      deriveBlocks(
        makeStore({ phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: mockBlocks } }),
      ),
    ).toEqual(mockBlocks)
  })

  test('cc_cli → blocks (history preserved while watching)', () => {
    expect(
      deriveBlocks(
        makeStore({
          phase: 'cc_cli',
          sessionId: 's1',
          blocks: mockBlocks,
          sub: { sub: 'watching' },
        }),
      ),
    ).toEqual(mockBlocks)
  })

  test('acquiring → historyBlocks for display continuity', () => {
    const store = makeStore({
      phase: 'acquiring',
      sessionId: 's1',
      targetSessionId: null,
      action: 'resume' as const,
      historyBlocks: mockBlocks,
      pendingMessage: null,
      step: { step: 'posting' },
    })
    expect(deriveBlocks(store)).toEqual(mockBlocks)
  })

  test('sdk_owned → live blocks', () => {
    const store = makeStore({
      phase: 'sdk_owned',
      sessionId: 's1',
      controlId: 'c1',
      blocks: mockBlocks,
      pendingText: '',
      ephemeral: false,
      turn: { turn: 'idle' },
      conn: { health: 'ok' },
    })
    expect(deriveBlocks(store)).toEqual(mockBlocks)
  })

  test('sdk_owned with pendingText → appends synthetic assistant block', () => {
    const store = makeStore({
      phase: 'sdk_owned',
      sessionId: 's1',
      controlId: 'c1',
      blocks: mockBlocks,
      pendingText: 'thinking...',
      ephemeral: false,
      turn: { turn: 'streaming' },
      conn: { health: 'ok' },
    })
    const blocks = deriveBlocks(store)
    expect(blocks).toHaveLength(2)
    expect(blocks[1]).toMatchObject({ type: 'assistant' })
  })

  test('sdk_owned with empty pendingText → no synthetic block', () => {
    const store = makeStore({
      phase: 'sdk_owned',
      sessionId: 's1',
      controlId: 'c1',
      blocks: mockBlocks,
      pendingText: '',
      ephemeral: false,
      turn: { turn: 'streaming' },
      conn: { health: 'ok' },
    })
    expect(deriveBlocks(store)).toEqual(mockBlocks)
  })

  test('recovering → stored blocks', () => {
    const store = makeStore({
      phase: 'recovering',
      sessionId: 's1',
      blocks: mockBlocks,
      recovering: { kind: 'action_failed', error: 'err' },
    })
    expect(deriveBlocks(store)).toEqual(mockBlocks)
  })

  test('closed → stored blocks', () => {
    expect(
      deriveBlocks(
        makeStore({ phase: 'closed', sessionId: 's1', blocks: mockBlocks, ephemeral: false }),
      ),
    ).toEqual(mockBlocks)
  })

  // Outbox reconciliation
  test('outbox entries appended as synthetic user blocks', () => {
    const store = makeStore(
      { phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: [] } },
      { outbox: { messages: [{ localId: 'l1', text: 'hello', status: 'queued' as const }] } },
    )
    const blocks = deriveBlocks(store)
    expect(blocks).toHaveLength(1)
    expect(blocks[0]).toMatchObject({ type: 'user', text: 'hello' })
  })

  test('outbox entry matching existing block text is NOT appended (reconciled)', () => {
    const store = makeStore(
      { phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: mockBlocks } },
      { outbox: { messages: [{ localId: 'l1', text: 'hi', status: 'sent' as const }] } },
    )
    const blocks = deriveBlocks(store)
    // 'hi' matches mockBlocks[0].text, so outbox entry should NOT be appended
    expect(blocks).toHaveLength(1)
    expect(blocks[0]).toBe(mockBlocks[0])
  })

  test('outbox entry NOT matching existing block text IS appended', () => {
    const store = makeStore(
      { phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: mockBlocks } },
      { outbox: { messages: [{ localId: 'l1', text: 'new message', status: 'queued' as const }] } },
    )
    const blocks = deriveBlocks(store)
    expect(blocks).toHaveLength(2)
    expect(blocks[1]).toMatchObject({ type: 'user', text: 'new message' })
  })
})

// ─── deriveCanSend ─────────────────────────────────────────

describe('deriveCanSend', () => {
  test('empty → true', () => {
    expect(deriveCanSend(makeStore({ phase: 'empty' }))).toBe(true)
  })

  test('nobody.loading → false', () => {
    expect(
      deriveCanSend(makeStore({ phase: 'nobody', sessionId: 's1', sub: { sub: 'loading' } })),
    ).toBe(false)
  })

  test('nobody.ready → true', () => {
    expect(
      deriveCanSend(
        makeStore({ phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: [] } }),
      ),
    ).toBe(true)
  })

  test('cc_cli.watching → true', () => {
    expect(
      deriveCanSend(
        makeStore({ phase: 'cc_cli', sessionId: 's1', blocks: [], sub: { sub: 'watching' } }),
      ),
    ).toBe(true)
  })

  test('cc_cli.takeover_killing → false', () => {
    expect(
      deriveCanSend(
        makeStore({
          phase: 'cc_cli',
          sessionId: 's1',
          blocks: [],
          sub: { sub: 'takeover_killing' },
        }),
      ),
    ).toBe(false)
  })

  test('acquiring → false', () => {
    expect(
      deriveCanSend(
        makeStore({
          phase: 'acquiring',
          sessionId: 's1',
          targetSessionId: null,
          action: 'resume' as const,
          historyBlocks: [],
          pendingMessage: null,
          step: { step: 'posting' },
        }),
      ),
    ).toBe(false)
  })

  test('sdk_owned.idle → true', () => {
    expect(
      deriveCanSend(
        makeStore({
          phase: 'sdk_owned',
          sessionId: 's1',
          controlId: 'c1',
          blocks: [],
          pendingText: '',
          ephemeral: false,
          turn: { turn: 'idle' },
          conn: { health: 'ok' },
        }),
      ),
    ).toBe(true)
  })

  test('sdk_owned.streaming → false', () => {
    expect(
      deriveCanSend(
        makeStore({
          phase: 'sdk_owned',
          sessionId: 's1',
          controlId: 'c1',
          blocks: [],
          pendingText: '',
          ephemeral: false,
          turn: { turn: 'streaming' },
          conn: { health: 'ok' },
        }),
      ),
    ).toBe(false)
  })

  test('sdk_owned.awaiting → false', () => {
    expect(
      deriveCanSend(
        makeStore({
          phase: 'sdk_owned',
          sessionId: 's1',
          controlId: 'c1',
          blocks: [],
          pendingText: '',
          ephemeral: false,
          turn: { turn: 'awaiting', kind: 'permission', requestId: 'r1' },
          conn: { health: 'ok' },
        }),
      ),
    ).toBe(false)
  })

  test('sdk_owned.compacting → false', () => {
    expect(
      deriveCanSend(
        makeStore({
          phase: 'sdk_owned',
          sessionId: 's1',
          controlId: 'c1',
          blocks: [],
          pendingText: '',
          ephemeral: false,
          turn: { turn: 'compacting' },
          conn: { health: 'ok' },
        }),
      ),
    ).toBe(false)
  })

  test('recovering → true', () => {
    expect(
      deriveCanSend(
        makeStore({
          phase: 'recovering',
          sessionId: 's1',
          blocks: [],
          recovering: { kind: 'action_failed', error: 'err' },
        }),
      ),
    ).toBe(true)
  })

  test('closed(ephemeral: false) → true', () => {
    expect(
      deriveCanSend(makeStore({ phase: 'closed', sessionId: 's1', blocks: [], ephemeral: false })),
    ).toBe(true)
  })

  test('closed(ephemeral: true) → false', () => {
    expect(
      deriveCanSend(makeStore({ phase: 'closed', sessionId: 's1', blocks: [], ephemeral: true })),
    ).toBe(false)
  })
})

// ─── deriveCanFork ─────────────────────────────────────────

describe('deriveCanFork', () => {
  test('nobody.ready with blocks → true', () => {
    expect(
      deriveCanFork(
        makeStore({ phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: mockBlocks } }),
      ),
    ).toBe(true)
  })

  test('nobody.ready with empty blocks → false', () => {
    expect(
      deriveCanFork(
        makeStore({ phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: [] } }),
      ),
    ).toBe(false)
  })

  test('sdk_owned with blocks → true', () => {
    expect(
      deriveCanFork(
        makeStore({
          phase: 'sdk_owned',
          sessionId: 's1',
          controlId: 'c1',
          blocks: mockBlocks,
          pendingText: '',
          ephemeral: false,
          turn: { turn: 'idle' },
          conn: { health: 'ok' },
        }),
      ),
    ).toBe(true)
  })

  test('sdk_owned with empty blocks → false', () => {
    expect(
      deriveCanFork(
        makeStore({
          phase: 'sdk_owned',
          sessionId: 's1',
          controlId: 'c1',
          blocks: [],
          pendingText: '',
          ephemeral: false,
          turn: { turn: 'idle' },
          conn: { health: 'ok' },
        }),
      ),
    ).toBe(false)
  })

  test('empty → false', () => {
    expect(deriveCanFork(makeStore({ phase: 'empty' }))).toBe(false)
  })

  test('cc_cli → false', () => {
    expect(
      deriveCanFork(
        makeStore({ phase: 'cc_cli', sessionId: 's1', blocks: [], sub: { sub: 'watching' } }),
      ),
    ).toBe(false)
  })

  test('acquiring → false', () => {
    expect(
      deriveCanFork(
        makeStore({
          phase: 'acquiring',
          sessionId: 's1',
          targetSessionId: null,
          action: 'resume' as const,
          historyBlocks: mockBlocks,
          pendingMessage: null,
          step: { step: 'posting' },
        }),
      ),
    ).toBe(false)
  })

  test('recovering → false', () => {
    expect(
      deriveCanFork(
        makeStore({
          phase: 'recovering',
          sessionId: 's1',
          blocks: mockBlocks,
          recovering: { kind: 'action_failed', error: 'err' },
        }),
      ),
    ).toBe(false)
  })

  test('closed → false', () => {
    expect(
      deriveCanFork(
        makeStore({ phase: 'closed', sessionId: 's1', blocks: mockBlocks, ephemeral: false }),
      ),
    ).toBe(false)
  })
})

// ─── deriveInputBar ────────────────────────────────────────

describe('deriveInputBar', () => {
  test('empty → dormant', () => {
    expect(deriveInputBar(makeStore({ phase: 'empty' }))).toBe('dormant')
  })

  test('nobody.loading → active', () => {
    expect(
      deriveInputBar(makeStore({ phase: 'nobody', sessionId: 's1', sub: { sub: 'loading' } })),
    ).toBe('active')
  })

  test('nobody.ready → active', () => {
    expect(
      deriveInputBar(
        makeStore({ phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: [] } }),
      ),
    ).toBe('active')
  })

  test('cc_cli → controlled_elsewhere', () => {
    expect(
      deriveInputBar(
        makeStore({ phase: 'cc_cli', sessionId: 's1', blocks: [], sub: { sub: 'watching' } }),
      ),
    ).toBe('controlled_elsewhere')
  })

  test('acquiring → connecting', () => {
    expect(
      deriveInputBar(
        makeStore({
          phase: 'acquiring',
          sessionId: 's1',
          targetSessionId: null,
          action: 'resume' as const,
          historyBlocks: [],
          pendingMessage: null,
          step: { step: 'posting' },
        }),
      ),
    ).toBe('connecting')
  })

  test('sdk_owned.idle → active', () => {
    expect(
      deriveInputBar(
        makeStore({
          phase: 'sdk_owned',
          sessionId: 's1',
          controlId: 'c1',
          blocks: [],
          pendingText: '',
          ephemeral: false,
          turn: { turn: 'idle' },
          conn: { health: 'ok' },
        }),
      ),
    ).toBe('active')
  })

  test('sdk_owned.streaming → streaming', () => {
    expect(
      deriveInputBar(
        makeStore({
          phase: 'sdk_owned',
          sessionId: 's1',
          controlId: 'c1',
          blocks: [],
          pendingText: '',
          ephemeral: false,
          turn: { turn: 'streaming' },
          conn: { health: 'ok' },
        }),
      ),
    ).toBe('streaming')
  })

  test('sdk_owned.awaiting → waiting_permission', () => {
    expect(
      deriveInputBar(
        makeStore({
          phase: 'sdk_owned',
          sessionId: 's1',
          controlId: 'c1',
          blocks: [],
          pendingText: '',
          ephemeral: false,
          turn: { turn: 'awaiting', kind: 'permission', requestId: 'r1' },
          conn: { health: 'ok' },
        }),
      ),
    ).toBe('waiting_permission')
  })

  test('sdk_owned.compacting → streaming', () => {
    expect(
      deriveInputBar(
        makeStore({
          phase: 'sdk_owned',
          sessionId: 's1',
          controlId: 'c1',
          blocks: [],
          pendingText: '',
          ephemeral: false,
          turn: { turn: 'compacting' },
          conn: { health: 'ok' },
        }),
      ),
    ).toBe('streaming')
  })

  test('sdk_owned reconnecting → reconnecting', () => {
    expect(
      deriveInputBar(
        makeStore({
          phase: 'sdk_owned',
          sessionId: 's1',
          controlId: 'c1',
          blocks: [],
          pendingText: '',
          ephemeral: false,
          turn: { turn: 'idle' },
          conn: { health: 'reconnecting', attempt: 1 },
        }),
      ),
    ).toBe('reconnecting')
  })

  test('recovering → active', () => {
    expect(
      deriveInputBar(
        makeStore({
          phase: 'recovering',
          sessionId: 's1',
          blocks: [],
          recovering: { kind: 'action_failed', error: 'err' },
        }),
      ),
    ).toBe('active')
  })

  test('closed → completed', () => {
    expect(
      deriveInputBar(makeStore({ phase: 'closed', sessionId: 's1', blocks: [], ephemeral: false })),
    ).toBe('completed')
  })
})

// ─── deriveViewMode ────────────────────────────────────────

describe('deriveViewMode', () => {
  test('empty → blank', () => {
    expect(deriveViewMode(makeStore({ phase: 'empty' }))).toBe('blank')
  })

  test('nobody.loading → loading', () => {
    expect(
      deriveViewMode(makeStore({ phase: 'nobody', sessionId: 's1', sub: { sub: 'loading' } })),
    ).toBe('loading')
  })

  test('nobody.ready → history', () => {
    expect(
      deriveViewMode(
        makeStore({ phase: 'nobody', sessionId: 's1', sub: { sub: 'ready', blocks: [] } }),
      ),
    ).toBe('history')
  })

  test('cc_cli → watching', () => {
    expect(
      deriveViewMode(
        makeStore({ phase: 'cc_cli', sessionId: 's1', blocks: [], sub: { sub: 'watching' } }),
      ),
    ).toBe('watching')
  })

  test('acquiring → connecting', () => {
    expect(
      deriveViewMode(
        makeStore({
          phase: 'acquiring',
          sessionId: 's1',
          targetSessionId: null,
          action: 'resume' as const,
          historyBlocks: [],
          pendingMessage: null,
          step: { step: 'posting' },
        }),
      ),
    ).toBe('connecting')
  })

  test('sdk_owned → active', () => {
    expect(
      deriveViewMode(
        makeStore({
          phase: 'sdk_owned',
          sessionId: 's1',
          controlId: 'c1',
          blocks: [],
          pendingText: '',
          ephemeral: false,
          turn: { turn: 'idle' },
          conn: { health: 'ok' },
        }),
      ),
    ).toBe('active')
  })

  test('recovering → error', () => {
    expect(
      deriveViewMode(
        makeStore({
          phase: 'recovering',
          sessionId: 's1',
          blocks: [],
          recovering: { kind: 'action_failed', error: 'err' },
        }),
      ),
    ).toBe('error')
  })

  test('closed → closed', () => {
    expect(
      deriveViewMode(makeStore({ phase: 'closed', sessionId: 's1', blocks: [], ephemeral: false })),
    ).toBe('closed')
  })
})
