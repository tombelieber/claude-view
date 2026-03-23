/**
 * Regression protection for E2E-verified chat panel flows.
 *
 * Each describe block maps to a flow the user confirmed working via manual E2E.
 * Tests cover both FSM transitions AND derive-layer outputs (ViewMode, InputBarState,
 * ThinkingState, CanSend, Blocks) at every significant step — because regressions
 * in the derive layer silently break the UI even when the FSM is correct.
 *
 * Verified flows:
 *   1. Init new chat (empty → create → sdk_owned)
 *   2. Resume history chat (nobody → resume → sdk_owned)
 *   3. Shut down SDK-owned live session (sdk_owned → closed)
 *   4. Watching mode (nobody → cc_cli.watching)
 *   5. Resume from closed (closed → resume → sdk_owned)
 */
import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { describe, expect, test } from 'vitest'
import { coordinate } from './coordinator'
import {
  deriveBlocks,
  deriveCanFork,
  deriveCanSend,
  deriveConnectionStatus,
  deriveInputBar,
  deriveThinkingState,
  deriveViewMode,
} from './derive'
import type { ChatPanelStore, Command, RawEvent, SessionMeta } from './types'

// ── Fixtures ──────────────────────────────────────────────────────

const EMPTY_STORE: ChatPanelStore = {
  panel: { phase: 'empty' },
  outbox: { messages: [] },
  meta: null,
  projectPath: null,
  lastModel: null,
  lastPermissionMode: null,
  historyPagination: null,
}

const historyBlocks = [
  { type: 'user' as const, id: 'u1', text: 'hello', timestamp: 1 },
  {
    type: 'assistant' as const,
    id: 'a1',
    segments: [{ kind: 'text', text: 'Hi there!' }],
    streaming: false,
    timestamp: 2,
  },
  // biome-ignore lint/suspicious/noExplicitAny: test fixture — ConversationBlock shape is complex
] as any as ConversationBlock[]

const SESSION_INIT_EVENT: RawEvent = {
  type: 'SESSION_INIT',
  model: 'claude-sonnet-4-20250514',
  permissionMode: 'default',
  slashCommands: ['help'],
  mcpServers: [],
  skills: [],
  agents: [],
  capabilities: [],
}

const META_FIXTURE: SessionMeta = {
  model: 'claude-sonnet-4-20250514',
  permissionMode: 'default',
  slashCommands: ['help'],
  mcpServers: [],
  skills: [],
  agents: [],
  capabilities: [],
  totalInputTokens: 0,
  contextWindowSize: 0,
}

// ── Test helpers ──────────────────────────────────────────────────

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

/** Snapshot all derive outputs for a given store. */
function snapshot(store: ChatPanelStore) {
  return {
    viewMode: deriveViewMode(store),
    inputBar: deriveInputBar(store),
    thinking: deriveThinkingState(store),
    canSend: deriveCanSend(store),
    canFork: deriveCanFork(store),
    connectionStatus: deriveConnectionStatus(store),
    blockCount: deriveBlocks(store).length,
  }
}

// ═════════════════════════════════════════════════════════════════
// FLOW 1: Init new chat
// ═════════════════════════════════════════════════════════════════

describe('regression: init new chat (empty → create → sdk_owned)', () => {
  test('empty state → UI shows blank page with active input', () => {
    const s = snapshot(EMPTY_STORE)
    expect(s.viewMode).toBe('blank')
    expect(s.inputBar).toBe('dormant')
    expect(s.thinking).toBeNull()
    expect(s.canSend).toBe(true)
    expect(s.canFork).toBe(false)
    expect(s.blockCount).toBe(0)
  })

  test('send from empty → acquiring(create), optimistic message visible', () => {
    const { store, cmds } = step(EMPTY_STORE, {
      type: 'SEND_MESSAGE',
      text: 'start a new chat',
      localId: 'l1',
      model: 'claude-sonnet-4-20250514',
    })
    expect(store.panel.phase).toBe('acquiring')
    if (store.panel.phase === 'acquiring') {
      expect(store.panel.action).toBe('create')
      expect(store.panel.pendingMessage).toBe('start a new chat')
    }
    // Outbox has the queued-then-sent message
    expect(store.outbox.messages).toHaveLength(1)
    expect(store.outbox.messages[0].status).toBe('sent')

    // Derive: connecting state, thinking indicator, optimistic block
    const s = snapshot(store)
    expect(s.viewMode).toBe('connecting')
    expect(s.inputBar).toBe('connecting')
    expect(s.thinking).toBe('connecting')
    expect(s.canSend).toBe(false)
    expect(s.connectionStatus?.kind).toBe('loading')
    expect(s.connectionStatus?.message).toContain('Creating')
    // Optimistic outbox message shows as a block
    expect(s.blockCount).toBe(1)

    // Must emit POST_CREATE, not POST_RESUME
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'POST_CREATE' }))
    expect(cmds).not.toContainEqual(expect.objectContaining({ cmd: 'POST_RESUME' }))
  })

  test('full create → acquire → WS → init → streaming → complete lifecycle', () => {
    const finalBlocks = [
      { type: 'user', id: 'u1', text: 'start a new chat', timestamp: 1 },
      {
        type: 'assistant',
        id: 'a1',
        segments: [{ kind: 'text', text: 'Hello!' }],
        streaming: false,
        timestamp: 2,
      },
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
    ] as any

    let current = EMPTY_STORE

    // Step 1: SEND_MESSAGE → acquiring
    ;({ store: current } = step(current, {
      type: 'SEND_MESSAGE',
      text: 'start a new chat',
      localId: 'l1',
    }))
    expect(snapshot(current).thinking).toBe('connecting')

    // Step 2: ACQUIRE_OK → ws_connecting
    ;({ store: current } = step(current, {
      type: 'ACQUIRE_OK',
      controlId: 'c1',
      sessionId: 'new-session-1',
    }))
    expect(current.panel.phase).toBe('acquiring')
    expect(snapshot(current).connectionStatus?.message).toBe('Connecting to session...')

    // Step 3: WS_OPEN → ws_initializing
    ;({ store: current } = step(current, { type: 'WS_OPEN' }))
    expect(snapshot(current).connectionStatus?.message).toBe('Initializing...')

    // Step 4: SESSION_INIT → sdk_owned
    ;({ store: current } = step(current, SESSION_INIT_EVENT))
    expect(current.panel.phase).toBe('sdk_owned')
    if (current.panel.phase === 'sdk_owned') {
      expect(current.panel.sessionId).toBe('new-session-1')
      expect(current.panel.ephemeral).toBe(true)
      expect(current.panel.turn).toEqual({ turn: 'pending' })
    }
    let s = snapshot(current)
    expect(s.viewMode).toBe('active')
    expect(s.inputBar).toBe('streaming')
    expect(s.thinking).toBe('thinking')
    expect(s.connectionStatus).toBeNull()

    // Step 5: STREAM_DELTA → streaming with visible text
    ;({ store: current } = step(current, { type: 'STREAM_DELTA', text: 'Hello!' }))
    if (current.panel.phase === 'sdk_owned') {
      expect(current.panel.turn).toEqual({ turn: 'streaming' })
      expect(current.panel.pendingText).toBe('Hello!')
    }
    s = snapshot(current)
    expect(s.thinking).toBeNull() // text visible → no more thinking indicator
    // Blocks: outbox msg + pending assistant text
    expect(s.blockCount).toBeGreaterThanOrEqual(2)

    // Step 6: TURN_COMPLETE → idle
    ;({ store: current } = step(current, {
      type: 'TURN_COMPLETE',
      blocks: finalBlocks,
      totalInputTokens: 300,
      contextWindowSize: 200000,
    }))
    if (current.panel.phase === 'sdk_owned') {
      expect(current.panel.turn).toEqual({ turn: 'idle' })
      expect(current.panel.pendingText).toBe('')
    }
    s = snapshot(current)
    expect(s.viewMode).toBe('active')
    expect(s.inputBar).toBe('active')
    expect(s.canSend).toBe(true)
    expect(s.canFork).toBe(true) // now has blocks → forkable
    expect(s.blockCount).toBe(2)

    // Meta captured
    expect(current.meta).toMatchObject({
      model: 'claude-sonnet-4-20250514',
      totalInputTokens: 300,
    })
  })

  test('lastModel/lastPermissionMode captured from first SEND_MESSAGE', () => {
    const { store } = step(EMPTY_STORE, {
      type: 'SEND_MESSAGE',
      text: 'hi',
      localId: 'l1',
      model: 'claude-opus-4-20250514',
      permissionMode: 'plan',
    })
    expect(store.lastModel).toBe('claude-opus-4-20250514')
    expect(store.lastPermissionMode).toBe('plan')
  })

  test('projectPath threaded into POST_CREATE', () => {
    const storeWithPath = { ...EMPTY_STORE, projectPath: '/my/project' }
    const { cmds } = step(storeWithPath, {
      type: 'SEND_MESSAGE',
      text: 'hi',
      localId: 'l1',
    })
    const createCmd = cmds.find((c) => c.cmd === 'POST_CREATE')
    expect(createCmd).toBeDefined()
    if (createCmd?.cmd === 'POST_CREATE') {
      expect(createCmd.projectPath).toBe('/my/project')
    }
  })
})

// ═════════════════════════════════════════════════════════════════
// FLOW 2: Resume history chat
// ═════════════════════════════════════════════════════════════════

describe('regression: resume history chat (nobody → resume → sdk_owned)', () => {
  const readyStore: ChatPanelStore = {
    panel: {
      phase: 'nobody',
      sessionId: 'sess-1',
      sub: { sub: 'ready', blocks: historyBlocks },
    },
    outbox: { messages: [] },
    meta: null,
    projectPath: '/my/project',
    lastModel: null,
    lastPermissionMode: null,
    historyPagination: null,
  }

  test('nobody(ready) → UI shows history with active input', () => {
    const s = snapshot(readyStore)
    expect(s.viewMode).toBe('history')
    expect(s.inputBar).toBe('active')
    expect(s.thinking).toBeNull()
    expect(s.canSend).toBe(true)
    expect(s.canFork).toBe(true) // has blocks
    expect(s.blockCount).toBe(2) // u1 + a1
    expect(s.connectionStatus).toBeNull()
  })

  test('nobody(loading) → thinking indicator shown, input disabled', () => {
    const loadingStore: ChatPanelStore = {
      ...readyStore,
      panel: { phase: 'nobody', sessionId: 'sess-1', sub: { sub: 'loading' } },
    }
    const s = snapshot(loadingStore)
    expect(s.viewMode).toBe('loading')
    expect(s.thinking).toBe('loading')
    expect(s.canSend).toBe(false)
    expect(s.blockCount).toBe(0)
  })

  test('send from history → acquiring(resume), history blocks preserved', () => {
    const { store, cmds } = step(readyStore, {
      type: 'SEND_MESSAGE',
      text: 'follow up question',
      localId: 'l1',
    })
    expect(store.panel.phase).toBe('acquiring')
    if (store.panel.phase === 'acquiring') {
      expect(store.panel.action).toBe('resume')
      expect(store.panel.historyBlocks).toEqual(historyBlocks)
      expect(store.panel.pendingMessage).toBe('follow up question')
    }

    const s = snapshot(store)
    expect(s.viewMode).toBe('connecting')
    expect(s.thinking).toBe('connecting')
    expect(s.connectionStatus?.message).toContain('Resuming')

    // Blocks: history + optimistic outbox message
    expect(s.blockCount).toBe(3)

    expect(cmds).toContainEqual(
      expect.objectContaining({ cmd: 'POST_RESUME', sessionId: 'sess-1' }),
    )
  })

  test('full resume → stream → complete lifecycle preserves projectPath', () => {
    const finalBlocks = [
      ...historyBlocks,
      { type: 'user', id: 'u2', text: 'follow up', timestamp: 3 },
      {
        type: 'assistant',
        id: 'a2',
        segments: [{ kind: 'text', text: 'Sure thing!' }],
        streaming: false,
        timestamp: 4,
      },
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
    ] as any

    const { store } = drive(readyStore, [
      { type: 'SEND_MESSAGE', text: 'follow up', localId: 'l1' },
      { type: 'ACQUIRE_OK', controlId: 'c1' },
      { type: 'WS_OPEN' },
      SESSION_INIT_EVENT,
      { type: 'STREAM_DELTA', text: 'Sure thing!' },
      {
        type: 'TURN_COMPLETE',
        blocks: finalBlocks,
        totalInputTokens: 600,
        contextWindowSize: 200000,
      },
    ])

    expect(store.panel.phase).toBe('sdk_owned')
    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.turn).toEqual({ turn: 'idle' })
      expect(store.panel.blocks).toHaveLength(4)
      expect(store.panel.ephemeral).toBe(false)
    }
    // projectPath survives the full flow
    expect(store.projectPath).toBe('/my/project')
    expect(store.meta?.totalInputTokens).toBe(600)
  })

  test('SELECT → sidecar active → auto-connect skips resume POST', () => {
    const { store, allCmds } = drive(EMPTY_STORE, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'SIDECAR_HAS_SESSION', controlId: 'c1' },
      { type: 'WS_OPEN' },
      SESSION_INIT_EVENT,
    ])
    expect(store.panel.phase).toBe('sdk_owned')
    // No POST_RESUME — sidecar was already active, just WS connect
    expect(allCmds).not.toContainEqual(expect.objectContaining({ cmd: 'POST_RESUME' }))
    expect(allCmds).not.toContainEqual(expect.objectContaining({ cmd: 'POST_CREATE' }))
    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'OPEN_SIDECAR_WS' }))
  })
})

// ═════════════════════════════════════════════════════════════════
// FLOW 3: Shut down SDK-owned live session
// ═════════════════════════════════════════════════════════════════

describe('regression: shut down SDK-owned session (sdk_owned → closed)', () => {
  const liveStore: ChatPanelStore = {
    panel: {
      phase: 'sdk_owned',
      sessionId: 'sess-1',
      controlId: 'c1',
      blocks: historyBlocks,
      pendingText: '',
      ephemeral: false,
      turn: { turn: 'idle' },
      conn: { health: 'ok' },
    },
    outbox: { messages: [] },
    meta: META_FIXTURE,
    projectPath: '/my/project',
    lastModel: 'claude-sonnet-4-20250514',
    lastPermissionMode: 'default',
    historyPagination: null,
  }

  test('sdk_owned(idle) → UI shows active session with full controls', () => {
    const s = snapshot(liveStore)
    expect(s.viewMode).toBe('active')
    expect(s.inputBar).toBe('active')
    expect(s.thinking).toBeNull()
    expect(s.canSend).toBe(true)
    expect(s.canFork).toBe(true)
    expect(s.connectionStatus).toBeNull()
    expect(s.blockCount).toBe(2)
  })

  test('SESSION_CLOSED → closed phase, WS cleaned up, blocks preserved', () => {
    const { store, cmds } = step(liveStore, { type: 'SESSION_CLOSED' })
    expect(store.panel.phase).toBe('closed')
    if (store.panel.phase === 'closed') {
      expect(store.panel.blocks).toEqual(historyBlocks)
      expect(store.panel.sessionId).toBe('sess-1')
      expect(store.panel.ephemeral).toBe(false)
    }

    const s = snapshot(store)
    expect(s.viewMode).toBe('closed')
    expect(s.inputBar).toBe('completed')
    expect(s.canSend).toBe(true) // non-ephemeral → can resume
    expect(s.canFork).toBe(false) // fork only in nobody/sdk_owned
    expect(s.blockCount).toBe(2)

    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'CLOSE_SIDECAR_WS' }))
  })

  test('SESSION_CLOSED on ephemeral session → canSend is false', () => {
    const ephemeralStore: ChatPanelStore = {
      ...liveStore,
      // biome-ignore lint/suspicious/noExplicitAny: test fixture — spread override
      panel: { ...liveStore.panel, ephemeral: true } as any,
    }
    const { store } = step(ephemeralStore, { type: 'SESSION_CLOSED' })
    expect(deriveCanSend(store)).toBe(false)
  })

  test('SESSION_CLOSED mid-streaming → blocks + pendingText cleared', () => {
    const streamingStore: ChatPanelStore = {
      ...liveStore,
      panel: {
        ...liveStore.panel,
        turn: { turn: 'streaming' },
        pendingText: 'partial response...',
        // biome-ignore lint/suspicious/noExplicitAny: test fixture — spread override
      } as any,
    }
    const { store } = step(streamingStore, { type: 'SESSION_CLOSED' })
    expect(store.panel.phase).toBe('closed')
    if (store.panel.phase === 'closed') {
      // Blocks preserved (server blocks), pendingText gone (was streaming cursor)
      expect(store.panel.blocks).toEqual(historyBlocks)
    }
  })

  test('meta and projectPath survive SESSION_CLOSED', () => {
    const { store } = step(liveStore, { type: 'SESSION_CLOSED' })
    expect(store.meta).toEqual(META_FIXTURE)
    expect(store.projectPath).toBe('/my/project')
    expect(store.lastModel).toBe('claude-sonnet-4-20250514')
    expect(store.lastPermissionMode).toBe('default')
  })
})

// ═════════════════════════════════════════════════════════════════
// FLOW 4: Watching mode (cc_cli)
// ═════════════════════════════════════════════════════════════════

describe('regression: watching mode (nobody → cc_cli.watching)', () => {
  const readyStore: ChatPanelStore = {
    panel: {
      phase: 'nobody',
      sessionId: 'sess-1',
      sub: { sub: 'ready', blocks: historyBlocks },
    },
    outbox: { messages: [] },
    meta: null,
    projectPath: null,
    lastModel: null,
    lastPermissionMode: null,
    historyPagination: null,
  }

  test('LIVE_STATUS cc_owned → cc_cli(watching), terminal WS opened', () => {
    const { store, cmds } = step(readyStore, {
      type: 'LIVE_STATUS_CHANGED',
      status: 'cc_owned',
      projectPath: '/cli/project',
    })
    expect(store.panel.phase).toBe('cc_cli')
    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.sub).toEqual({ sub: 'watching' })
      expect(store.panel.blocks).toEqual(historyBlocks)
    }

    const s = snapshot(store)
    expect(s.viewMode).toBe('watching')
    expect(s.inputBar).toBe('controlled_elsewhere')
    expect(s.canSend).toBe(true) // watching allows send (triggers takeover dialog)
    expect(s.canFork).toBe(false) // cc_cli doesn't support fork

    expect(cmds).toContainEqual(
      expect.objectContaining({ cmd: 'OPEN_TERMINAL_WS', sessionId: 'sess-1' }),
    )
    // projectPath updated from SSE
    expect(store.projectPath).toBe('/cli/project')
  })

  test('race: LIVE_STATUS before HISTORY_OK → deferred, then completes', () => {
    const loadingStore: ChatPanelStore = {
      ...readyStore,
      panel: { phase: 'nobody', sessionId: 'sess-1', sub: { sub: 'loading' } },
    }

    // Step 1: LIVE_STATUS while loading → deferred
    const { store: mid, cmds: midCmds } = step(loadingStore, {
      type: 'LIVE_STATUS_CHANGED',
      status: 'cc_owned',
    })
    expect(mid.panel.phase).toBe('nobody')
    if (mid.panel.phase === 'nobody' && mid.panel.sub.sub === 'loading') {
      expect(mid.panel.sub.pendingLive).toBe('cc_owned')
    }
    expect(midCmds).toHaveLength(0) // no terminal WS yet

    // Step 2: HISTORY_OK → deferred transition fires
    const { store, cmds } = step(mid, { type: 'HISTORY_OK', blocks: historyBlocks })
    expect(store.panel.phase).toBe('cc_cli')
    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.blocks).toEqual(historyBlocks)
    }
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'OPEN_TERMINAL_WS' }))
  })

  test('TERMINAL_BLOCK appends new blocks in watching mode', () => {
    const watchingStore: ChatPanelStore = {
      panel: {
        phase: 'cc_cli',
        sessionId: 'sess-1',
        blocks: historyBlocks,
        sub: { sub: 'watching' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    // biome-ignore lint/suspicious/noExplicitAny: test fixture
    const newBlock = { type: 'user', id: 'u3', text: 'live msg', timestamp: 5 } as any
    const { store } = step(watchingStore, { type: 'TERMINAL_BLOCK', block: newBlock })
    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.blocks).toHaveLength(3)
      expect(store.panel.blocks[2]).toEqual(newBlock)
    }
  })

  test('TERMINAL_BLOCK replaces existing block by ID', () => {
    const watchingStore: ChatPanelStore = {
      panel: {
        phase: 'cc_cli',
        sessionId: 'sess-1',
        blocks: historyBlocks,
        sub: { sub: 'watching' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    // biome-ignore lint/suspicious/noExplicitAny: test fixture
    const updatedBlock = { type: 'user', id: 'u1', text: 'updated', timestamp: 10 } as any
    const { store } = step(watchingStore, { type: 'TERMINAL_BLOCK', block: updatedBlock })
    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.blocks).toHaveLength(2) // replaced, not appended
      expect(store.panel.blocks[0]).toEqual(updatedBlock)
    }
  })

  test('LIVE_STATUS inactive → back to nobody(ready) with blocks preserved', () => {
    const watchingStore: ChatPanelStore = {
      panel: {
        phase: 'cc_cli',
        sessionId: 'sess-1',
        blocks: historyBlocks,
        sub: { sub: 'watching' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const { store, cmds } = step(watchingStore, {
      type: 'LIVE_STATUS_CHANGED',
      status: 'inactive',
    })
    expect(store.panel.phase).toBe('nobody')
    if (store.panel.phase === 'nobody') {
      expect(store.panel.sub).toEqual({ sub: 'ready', blocks: historyBlocks })
    }
    expect(cmds).toContainEqual({ cmd: 'CLOSE_TERMINAL_WS' })

    const s = snapshot(store)
    expect(s.viewMode).toBe('history')
    expect(s.inputBar).toBe('active')
    expect(s.canSend).toBe(true)
  })
})

// ═════════════════════════════════════════════════════════════════
// FLOW 5: Resume from closed
// ═════════════════════════════════════════════════════════════════

describe('regression: resume from closed (closed → resume → sdk_owned)', () => {
  const closedStore: ChatPanelStore = {
    panel: {
      phase: 'closed',
      sessionId: 'sess-1',
      blocks: historyBlocks,
      ephemeral: false,
    },
    outbox: { messages: [] },
    meta: META_FIXTURE,
    projectPath: '/my/project',
    lastModel: 'claude-sonnet-4-20250514',
    lastPermissionMode: 'default',
    historyPagination: null,
  }

  test('closed(non-ephemeral) → UI shows history with resume-capable input', () => {
    const s = snapshot(closedStore)
    expect(s.viewMode).toBe('closed')
    expect(s.inputBar).toBe('completed')
    expect(s.canSend).toBe(true)
    expect(s.blockCount).toBe(2)
    expect(s.connectionStatus).toBeNull()
  })

  test('send from closed → acquiring(resume), message carried as initialMessage', () => {
    const { store, cmds } = step(closedStore, {
      type: 'SEND_MESSAGE',
      text: 'continue please',
      localId: 'l1',
    })
    expect(store.panel.phase).toBe('acquiring')
    if (store.panel.phase === 'acquiring') {
      expect(store.panel.action).toBe('resume')
      expect(store.panel.pendingMessage).toBe('continue please')
      expect(store.panel.historyBlocks).toEqual(historyBlocks)
    }

    // Outbox queued and immediately sent (initialMessage path)
    expect(store.outbox.messages).toHaveLength(1)
    expect(store.outbox.messages[0].status).toBe('sent')

    const s = snapshot(store)
    expect(s.viewMode).toBe('connecting')
    expect(s.thinking).toBe('connecting')

    // POST_RESUME carries the message
    const resumeCmd = cmds.find((c) => c.cmd === 'POST_RESUME')
    expect(resumeCmd).toBeDefined()
    if (resumeCmd?.cmd === 'POST_RESUME') {
      expect(resumeCmd.message).toBe('continue please')
      expect(resumeCmd.sessionId).toBe('sess-1')
      expect(resumeCmd.projectPath).toBe('/my/project')
    }
  })

  test('full closed → resume → sdk_owned lifecycle', () => {
    const finalBlocks = [
      ...historyBlocks,
      { type: 'user', id: 'u2', text: 'continue', timestamp: 10 },
      {
        type: 'assistant',
        id: 'a2',
        segments: [{ kind: 'text', text: 'Continuing...' }],
        streaming: false,
        timestamp: 11,
      },
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
    ] as any

    const { store } = drive(closedStore, [
      { type: 'SEND_MESSAGE', text: 'continue', localId: 'l1' },
      { type: 'ACQUIRE_OK', controlId: 'c2' },
      { type: 'WS_OPEN' },
      SESSION_INIT_EVENT,
      { type: 'STREAM_DELTA', text: 'Continuing...' },
      {
        type: 'TURN_COMPLETE',
        blocks: finalBlocks,
        totalInputTokens: 800,
        contextWindowSize: 200000,
      },
    ])

    expect(store.panel.phase).toBe('sdk_owned')
    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.turn).toEqual({ turn: 'idle' })
      expect(store.panel.blocks).toHaveLength(4)
    }

    const s = snapshot(store)
    expect(s.viewMode).toBe('active')
    expect(s.inputBar).toBe('active')
    expect(s.canSend).toBe(true)

    // Orthogonal state preserved
    expect(store.projectPath).toBe('/my/project')
    expect(store.meta?.totalInputTokens).toBe(800)
  })

  test('lastModel/lastPermissionMode used as fallback in resume POST', () => {
    const { cmds } = step(closedStore, {
      type: 'SEND_MESSAGE',
      text: 'retry',
      localId: 'l1',
      // No model/permissionMode in event → should fallback
    })
    const resumeCmd = cmds.find((c) => c.cmd === 'POST_RESUME')
    if (resumeCmd?.cmd === 'POST_RESUME') {
      expect(resumeCmd.model).toBe('claude-sonnet-4-20250514')
      expect(resumeCmd.permissionMode).toBe('default')
    }
  })

  test('LIVE_STATUS cc_owned from closed → enters watching mode', () => {
    const { store, cmds } = step(closedStore, {
      type: 'LIVE_STATUS_CHANGED',
      status: 'cc_owned',
    })
    expect(store.panel.phase).toBe('cc_cli')
    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.sub).toEqual({ sub: 'watching' })
      expect(store.panel.blocks).toEqual(historyBlocks)
    }
    expect(cmds).toContainEqual(expect.objectContaining({ cmd: 'OPEN_TERMINAL_WS' }))
  })
})

// ═════════════════════════════════════════════════════════════════
// Cross-cutting: outbox reconciliation in deriveBlocks
// ═════════════════════════════════════════════════════════════════

describe('regression: outbox reconciliation produces correct block counts', () => {
  test('queued message shows as optimistic block before server confirms', () => {
    const store: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'sess-1',
        controlId: 'c1',
        blocks: historyBlocks,
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'pending' },
        conn: { health: 'ok' },
      },
      outbox: {
        messages: [{ localId: 'l1', text: 'new message', status: 'sent' as const }],
      },
      meta: META_FIXTURE,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const blocks = deriveBlocks(store)
    // 2 history blocks + 1 optimistic outbox block
    expect(blocks).toHaveLength(3)
    expect(blocks[2].id).toBe('outbox-l1')
  })

  test('outbox message deduped when server blocks include same text', () => {
    const blocksWithUserMsg = [
      ...historyBlocks,
      { type: 'user', id: 'u2', text: 'new message', timestamp: 5 },
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
    ] as any

    const store: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'sess-1',
        controlId: 'c1',
        blocks: blocksWithUserMsg,
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'streaming' },
        conn: { health: 'ok' },
      },
      outbox: {
        messages: [{ localId: 'l1', text: 'new message', status: 'sent' as const }],
      },
      meta: META_FIXTURE,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const blocks = deriveBlocks(store)
    // 3 server blocks, outbox deduped (same text already in server blocks)
    expect(blocks).toHaveLength(3)
  })

  test('pendingText generates synthetic __pending__ assistant block', () => {
    const store: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'sess-1',
        controlId: 'c1',
        blocks: historyBlocks,
        pendingText: 'streaming text...',
        ephemeral: false,
        turn: { turn: 'streaming' },
        conn: { health: 'ok' },
      },
      outbox: { messages: [] },
      meta: META_FIXTURE,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const blocks = deriveBlocks(store)
    const pending = blocks.find((b) => b.id === '__pending__')
    expect(pending).toBeDefined()
    expect(pending?.type).toBe('assistant')
    if (pending?.type === 'assistant') {
      expect(pending.streaming).toBe(true)
      const seg = pending.segments?.[0]
      expect(seg?.kind).toBe('text')
      if (seg?.kind === 'text') {
        expect(seg.text).toBe('streaming text...')
      }
    }
  })
})
