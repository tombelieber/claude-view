import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { describe, expect, test } from 'vitest'
import { coordinate } from './coordinator'
import type { ChatPanelStore, Command, RawEvent } from './types'

const INITIAL: ChatPanelStore = {
  panel: { phase: 'empty' },
  outbox: { messages: [] },
  meta: null,
  projectPath: null,
}

const mockBlocks: ConversationBlock[] = [{ type: 'user', id: '1', text: 'hi', timestamp: 1 }]
const mockInitEvent: RawEvent = {
  type: 'SESSION_INIT',
  model: 'opus',
  permissionMode: 'default',
  slashCommands: [],
  mcpServers: [],
  skills: [],
  agents: [],
  capabilities: [],
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

describe('coordinator', () => {
  // Scenario 1: SELECT → history loads → nobody.ready
  test('SELECT_SESSION → nobody.loading → HISTORY_OK → nobody.ready', () => {
    const { store, allCmds } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'SIDECAR_NO_SESSION' },
      { type: 'HISTORY_OK', blocks: mockBlocks },
    ])
    expect(store.panel.phase).toBe('nobody')
    if (store.panel.phase === 'nobody') {
      expect(store.panel.sub).toEqual({ sub: 'ready', blocks: mockBlocks })
    }
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'FETCH_HISTORY', sessionId: 'abc' }),
    )
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'CHECK_SIDECAR_ACTIVE', sessionId: 'abc' }),
    )
  })

  // Scenario 2: Send from history → acquiring → sdk_owned
  test('nobody.ready + SEND_MESSAGE → acquiring', () => {
    const historyStore: ChatPanelStore = {
      panel: { phase: 'nobody', sessionId: 'abc', sub: { sub: 'ready', blocks: mockBlocks } },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
    }
    const { store, allCmds } = drive(historyStore, [
      { type: 'SEND_MESSAGE', text: 'hello', localId: 'l1' },
    ])
    expect(store.panel.phase).toBe('acquiring')
    expect(store.outbox.messages).toHaveLength(1)
    expect(store.outbox.messages[0]).toMatchObject({
      localId: 'l1',
      text: 'hello',
      status: 'sent',
    })
    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'POST_RESUME' }))
  })

  // Scenario 3: Full acquire lifecycle → sdk_owned with outbox drain
  test('acquiring → ACQUIRE_OK → WS_OPEN → SESSION_INIT → sdk_owned + outbox drained', () => {
    const acquiringStore: ChatPanelStore = {
      panel: {
        phase: 'acquiring',
        sessionId: 'abc',
        targetSessionId: null,
        action: 'resume' as const,
        historyBlocks: mockBlocks,
        pendingMessage: 'hello',
        step: { step: 'posting' },
      },
      outbox: { messages: [{ localId: 'l1', text: 'hello', status: 'queued' as const }] },
      meta: null,
      projectPath: null,
    }
    const { store, allCmds } = drive(acquiringStore, [
      { type: 'ACQUIRE_OK', controlId: 'c1' },
      { type: 'WS_OPEN' },
      mockInitEvent,
    ])
    expect(store.panel.phase).toBe('sdk_owned')
    // Outbox drained: queued → sent
    expect(store.outbox.messages[0]?.status).toBe('sent')
    // WS_SEND command issued for the queued message
    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'WS_SEND' }))
    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'OPEN_SIDECAR_WS' }))
  })

  // Scenario 4: SEND in sdk_owned.idle → WS_SEND (no new query/resume)
  test('sdk_owned.idle + SEND_MESSAGE → WS_SEND only', () => {
    const liveStore: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'abc',
        controlId: 'c1',
        blocks: mockBlocks,
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'idle' },
        conn: { health: 'ok' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
    }
    const [store, cmds] = coordinate(liveStore, {
      type: 'SEND_MESSAGE',
      text: 'world',
      localId: 'l2',
    })
    expect(store.panel.phase).toBe('sdk_owned')
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'WS_SEND' }))
    // Should NOT issue POST_RESUME
    expect(cmds).not.toContainEqual(expect.objectContaining({ cmd: 'POST_RESUME' }))
  })

  // Scenario 5: SSE race — LIVE_STATUS_CHANGED during acquiring is IGNORED
  test('acquiring + LIVE_STATUS_CHANGED → stays acquiring (SSE race rejection)', () => {
    const acquiringStore: ChatPanelStore = {
      panel: {
        phase: 'acquiring',
        sessionId: 'abc',
        targetSessionId: null,
        action: 'resume' as const,
        historyBlocks: [],
        pendingMessage: null,
        step: { step: 'posting' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
    }
    const [store] = coordinate(acquiringStore, {
      type: 'LIVE_STATUS_CHANGED',
      status: 'cc_owned',
    })
    expect(store.panel.phase).toBe('acquiring')
  })

  // Scenario 6: ACQUIRE_FAILED → recovering
  test('acquiring + ACQUIRE_FAILED → recovering', () => {
    const acquiringStore: ChatPanelStore = {
      panel: {
        phase: 'acquiring',
        sessionId: 'abc',
        targetSessionId: null,
        action: 'resume' as const,
        historyBlocks: mockBlocks,
        pendingMessage: null,
        step: { step: 'posting' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
    }
    const [store, cmds] = coordinate(acquiringStore, {
      type: 'ACQUIRE_FAILED',
      error: 'timeout',
    })
    expect(store.panel.phase).toBe('recovering')
    if (store.panel.phase === 'recovering') {
      expect(store.panel.recovering).toEqual({ kind: 'action_failed', error: 'timeout' })
    }
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'TOAST', variant: 'error' }))
  })

  // Scenario 7: CC CLI watching + takeover flow
  test('cc_cli.watching + TAKEOVER_CLI → killing → KILL_CLI_OK → acquiring', () => {
    const watchingStore: ChatPanelStore = {
      panel: { phase: 'cc_cli', sessionId: 'abc', blocks: [], sub: { sub: 'watching' } },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
    }
    // Takeover
    const [s1, c1] = coordinate(watchingStore, { type: 'TAKEOVER_CLI' })
    expect(s1.panel.phase).toBe('cc_cli')
    if (s1.panel.phase === 'cc_cli') {
      expect(s1.panel.sub).toEqual({ sub: 'takeover_killing' })
    }
    expect(c1).toContainEqual(expect.objectContaining({ cmd: 'KILL_CLI_SESSION' }))

    // Kill succeeds
    const [s2, c2] = coordinate(s1, { type: 'KILL_CLI_OK' })
    expect(s2.panel.phase).toBe('acquiring')
    expect(c2).toContainEqual(expect.objectContaining({ cmd: 'POST_RESUME' }))
  })

  // Scenario 8: Create from empty
  test('empty + SEND_MESSAGE → acquiring{create}', () => {
    const [store, cmds] = coordinate(INITIAL, {
      type: 'SEND_MESSAGE',
      text: 'start',
      localId: 'l3',
    })
    expect(store.panel.phase).toBe('acquiring')
    if (store.panel.phase === 'acquiring') {
      expect(store.panel.action).toBe('create')
    }
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'POST_CREATE' }))
  })

  // Scenario 9: Fork from history
  test('nobody.ready + FORK_SESSION → acquiring{fork}', () => {
    const historyStore: ChatPanelStore = {
      panel: { phase: 'nobody', sessionId: 'abc', sub: { sub: 'ready', blocks: mockBlocks } },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
    }
    const [store, cmds] = coordinate(historyStore, { type: 'FORK_SESSION' })
    expect(store.panel.phase).toBe('acquiring')
    if (store.panel.phase === 'acquiring') {
      expect(store.panel.action).toBe('fork')
    }
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'POST_FORK' }))
  })

  // Scenario 10: WS reconnect in sdk_owned
  test('sdk_owned + WS_CLOSE(recoverable) → conn reconnecting', () => {
    const liveStore: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'abc',
        controlId: 'c1',
        blocks: mockBlocks,
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'idle' },
        conn: { health: 'ok' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
    }
    const [store, cmds] = coordinate(liveStore, {
      type: 'WS_CLOSE',
      code: 1006,
      recoverable: true,
    })
    expect(store.panel.phase).toBe('sdk_owned')
    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.conn).toEqual({ health: 'reconnecting', attempt: 1 })
    }
    // Should schedule reconnect attempt
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'START_TIMER' }))
  })

  // Scenario 11: DESELECT clears everything
  test('DESELECT → empty', () => {
    const historyStore: ChatPanelStore = {
      panel: { phase: 'nobody', sessionId: 'abc', sub: { sub: 'ready', blocks: mockBlocks } },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
    }
    const [store] = coordinate(historyStore, { type: 'DESELECT' })
    expect(store.panel.phase).toBe('empty')
  })

  // Scenario 12: SELECT with sidecar active → auto-connect
  test('SELECT + SIDECAR_HAS_SESSION → acquiring.ws_connecting', () => {
    const { store } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'SIDECAR_HAS_SESSION', controlId: 'c1' },
    ])
    expect(store.panel.phase).toBe('acquiring')
    if (store.panel.phase === 'acquiring') {
      expect(store.panel.step).toEqual({ step: 'ws_connecting', controlId: 'c1' })
    }
  })

  // Scenario 13: Turn lifecycle in sdk_owned
  test('sdk_owned: STREAM_DELTA → streaming, TURN_COMPLETE → idle', () => {
    const liveStore: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'abc',
        controlId: 'c1',
        blocks: [],
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'idle' },
        conn: { health: 'ok' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
    }
    const [s1] = coordinate(liveStore, { type: 'STREAM_DELTA', text: 'hello' })
    if (s1.panel.phase === 'sdk_owned') {
      expect(s1.panel.turn).toEqual({ turn: 'streaming' })
      expect(s1.panel.pendingText).toBe('hello')
    }
    const [s2] = coordinate(s1, {
      type: 'TURN_COMPLETE',
      blocks: mockBlocks,
      totalInputTokens: 500,
      contextWindowSize: 200000,
    })
    if (s2.panel.phase === 'sdk_owned') {
      expect(s2.panel.turn).toEqual({ turn: 'idle' })
      expect(s2.panel.blocks).toEqual(mockBlocks)
      expect(s2.panel.pendingText).toBe('')
    }
  })

  // Scenario 14: SESSION_CLOSED in sdk_owned → closed
  test('sdk_owned + SESSION_CLOSED → closed', () => {
    const liveStore: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'abc',
        controlId: 'c1',
        blocks: mockBlocks,
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'idle' },
        conn: { health: 'ok' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
    }
    const [store, cmds] = coordinate(liveStore, { type: 'SESSION_CLOSED' })
    expect(store.panel.phase).toBe('closed')
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'CLOSE_SIDECAR_WS' }))
  })

  // Scenario 15: INTERRUPT in sdk_owned
  test('sdk_owned + INTERRUPT → WS_SEND interrupt', () => {
    const liveStore: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'abc',
        controlId: 'c1',
        blocks: [],
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'streaming' },
        conn: { health: 'ok' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
    }
    const [, cmds] = coordinate(liveStore, { type: 'INTERRUPT' })
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'WS_SEND' }))
  })

  // Scenario 16: LIVE_STATUS cc_cli → nobody transitions to cc_cli
  test('nobody + LIVE_STATUS cc_owned → cc_cli.watching', () => {
    const historyStore: ChatPanelStore = {
      panel: { phase: 'nobody', sessionId: 'abc', sub: { sub: 'ready', blocks: mockBlocks } },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
    }
    const [store, cmds] = coordinate(historyStore, {
      type: 'LIVE_STATUS_CHANGED',
      status: 'cc_owned',
    })
    expect(store.panel.phase).toBe('cc_cli')
    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.sub).toEqual({ sub: 'watching' })
    }
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'OPEN_TERMINAL_WS' }))
  })
})
