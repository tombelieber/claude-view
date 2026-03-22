import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { describe, expect, test } from 'vitest'
import { coordinate } from './coordinator'
import type { ChatPanelStore, Command, RawEvent } from './types'

const INITIAL: ChatPanelStore = {
  panel: { phase: 'empty' },
  outbox: { messages: [] },
  meta: null,
  projectPath: null,
  lastModel: null,
  lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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
      lastModel: null,
      lastPermissionMode: null,
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

  // Scenario 16b: LIVE_STATUS cc_owned arrives BEFORE HISTORY_OK (race condition)
  // Must defer cc_cli transition until history loads to avoid blank page.
  test('nobody(loading) + LIVE_STATUS cc_owned → deferred, then HISTORY_OK → cc_cli with blocks', () => {
    const loadingStore: ChatPanelStore = {
      panel: { phase: 'nobody', sessionId: 'abc', sub: { sub: 'loading' } },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
    }
    // Step 1: LIVE_STATUS arrives while loading — should NOT transition to cc_cli
    const [midStore, midCmds] = coordinate(loadingStore, {
      type: 'LIVE_STATUS_CHANGED',
      status: 'cc_owned',
    })
    expect(midStore.panel.phase).toBe('nobody') // still nobody!
    if (midStore.panel.phase === 'nobody') {
      expect(midStore.panel.sub.sub).toBe('loading')
      if (midStore.panel.sub.sub === 'loading') {
        expect(midStore.panel.sub.pendingLive).toBe('cc_owned')
      }
    }
    expect(midCmds).toHaveLength(0) // no OPEN_TERMINAL_WS yet

    // Step 2: HISTORY_OK arrives — should complete the deferred cc_cli transition
    const [store, cmds] = coordinate(midStore, {
      type: 'HISTORY_OK',
      blocks: mockBlocks,
    })
    expect(store.panel.phase).toBe('cc_cli')
    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.blocks).toEqual(mockBlocks) // blocks carried!
      expect(store.panel.sub).toEqual({ sub: 'watching' })
    }
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'OPEN_TERMINAL_WS' }))
  })

  // ── Gap fixes ──────────────────────────────────────────────────

  // Gap 4+9: resumeAtMessageId removed from PanelState.acquiring
  test('acquiring phase type has no resumeAtMessageId field', () => {
    const { store } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'SIDECAR_NO_SESSION' },
      { type: 'HISTORY_OK', blocks: mockBlocks },
      { type: 'SEND_MESSAGE', text: 'hi', localId: 'l1' },
    ])
    expect(store.panel.phase).toBe('acquiring')
    if (store.panel.phase === 'acquiring') {
      expect('resumeAtMessageId' in store.panel).toBe(false)
    }
  })

  // Gap 2: POST_CREATE includes projectPath from store
  test('empty + SEND_MESSAGE → POST_CREATE includes projectPath', () => {
    const storeWithProject: ChatPanelStore = {
      ...INITIAL,
      projectPath: '/some/project',
    }
    const [, cmds] = coordinate(storeWithProject, {
      type: 'SEND_MESSAGE',
      text: 'hi',
      localId: 'l1',
      model: 'opus',
    })
    const createCmd = cmds.find((c) => c.cmd === 'POST_CREATE')
    expect(createCmd).toBeDefined()
    if (createCmd && createCmd.cmd === 'POST_CREATE') {
      expect(createCmd.projectPath).toBe('/some/project')
    }
  })

  // Gap 5: lastModel/lastPermissionMode captured from SEND_MESSAGE
  test('SEND_MESSAGE captures lastModel and lastPermissionMode in store', () => {
    const readyStore: ChatPanelStore = {
      panel: { phase: 'nobody', sessionId: 'abc', sub: { sub: 'ready', blocks: mockBlocks } },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
    }
    const [store] = coordinate(readyStore, {
      type: 'SEND_MESSAGE',
      text: 'hi',
      localId: 'l1',
      model: 'opus',
      permissionMode: 'plan',
    })
    expect(store.lastModel).toBe('opus')
    expect(store.lastPermissionMode).toBe('plan')
  })

  // Gap 5: recovering retry uses lastModel as fallback
  test('recovering + SEND_MESSAGE uses lastModel/lastPermissionMode as fallback', () => {
    const recoverStore: ChatPanelStore = {
      panel: {
        phase: 'recovering',
        sessionId: 'abc',
        blocks: mockBlocks,
        recovering: { kind: 'action_failed', error: 'fail' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: '/proj',
      lastModel: 'opus',
      lastPermissionMode: 'plan',
    }
    const [store, cmds] = coordinate(recoverStore, {
      type: 'SEND_MESSAGE',
      text: 'retry',
      localId: 'l2',
      // No model or permissionMode in event — should fallback to lastModel/lastPermissionMode
    })
    expect(store.panel.phase).toBe('acquiring')
    const resumeCmd = cmds.find((c) => c.cmd === 'POST_RESUME')
    expect(resumeCmd).toBeDefined()
    if (resumeCmd && resumeCmd.cmd === 'POST_RESUME') {
      expect(resumeCmd.model).toBe('opus')
      expect(resumeCmd.permissionMode).toBe('plan')
      expect(resumeCmd.projectPath).toBe('/proj')
    }
  })

  // Scenario 17: cc_cli + TERMINAL_BLOCK merges live blocks
  test('cc_cli + TERMINAL_BLOCK appends new block and replaces existing by ID', () => {
    const ccCliStore: ChatPanelStore = {
      panel: {
        phase: 'cc_cli',
        sessionId: 'abc',
        blocks: mockBlocks, // has 'u1' and 'a1'
        sub: { sub: 'watching' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
    }

    // New block → append
    const newBlock = { type: 'user' as const, id: 'u2', text: 'new msg', timestamp: 2 }
    const [s1] = coordinate(ccCliStore, { type: 'TERMINAL_BLOCK', block: newBlock as any })
    if (s1.panel.phase === 'cc_cli') {
      expect(s1.panel.blocks).toHaveLength(2) // '1' + 'u2'
      expect(s1.panel.blocks[1]).toEqual(newBlock)
    }

    // Existing block → replace by ID
    const updatedBlock = { type: 'user' as const, id: '1', text: 'updated', timestamp: 3 }
    const [s2] = coordinate(ccCliStore, { type: 'TERMINAL_BLOCK', block: updatedBlock as any })
    if (s2.panel.phase === 'cc_cli') {
      expect(s2.panel.blocks).toHaveLength(1) // replaced '1'
      expect(s2.panel.blocks[0]).toEqual(updatedBlock)
    }
  })

  // Gap 7: acquiring + LIVE_STATUS_CHANGED updates projectPath but stays acquiring
  test('acquiring + LIVE_STATUS_CHANGED updates projectPath without phase change', () => {
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
      lastModel: null,
      lastPermissionMode: null,
    }
    const [store] = coordinate(acquiringStore, {
      type: 'LIVE_STATUS_CHANGED',
      status: 'cc_owned',
      projectPath: '/new/path',
    })
    expect(store.panel.phase).toBe('acquiring')
    expect(store.projectPath).toBe('/new/path')
  })

  // Gap 3 (projectPath fix): takeover POST_RESUME includes projectPath
  test('cc_cli takeover → POST_RESUME includes projectPath from store', () => {
    const watchingStore: ChatPanelStore = {
      panel: { phase: 'cc_cli', sessionId: 'abc', blocks: mockBlocks, sub: { sub: 'watching' } },
      outbox: { messages: [] },
      meta: null,
      projectPath: '/my/project',
      lastModel: null,
      lastPermissionMode: null,
    }
    const [s1] = coordinate(watchingStore, { type: 'TAKEOVER_CLI' })
    const [, c2] = coordinate(s1, { type: 'KILL_CLI_OK' })
    const resumeCmd = c2.find((c) => c.cmd === 'POST_RESUME')
    expect(resumeCmd).toBeDefined()
    if (resumeCmd && resumeCmd.cmd === 'POST_RESUME') {
      expect(resumeCmd.projectPath).toBe('/my/project')
    }
  })

  // LIVE_STATUS_CHANGED with projectPath updates store.projectPath
  test('nobody + LIVE_STATUS_CHANGED with projectPath → store.projectPath updated', () => {
    const historyStore: ChatPanelStore = {
      panel: { phase: 'nobody', sessionId: 'abc', sub: { sub: 'ready', blocks: mockBlocks } },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
    }
    const [store] = coordinate(historyStore, {
      type: 'LIVE_STATUS_CHANGED',
      status: 'cc_owned',
      projectPath: '/from/sse',
    })
    expect(store.projectPath).toBe('/from/sse')
  })

  // Gap 8: SIDECAR_HAS_SESSION passes sessionState
  test('SELECT + SIDECAR_HAS_SESSION with sessionState → acquiring', () => {
    const { store } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'SIDECAR_HAS_SESSION', controlId: 'c1', sessionState: 'waiting_input' },
    ])
    expect(store.panel.phase).toBe('acquiring')
  })

  // SELECT_SESSION with projectPath sets store.projectPath
  test('SELECT_SESSION with projectPath → store.projectPath set', () => {
    const [store] = coordinate(INITIAL, {
      type: 'SELECT_SESSION',
      sessionId: 'abc',
      projectPath: '/initial/path',
    })
    expect(store.projectPath).toBe('/initial/path')
  })
})
