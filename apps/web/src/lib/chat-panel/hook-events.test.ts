import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { describe, expect, test } from 'vitest'
import { coordinate } from './coordinator'
import { deriveBlocks } from './derive'
import { blockTimestamp } from './hook-events'
import type { ChatPanelStore, Command, RawEvent } from './types'

const INITIAL: ChatPanelStore = {
  panel: { phase: 'empty' },
  outbox: { messages: [] },
  meta: null,
  projectPath: null,
  lastModel: null,
  lastPermissionMode: null,
  historyPagination: null,
}

const mockBlocks: ConversationBlock[] = [{ type: 'user', id: 'u1', text: 'hi', timestamp: 100 }]

// biome-ignore lint/suspicious/noExplicitAny: test fixture — ProgressBlock shapes are verbose
const mockHookBlock: any = {
  type: 'progress',
  id: 'hook-200-0',
  variant: 'hook',
  category: 'hook',
  data: {
    type: 'hook',
    hookEvent: 'PreToolUse',
    hookName: 'Bash',
    command: '',
    statusMessage: 'Running: git status',
  },
  ts: 200,
}

// biome-ignore lint/suspicious/noExplicitAny: test fixture
const mockHookBlock2: any = {
  type: 'progress',
  id: 'hook-300-0',
  variant: 'hook',
  category: 'hook',
  data: {
    type: 'hook',
    hookEvent: 'PostToolUse',
    hookName: 'Bash',
    command: '',
    statusMessage: 'Completed',
  },
  ts: 300,
}

function step(store: ChatPanelStore, event: RawEvent): { store: ChatPanelStore; cmds: Command[] } {
  const [newStore, cmds] = coordinate(store, event)
  return { store: newStore, cmds }
}

function drive(
  initial: ChatPanelStore,
  events: RawEvent[],
): { store: ChatPanelStore; allCmds: Command[] } {
  let current = initial
  const allCmds: Command[] = []
  for (const event of events) {
    const [newStore, cmds] = coordinate(current, event)
    current = newStore
    allCmds.push(...cmds)
  }
  return { store: current, allCmds }
}

// ═════════════════════════════════════════════════════════════════
// blockTimestamp utility
// ═════════════════════════════════════════════════════════════════

describe('blockTimestamp', () => {
  test('extracts ts from ProgressBlock', () => {
    expect(blockTimestamp(mockHookBlock)).toBe(200)
  })

  test('extracts timestamp from UserBlock', () => {
    expect(blockTimestamp(mockBlocks[0])).toBe(100)
  })

  test('returns 0 for blocks without timestamp', () => {
    // biome-ignore lint/suspicious/noExplicitAny: test fixture
    expect(blockTimestamp({ type: 'user', id: 'x', text: 'y' } as any)).toBe(0)
  })
})

// ═════════════════════════════════════════════════════════════════
// SELECT_SESSION emits FETCH_HOOK_EVENTS
// ═════════════════════════════════════════════════════════════════

describe('SELECT_SESSION emits FETCH_HOOK_EVENTS', () => {
  test('FETCH_HOOK_EVENTS is emitted alongside FETCH_HISTORY', () => {
    const { allCmds } = drive(INITIAL, [{ type: 'SELECT_SESSION', sessionId: 'abc' }])
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'FETCH_HISTORY', sessionId: 'abc' }),
    )
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'FETCH_HOOK_EVENTS', sessionId: 'abc' }),
    )
  })
})

// ═════════════════════════════════════════════════════════════════
// nobody phase: HOOK_EVENTS_OK
// ═════════════════════════════════════════════════════════════════

describe('nobody: HOOK_EVENTS_OK', () => {
  test('merges hook blocks into history blocks sorted by timestamp', () => {
    const { store } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'HISTORY_OK', blocks: mockBlocks },
      { type: 'HOOK_EVENTS_OK', blocks: [mockHookBlock] },
    ])

    expect(store.panel.phase).toBe('nobody')
    if (store.panel.phase === 'nobody' && store.panel.sub.sub === 'ready') {
      expect(store.panel.sub.blocks).toHaveLength(2)
      // User block (ts=100) before hook block (ts=200)
      expect(store.panel.sub.blocks[0].id).toBe('u1')
      expect(store.panel.sub.blocks[1].id).toBe('hook-200-0')
    }
  })

  test('deduplicates by ID — no doubles on re-fetch', () => {
    const { store } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'HISTORY_OK', blocks: mockBlocks },
      { type: 'HOOK_EVENTS_OK', blocks: [mockHookBlock] },
      // Same hook block dispatched again
      { type: 'HOOK_EVENTS_OK', blocks: [mockHookBlock] },
    ])

    if (store.panel.phase === 'nobody' && store.panel.sub.sub === 'ready') {
      expect(store.panel.sub.blocks).toHaveLength(2) // not 3
    }
  })

  test('discarded when history not yet loaded (sub=loading)', () => {
    const { store } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      // HOOK_EVENTS_OK arrives before HISTORY_OK
      { type: 'HOOK_EVENTS_OK', blocks: [mockHookBlock] },
    ])

    expect(store.panel.phase).toBe('nobody')
    if (store.panel.phase === 'nobody') {
      expect(store.panel.sub.sub).toBe('loading')
    }
  })

  test('no-op when zero hook events', () => {
    const { store } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'HISTORY_OK', blocks: mockBlocks },
      { type: 'HOOK_EVENTS_OK', blocks: [] },
    ])

    if (store.panel.phase === 'nobody' && store.panel.sub.sub === 'ready') {
      expect(store.panel.sub.blocks).toHaveLength(1) // unchanged
    }
  })

  test('hook blocks visible in deriveBlocks()', () => {
    const { store } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'HISTORY_OK', blocks: mockBlocks },
      { type: 'HOOK_EVENTS_OK', blocks: [mockHookBlock] },
    ])

    const blocks = deriveBlocks(store)
    expect(blocks).toHaveLength(2)
    expect(blocks[1].id).toBe('hook-200-0')
    expect(blocks[1].type).toBe('progress')
  })
})

// ═════════════════════════════════════════════════════════════════
// sdk_owned phase: HOOK_EVENTS_OK + TURN_COMPLETE triggers re-fetch
// ═════════════════════════════════════════════════════════════════

describe('sdk_owned: HOOK_EVENTS_OK', () => {
  const sdkOwnedStore: ChatPanelStore = {
    panel: {
      phase: 'sdk_owned',
      sessionId: 'abc',
      controlId: 'ctrl-1',
      blocks: [...mockBlocks],
      pendingText: '',
      ephemeral: false,
      turn: { turn: 'idle' },
      conn: { health: 'ok' },
    },
    outbox: { messages: [] },
    meta: null,
    projectPath: null,
    lastModel: null,
    lastPermissionMode: null,
    historyPagination: null,
  }

  test('merges hook blocks into sdk_owned blocks sorted by timestamp', () => {
    const { store } = step(sdkOwnedStore, {
      type: 'HOOK_EVENTS_OK',
      blocks: [mockHookBlock],
    })

    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.blocks).toHaveLength(2)
      expect(store.panel.blocks[0].id).toBe('u1')
      expect(store.panel.blocks[1].id).toBe('hook-200-0')
    }
  })

  test('deduplicates by ID — no doubles on re-fetch', () => {
    // First merge
    const { store: s1 } = step(sdkOwnedStore, {
      type: 'HOOK_EVENTS_OK',
      blocks: [mockHookBlock],
    })
    // Second merge with same + new hook
    const { store: s2 } = step(s1, {
      type: 'HOOK_EVENTS_OK',
      blocks: [mockHookBlock, mockHookBlock2],
    })

    if (s2.panel.phase === 'sdk_owned') {
      expect(s2.panel.blocks).toHaveLength(3) // u1 + hook-200-0 + hook-300-0
    }
  })

  test('TURN_COMPLETE emits FETCH_HOOK_EVENTS', () => {
    const streamingStore: ChatPanelStore = {
      ...sdkOwnedStore,
      panel: {
        ...(sdkOwnedStore.panel as Extract<typeof sdkOwnedStore.panel, { phase: 'sdk_owned' }>),
        turn: { turn: 'streaming' },
        pendingText: 'thinking...',
      },
    }

    const { cmds } = step(streamingStore, {
      type: 'TURN_COMPLETE',
      blocks: mockBlocks,
      totalInputTokens: 100,
      contextWindowSize: 200000,
    })

    expect(cmds).toContainEqual(
      expect.objectContaining({ cmd: 'FETCH_HOOK_EVENTS', sessionId: 'abc' }),
    )
  })

  test('TURN_ERROR emits FETCH_HOOK_EVENTS', () => {
    const streamingStore: ChatPanelStore = {
      ...sdkOwnedStore,
      panel: {
        ...(sdkOwnedStore.panel as Extract<typeof sdkOwnedStore.panel, { phase: 'sdk_owned' }>),
        turn: { turn: 'streaming' },
        pendingText: 'thinking...',
      },
    }

    const { cmds } = step(streamingStore, {
      type: 'TURN_ERROR',
      blocks: mockBlocks,
      totalInputTokens: 100,
      contextWindowSize: 200000,
    })

    expect(cmds).toContainEqual(
      expect.objectContaining({ cmd: 'FETCH_HOOK_EVENTS', sessionId: 'abc' }),
    )
  })
})

// ═════════════════════════════════════════════════════════════════
// cc_cli phase: HOOK_EVENTS_OK is no-op
// ═════════════════════════════════════════════════════════════════

describe('cc_cli: HOOK_EVENTS_OK is no-op', () => {
  test('hooks already flow via TERMINAL_BLOCK — HOOK_EVENTS_OK ignored', () => {
    const ccCliStore: ChatPanelStore = {
      panel: {
        phase: 'cc_cli',
        sessionId: 'abc',
        blocks: [...mockBlocks],
        sub: { sub: 'watching' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const { store } = step(ccCliStore, {
      type: 'HOOK_EVENTS_OK',
      blocks: [mockHookBlock],
    })

    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.blocks).toHaveLength(1) // unchanged
      expect(store.panel.blocks[0].id).toBe('u1')
    }
  })
})

// ═════════════════════════════════════════════════════════════════
// Integration: full lifecycle with hook events
// ═════════════════════════════════════════════════════════════════

describe('integration: hook events across phase transitions', () => {
  test('select → history → hooks → nobody shows merged blocks', () => {
    const { store } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'SIDECAR_NO_SESSION' },
      { type: 'HISTORY_OK', blocks: mockBlocks },
      { type: 'HOOK_EVENTS_OK', blocks: [mockHookBlock, mockHookBlock2] },
    ])

    const blocks = deriveBlocks(store)
    expect(blocks).toHaveLength(3)
    // Sorted by timestamp: u1(100) → hook(200) → hook(300)
    expect(blocks[0].id).toBe('u1')
    expect(blocks[1].id).toBe('hook-200-0')
    expect(blocks[2].id).toBe('hook-300-0')
  })

  test('hooks survive nobody → cc_cli transition (carried in blocks)', () => {
    const { store } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'HISTORY_OK', blocks: mockBlocks },
      { type: 'HOOK_EVENTS_OK', blocks: [mockHookBlock] },
      // Go live → cc_cli
      { type: 'LIVE_STATUS_CHANGED', status: 'cc_owned' },
    ])

    expect(store.panel.phase).toBe('cc_cli')
    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.blocks).toHaveLength(2)
      expect(store.panel.blocks[1].id).toBe('hook-200-0')
    }
  })
})
