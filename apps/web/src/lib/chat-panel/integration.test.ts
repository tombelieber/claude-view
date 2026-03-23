import { describe, expect, test } from 'vitest'
import { coordinate } from './coordinator'
import { deriveBlocks } from './derive'
import { mapWsEvent } from './event-mapper'
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

// biome-ignore lint/suspicious/noExplicitAny: test fixture — ConversationBlock shape is complex
const mockBlocks = [{ type: 'user' as const, id: '1', text: 'hi', timestamp: 1 }] as any

function drive(
  init: ChatPanelStore,
  events: RawEvent[],
): { store: ChatPanelStore; allCmds: Command[] } {
  let store = init
  const allCmds: Command[] = []
  for (const event of events) {
    const [newStore, cmds] = coordinate(store, event)
    store = newStore
    allCmds.push(...cmds)
  }
  return { store, allCmds }
}

// ── Full resume flow ──────────────────────────────────────────────

describe('integration: full resume flow', () => {
  test('select → history → send → resume → WS → SESSION_INIT → sdk_owned → stream → turn complete', () => {
    const finalBlocks = [
      ...mockBlocks,
      { type: 'user' as const, id: '2', text: 'hello', timestamp: 2 },
      {
        type: 'assistant' as const,
        id: '3',
        text: 'Hello!',
        timestamp: 3,
        toolCalls: [],
        thinking: null,
        thinkingTokens: 0,
        rawJson: null,
      },
    ] as any

    const { store, allCmds } = drive(INITIAL, [
      // 1. Select session → nobody(loading), emits FETCH_HISTORY + CHECK_SIDECAR_ACTIVE
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      // 2. Sidecar inactive — no-op in nobody
      { type: 'SIDECAR_NO_SESSION' },
      // 3. History loads → nobody(ready)
      { type: 'HISTORY_OK', blocks: mockBlocks },
      // 4. User sends → acquiring(posting), emits POST_RESUME
      { type: 'SEND_MESSAGE', text: 'hello', localId: 'l1' },
      // 5. SSE race — LIVE_STATUS_CHANGED ignored during acquiring
      { type: 'LIVE_STATUS_CHANGED', status: 'cc_owned' },
      // 6. Resume OK → acquiring(ws_connecting)
      { type: 'ACQUIRE_OK', controlId: 'ctrl-1' },
      // 7. WS opens → acquiring(ws_initializing), emits START_TIMER(init-timeout)
      { type: 'WS_OPEN' },
      // 8. Session init → exits acquiring → sdk_owned(pending), emits CANCEL_TIMER + WS_SEND (queued msg)
      {
        type: 'SESSION_INIT',
        model: 'opus',
        permissionMode: 'default',
        slashCommands: [],
        mcpServers: [],
        skills: [],
        agents: [],
        capabilities: [],
      },
      // 9. Stream delta → sdk_owned(streaming)
      { type: 'STREAM_DELTA', text: 'Hello! ' },
      // 10. Turn complete → sdk_owned(idle), blocks replaced, meta updated with usage
      {
        type: 'TURN_COMPLETE',
        blocks: finalBlocks,
        totalInputTokens: 500,
        contextWindowSize: 200000,
      },
    ])

    // Final state: sdk_owned, idle, with final blocks
    expect(store.panel.phase).toBe('sdk_owned')
    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.turn).toEqual({ turn: 'idle' })
      expect(store.panel.pendingText).toBe('')
      expect(store.panel.blocks).toHaveLength(3)
      expect(store.panel.controlId).toBe('ctrl-1')
    }

    // Meta: SESSION_INIT during acquiring now populates meta via metaTransition
    // on exit to sdk_owned. TURN_COMPLETE then updates totalInputTokens + contextWindowSize.
    expect(store.meta).toMatchObject({
      model: 'opus',
      permissionMode: 'default',
      totalInputTokens: 500,
      contextWindowSize: 200000,
    })

    // Commands: verify key side effects emitted
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'FETCH_HISTORY', sessionId: 'abc' }),
    )
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'CHECK_SIDECAR_ACTIVE', sessionId: 'abc' }),
    )
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'POST_RESUME', sessionId: 'abc' }),
    )
    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'OPEN_SIDECAR_WS' }))
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'START_TIMER', id: 'init-timeout' }),
    )
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'CANCEL_TIMER', id: 'init-timeout' }),
    )
    // Message NOT drained via WS — it was already sent as POST_RESUME initialMessage.
    // Outbox entry is 'sent' (not 'queued'), so exitAcquiring skips it.
    expect(allCmds).not.toContainEqual(expect.objectContaining({ cmd: 'WS_SEND' }))
  })
})

// ── Fork flow ─────────────────────────────────────────────────────

describe('integration: fork flow', () => {
  test('nobody(ready) → fork → acquire → WS → SESSION_INIT → sdk_owned', () => {
    const historyStore: ChatPanelStore = {
      panel: {
        phase: 'nobody',
        sessionId: 'abc',
        sub: { sub: 'ready', blocks: mockBlocks },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const { store, allCmds } = drive(historyStore, [
      // Fork from history
      { type: 'FORK_SESSION', message: 'continue from here' },
      // Acquire OK with new session ID
      { type: 'ACQUIRE_OK', controlId: 'c2', sessionId: 'fork-123' },
      // WS connects
      { type: 'WS_OPEN' },
      // Session init exits acquiring
      {
        type: 'SESSION_INIT',
        model: 'sonnet',
        permissionMode: 'default',
        slashCommands: [],
        mcpServers: [],
        skills: [],
        agents: [],
        capabilities: [],
      },
    ])

    expect(store.panel.phase).toBe('sdk_owned')
    if (store.panel.phase === 'sdk_owned') {
      // Fork should update sessionId to the new one
      expect(store.panel.sessionId).toBe('fork-123')
      // Fork with message → pending (agent will respond)
      expect(store.panel.turn).toEqual({ turn: 'pending' })
    }

    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'POST_FORK', sessionId: 'abc' }))
  })
})

// ── Takeover flow (fork-based, no kill) ──────────────────────────

describe('integration: takeover flow (fork)', () => {
  test('cc_cli(watching) → TAKEOVER_CLI → fork → ACQUIRE_OK → WS → SESSION_INIT → sdk_owned', () => {
    const watchingStore: ChatPanelStore = {
      panel: {
        phase: 'cc_cli',
        sessionId: 'abc',
        blocks: [],
        sub: { sub: 'watching' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const { store, allCmds } = drive(watchingStore, [
      // User clicks takeover → acquiring{fork}, emits CLOSE_TERMINAL_WS + POST_FORK
      { type: 'TAKEOVER_CLI' },
      // Fork OK → acquiring(ws_connecting) with new forked sessionId
      { type: 'ACQUIRE_OK', controlId: 'c3', sessionId: 'forked-abc' },
      // WS opens → acquiring(ws_initializing)
      { type: 'WS_OPEN' },
      // Session init → sdk_owned
      {
        type: 'SESSION_INIT',
        model: 'opus',
        permissionMode: 'auto',
        slashCommands: [],
        mcpServers: [],
        skills: [],
        agents: [],
        capabilities: [],
      },
    ])

    expect(store.panel.phase).toBe('sdk_owned')
    // Fork-based: no kill, uses POST_FORK
    expect(allCmds).not.toContainEqual(expect.objectContaining({ cmd: 'KILL_CLI_SESSION' }))
    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'CLOSE_TERMINAL_WS' }))
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'POST_FORK', sessionId: 'abc' }),
    )
  })
})

// ── Error recovery ────────────────────────────────────────────────

describe('integration: error recovery', () => {
  test('nobody → send → resume fails → recovering → retry send → resume OK → sdk_owned', () => {
    const historyStore: ChatPanelStore = {
      panel: {
        phase: 'nobody',
        sessionId: 'abc',
        sub: { sub: 'ready', blocks: mockBlocks },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const { store, allCmds } = drive(historyStore, [
      // Send triggers resume
      { type: 'SEND_MESSAGE', text: 'hello', localId: 'l1' },
      // Resume fails → recovering(action_failed)
      { type: 'ACQUIRE_FAILED', error: 'server overloaded' },
      // User retries from recovering → back to acquiring
      { type: 'SEND_MESSAGE', text: 'hello', localId: 'l2' },
      // Second resume succeeds
      { type: 'ACQUIRE_OK', controlId: 'c4' },
      { type: 'WS_OPEN' },
      {
        type: 'SESSION_INIT',
        model: 'opus',
        permissionMode: 'default',
        slashCommands: [],
        mcpServers: [],
        skills: [],
        agents: [],
        capabilities: [],
      },
    ])

    expect(store.panel.phase).toBe('sdk_owned')
    // Error toast emitted on first failure
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'TOAST', variant: 'error', message: 'server overloaded' }),
    )
    // Two POST_RESUME attempts
    const resumes = allCmds.filter((c) => c.cmd === 'POST_RESUME')
    expect(resumes).toHaveLength(2)
  })
})

// ── Create new session ────────────────────────────────────────────

describe('integration: create new session', () => {
  test('empty → send → create → WS → SESSION_INIT → sdk_owned', () => {
    const { store, allCmds } = drive(INITIAL, [
      // Send from empty → acquiring(create), emits POST_CREATE
      { type: 'SEND_MESSAGE', text: 'new chat', localId: 'l5' },
      // Create OK with new session ID
      { type: 'ACQUIRE_OK', controlId: 'c5', sessionId: 'new-123' },
      { type: 'WS_OPEN' },
      {
        type: 'SESSION_INIT',
        model: 'sonnet',
        permissionMode: 'default',
        slashCommands: ['help'],
        mcpServers: [],
        skills: ['code-review'],
        agents: [],
        capabilities: ['set_max_thinking_tokens'],
      },
    ])

    expect(store.panel.phase).toBe('sdk_owned')
    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.sessionId).toBe('new-123')
      // ephemeral = true for create actions
      expect(store.panel.ephemeral).toBe(true)
    }

    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'POST_CREATE', model: 'default' }),
    )
  })
})

// ── Permission flow ───────────────────────────────────────────────

describe('integration: permission flow', () => {
  test('sdk_owned(streaming) → permission request → respond → stream continues', () => {
    const liveStore: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'abc',
        controlId: 'c1',
        blocks: mockBlocks,
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'streaming' },
        conn: { health: 'ok' },
      },
      outbox: { messages: [] },
      meta: {
        model: 'opus',
        permissionMode: 'default',
        slashCommands: [],
        mcpServers: [],
        skills: [],
        agents: [],
        capabilities: [],
        totalInputTokens: 100,
        contextWindowSize: 200000,
      },
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const { store, allCmds } = drive(liveStore, [
      // Permission request → turn becomes awaiting
      { type: 'PERMISSION_REQUEST', kind: 'permission', requestId: 'r1' },
      // User approves → WS_SEND, turn stays awaiting (coordinator doesn't change turn)
      { type: 'RESPOND_PERMISSION', requestId: 'r1', allowed: true },
      // Stream resumes → turn back to streaming
      { type: 'STREAM_DELTA', text: 'continuing...' },
    ])

    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.turn).toEqual({ turn: 'streaming' })
      expect(store.panel.pendingText).toBe('continuing...')
    }

    expect(allCmds).toContainEqual(
      expect.objectContaining({
        cmd: 'WS_SEND',
        message: expect.objectContaining({
          type: 'permission_response',
          requestId: 'r1',
          allowed: true,
        }),
      }),
    )
  })
})

// ── Session closed flow ───────────────────────────────────────────

describe('integration: session close and reopen', () => {
  test('sdk_owned → SESSION_CLOSED → closed → send → acquiring → sdk_owned', () => {
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
      historyPagination: null,
    }

    const { store, allCmds } = drive(liveStore, [
      // Session closes → closed phase
      { type: 'SESSION_CLOSED' },
      // User sends from closed → acquiring(resume)
      { type: 'SEND_MESSAGE', text: 'reopen', localId: 'l10' },
      // Resume succeeds
      { type: 'ACQUIRE_OK', controlId: 'c10' },
      { type: 'WS_OPEN' },
      {
        type: 'SESSION_INIT',
        model: 'opus',
        permissionMode: 'default',
        slashCommands: [],
        mcpServers: [],
        skills: [],
        agents: [],
        capabilities: [],
      },
    ])

    expect(store.panel.phase).toBe('sdk_owned')
    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'CLOSE_SIDECAR_WS' }))
    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'POST_RESUME' }))
  })
})

// ── Deselect from any phase ───────────────────────────────────────

describe('integration: deselect resets everything', () => {
  test('sdk_owned → DESELECT → empty, closes all connections', () => {
    const liveStore: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'abc',
        controlId: 'c1',
        blocks: mockBlocks,
        pendingText: 'partial',
        ephemeral: false,
        turn: { turn: 'streaming' },
        conn: { health: 'ok' },
      },
      outbox: { messages: [{ localId: 'x', text: 'x', status: 'sent' }] },
      meta: {
        model: 'opus',
        permissionMode: 'default',
        slashCommands: [],
        mcpServers: [],
        skills: [],
        agents: [],
        capabilities: [],
        totalInputTokens: 100,
        contextWindowSize: 200000,
      },
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const { store, allCmds } = drive(liveStore, [{ type: 'DESELECT' }])

    expect(store.panel.phase).toBe('empty')
    expect(store.outbox.messages).toEqual([])
    expect(store.meta).toBeNull()
    expect(allCmds).toContainEqual({ cmd: 'CLOSE_SIDECAR_WS' })
    expect(allCmds).toContainEqual({ cmd: 'CLOSE_TERMINAL_WS' })
  })
})

// ── Race: SIDECAR_HAS_SESSION before HISTORY_OK ──────────────────

describe('integration: history race — sidecar responds first', () => {
  test('SIDECAR_HAS_SESSION before HISTORY_OK → history preserved through acquiring → sdk_owned', () => {
    const historyBlocks = [
      { type: 'user' as const, id: 'u1', text: 'old msg', timestamp: 1 },
      { type: 'assistant' as const, id: 'a1', segments: [], streaming: false },
    ] as any

    const { store } = drive(INITIAL, [
      // 1. Select session → nobody(loading)
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      // 2. Sidecar responds FIRST (before history API) → acquiring with blocks: []
      { type: 'SIDECAR_HAS_SESSION', controlId: 'ctrl-1' },
      // 3. History arrives LATE → should update historyBlocks in acquiring
      { type: 'HISTORY_OK', blocks: historyBlocks },
      // 4. WS connects
      { type: 'WS_OPEN' },
      // 5. Session init → exits acquiring → sdk_owned with the history blocks
      {
        type: 'SESSION_INIT',
        model: 'opus',
        permissionMode: 'default',
        slashCommands: [],
        mcpServers: [],
        skills: [],
        agents: [],
        capabilities: [],
      },
    ])

    expect(store.panel.phase).toBe('sdk_owned')
    if (store.panel.phase === 'sdk_owned') {
      // History blocks carried through — not empty
      expect(store.panel.blocks).toHaveLength(2)
      expect(store.panel.blocks[0].id).toBe('u1')
    }
  })
})

// ── Race: HISTORY_OK arrives in sdk_owned ────────────────────────

describe('integration: history arrives after sdk_owned', () => {
  test('HISTORY_OK with empty blocks in sdk_owned → merges history', () => {
    const historyBlocks = [{ type: 'user' as const, id: 'u1', text: 'hello', timestamp: 1 }] as any

    // Start in sdk_owned with empty blocks (race happened)
    const emptyStore: ChatPanelStore = {
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
      historyPagination: null,
    }

    const { store } = drive(emptyStore, [{ type: 'HISTORY_OK', blocks: historyBlocks }])

    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.blocks).toHaveLength(1)
      expect(store.panel.blocks[0].id).toBe('u1')
    }
  })

  test('HISTORY_OK with existing blocks in sdk_owned → ignores stale history', () => {
    const liveBlocks = [
      { type: 'user' as const, id: 'u1', text: 'hello', timestamp: 1 },
      { type: 'assistant' as const, id: 'a1', segments: [], streaming: false },
    ] as any

    const liveStore: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'abc',
        controlId: 'c1',
        blocks: liveBlocks,
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

    const staleHistory = [{ type: 'user' as const, id: 'u1', text: 'hello', timestamp: 1 }] as any

    const { store } = drive(liveStore, [{ type: 'HISTORY_OK', blocks: staleHistory }])

    if (store.panel.phase === 'sdk_owned') {
      // Should NOT replace live blocks with stale history
      expect(store.panel.blocks).toHaveLength(2)
    }
  })
})

// ── stream_delta without textDelta ───────────────────────────────

describe('integration: stream_delta filtering', () => {
  test('stream_delta with null textDelta returns null from mapWsEvent', () => {
    // content_block_start — no textDelta
    expect(mapWsEvent({ type: 'stream_delta', deltaType: 'content_block_start' })).toBeNull()
    // content_block_delta with textDelta
    expect(mapWsEvent({ type: 'stream_delta', textDelta: 'hello' })).toEqual({
      type: 'STREAM_DELTA',
      text: 'hello',
    })
    // explicit undefined textDelta
    expect(mapWsEvent({ type: 'stream_delta', textDelta: undefined })).toBeNull()
  })
})

// ── CLI watching: full lifecycle + streaming ──────────────────────

describe('integration: CLI watching mode lifecycle', () => {
  test('cc_cli(watching) → LIVE_STATUS inactive → nobody(ready)', () => {
    const historyBlocks = [{ type: 'user' as const, id: 'u1', text: 'hello', timestamp: 1 }]
    const watchingStore: ChatPanelStore = {
      panel: {
        phase: 'cc_cli',
        sessionId: 'abc',
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

    const { store, allCmds } = drive(watchingStore, [
      { type: 'LIVE_STATUS_CHANGED', status: 'inactive' },
    ])

    expect(store.panel.phase).toBe('nobody')
    if (store.panel.phase === 'nobody') {
      expect(store.panel.sub).toEqual({ sub: 'ready', blocks: historyBlocks })
    }
    expect(allCmds).toContainEqual({ cmd: 'CLOSE_TERMINAL_WS' })
  })
})

// ── Watching mode streaming: regression protection ───────────────

describe('integration: watching mode streaming', () => {
  // Happy path: history first, then cc_owned, then live blocks stream in
  test('select → history → cc_owned → cc_cli → TERMINAL_BLOCK streams live updates', () => {
    const { store, allCmds } = drive(INITIAL, [
      // 1. Select session → nobody(loading)
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      // 2. History loads → nobody(ready, blocks)
      { type: 'HISTORY_OK', blocks: mockBlocks },
      // 3. SSE says CLI owns it → cc_cli(watching) with history blocks
      { type: 'LIVE_STATUS_CHANGED', status: 'cc_owned' },
      // 4. Terminal WS connected
      { type: 'TERMINAL_CONNECTED' },
      // 5. Live block streams in — new user message
      {
        type: 'TERMINAL_BLOCK',
        block: { type: 'user', id: 'u2', text: 'new msg', timestamp: 2 },
      },
      // 6. Live block streams in — assistant response
      {
        type: 'TERMINAL_BLOCK',
        block: { type: 'assistant', id: 'a1', segments: [], streaming: true, timestamp: 3 },
      },
      // 7. Assistant block updates (same ID = replace, not append)
      {
        type: 'TERMINAL_BLOCK',
        block: {
          type: 'assistant',
          id: 'a1',
          segments: [{ kind: 'text', text: 'Hello!' }],
          streaming: false,
          timestamp: 3,
        },
      },
    ])

    // Final state: cc_cli with 3 blocks (history + 2 streamed)
    expect(store.panel.phase).toBe('cc_cli')
    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.blocks).toHaveLength(3)
      expect(store.panel.blocks[0].id).toBe('1') // original history
      expect(store.panel.blocks[1].id).toBe('u2') // streamed user
      expect(store.panel.blocks[2].id).toBe('a1') // streamed assistant (replaced, not duplicated)
      // Verify the assistant block was replaced, not appended
      const a1 = store.panel.blocks[2]
      if (a1.type === 'assistant') {
        expect(a1.streaming).toBe(false) // final version
      }
    }

    // Commands: FETCH_HISTORY + CHECK_SIDECAR + OPEN_TERMINAL_WS
    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'FETCH_HISTORY' }))
    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'OPEN_TERMINAL_WS' }))
  })

  // Race condition: cc_owned arrives BEFORE history loads
  test('select → cc_owned (race!) → deferred → HISTORY_OK → cc_cli → streaming works', () => {
    const { store: midStore } = drive(INITIAL, [
      // 1. Select session → nobody(loading)
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      // 2. SSE arrives FIRST (race!) — cc_owned while still loading
      { type: 'LIVE_STATUS_CHANGED', status: 'cc_owned' },
    ])

    // Should NOT be cc_cli yet — still nobody with pendingLive
    expect(midStore.panel.phase).toBe('nobody')
    if (midStore.panel.phase === 'nobody' && midStore.panel.sub.sub === 'loading') {
      expect(midStore.panel.sub.pendingLive).toBe('cc_owned')
    }

    // Now history arrives → completes deferred transition
    const { store, allCmds } = drive(midStore, [
      { type: 'HISTORY_OK', blocks: mockBlocks },
      // Terminal WS connected, then live blocks
      { type: 'TERMINAL_CONNECTED' },
      { type: 'TERMINAL_BLOCK', block: { type: 'user', id: 'u2', text: 'live!', timestamp: 2 } },
    ])

    expect(store.panel.phase).toBe('cc_cli')
    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.blocks).toHaveLength(2) // history + streamed
      expect(store.panel.blocks[0].id).toBe('1') // from HISTORY_OK
      expect(store.panel.blocks[1].id).toBe('u2') // from TERMINAL_BLOCK
    }
    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'OPEN_TERMINAL_WS' }))
  })

  // Race condition: HISTORY_FAILED with pendingLive → cc_cli with empty blocks
  test('select → cc_owned (race!) → HISTORY_FAILED → cc_cli with empty blocks', () => {
    const { store } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'LIVE_STATUS_CHANGED', status: 'cc_owned' },
      { type: 'HISTORY_FAILED', error: 'not found' },
    ])

    // Still transitions to cc_cli (empty blocks is better than stuck in nobody)
    expect(store.panel.phase).toBe('cc_cli')
    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.blocks).toHaveLength(0)
    }
  })

  // TERMINAL_BLOCK in non-cc_cli phase → ignored (no-op)
  test('TERMINAL_BLOCK in nobody phase → no-op', () => {
    const nobodyStore: ChatPanelStore = {
      panel: { phase: 'nobody', sessionId: 'abc', sub: { sub: 'ready', blocks: mockBlocks } },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const { store } = drive(nobodyStore, [
      { type: 'TERMINAL_BLOCK', block: { type: 'user', id: 'u2', text: 'ignored', timestamp: 2 } },
    ])

    // Block should NOT be added — wrong phase
    if (store.panel.phase === 'nobody' && store.panel.sub.sub === 'ready') {
      expect(store.panel.sub.blocks).toHaveLength(1) // unchanged
    }
  })

  // Full lifecycle: start watching → stream → CLI ends → back to history
  test('full lifecycle: select → watch → stream → CLI exits → nobody preserves all blocks', () => {
    const { store } = drive(INITIAL, [
      { type: 'SELECT_SESSION', sessionId: 'abc' },
      { type: 'HISTORY_OK', blocks: mockBlocks },
      { type: 'LIVE_STATUS_CHANGED', status: 'cc_owned' },
      { type: 'TERMINAL_CONNECTED' },
      // Stream some blocks
      { type: 'TERMINAL_BLOCK', block: { type: 'user', id: 'u2', text: 'msg2', timestamp: 2 } },
      { type: 'TERMINAL_BLOCK', block: { type: 'user', id: 'u3', text: 'msg3', timestamp: 3 } },
      // CLI session ends
      { type: 'LIVE_STATUS_CHANGED', status: 'inactive' },
    ])

    // Back to nobody — all blocks (history + streamed) preserved
    expect(store.panel.phase).toBe('nobody')
    if (store.panel.phase === 'nobody' && store.panel.sub.sub === 'ready') {
      expect(store.panel.sub.blocks).toHaveLength(3) // 1 history + 2 streamed
      expect(store.panel.sub.blocks.map((b) => b.id)).toEqual(['1', 'u2', 'u3'])
    }
  })

  // Rapid merge-by-ID: same block updated 3 times → only last version kept
  test('rapid TERMINAL_BLOCK updates for same ID → replaced each time, no duplicates', () => {
    const watchingStore: ChatPanelStore = {
      panel: { phase: 'cc_cli', sessionId: 'abc', blocks: [], sub: { sub: 'watching' } },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const { store } = drive(watchingStore, [
      {
        type: 'TERMINAL_BLOCK',
        block: { type: 'assistant', id: 'a1', segments: [], streaming: true, timestamp: 1 },
      },
      {
        type: 'TERMINAL_BLOCK',
        block: {
          type: 'assistant',
          id: 'a1',
          segments: [{ kind: 'text', text: 'Hel' }],
          streaming: true,
          timestamp: 1,
        },
      },
      {
        type: 'TERMINAL_BLOCK',
        block: {
          type: 'assistant',
          id: 'a1',
          segments: [{ kind: 'text', text: 'Hello!' }],
          streaming: false,
          timestamp: 1,
        },
      },
    ])

    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.blocks).toHaveLength(1) // NOT 3
      const block = store.panel.blocks[0]
      if (block.type === 'assistant') {
        expect(block.streaming).toBe(false) // final version
        expect(block.segments).toEqual([{ kind: 'text', text: 'Hello!' }])
      }
    }
  })

  // Multiple different block types streaming concurrently
  test('mixed block types stream and maintain order', () => {
    const watchingStore: ChatPanelStore = {
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

    const { store } = drive(watchingStore, [
      // biome-ignore lint/suspicious/noExplicitAny: test fixture — ProgressData/SystemData shapes are complex
      {
        type: 'TERMINAL_BLOCK',
        block: { type: 'progress', id: 'p1', variant: 'hook', category: 'hook', data: {} } as any,
      },
      // biome-ignore lint/suspicious/noExplicitAny: test fixture
      {
        type: 'TERMINAL_BLOCK',
        block: { type: 'system', id: 's1', variant: 'file_history_snapshot', data: {} } as any,
      },
      {
        type: 'TERMINAL_BLOCK',
        block: { type: 'user', id: 'u2', text: 'next prompt', timestamp: 2 },
      },
      {
        type: 'TERMINAL_BLOCK',
        block: {
          type: 'assistant',
          id: 'a1',
          segments: [{ kind: 'text', text: 'response' }],
          streaming: false,
          timestamp: 3,
        },
      },
    ])

    if (store.panel.phase === 'cc_cli') {
      expect(store.panel.blocks).toHaveLength(5) // 1 history + 4 streamed
      expect(store.panel.blocks.map((b) => b.id)).toEqual(['1', 'p1', 's1', 'u2', 'a1'])
    }
  })
})

// ── REGRESSION: BLOCKS_UPDATE must clear pendingText ──────────────

describe('integration: BLOCKS_UPDATE clears pendingText (no doubled assistant text)', () => {
  test('STREAM_DELTA accumulates, then BLOCKS_UPDATE clears pendingText', () => {
    // Start in sdk_owned.streaming with some pendingText
    const liveStore: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 's1',
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
      historyPagination: null,
    }

    // 1. STREAM_DELTA accumulates text
    const { store: s1 } = drive(liveStore, [
      { type: 'STREAM_DELTA', text: 'Hello ' },
      { type: 'STREAM_DELTA', text: 'world!' },
    ])
    if (s1.panel.phase === 'sdk_owned') {
      expect(s1.panel.pendingText).toBe('Hello world!')
      expect(s1.panel.turn.turn).toBe('streaming')
    }

    // 2. BLOCKS_UPDATE brings the same text in server blocks → clears pendingText
    const assistantBlock = {
      type: 'assistant' as const,
      id: 'msg-1',
      segments: [{ kind: 'text' as const, text: 'Hello world!' }],
      streaming: true,
      timestamp: 1,
    }
    const { store: s2 } = drive(s1, [{ type: 'BLOCKS_UPDATE', blocks: [assistantBlock] }])
    if (s2.panel.phase === 'sdk_owned') {
      // CRITICAL: pendingText must be cleared — server blocks are authoritative
      expect(s2.panel.pendingText).toBe('')
      // Server blocks have the text
      expect(s2.panel.blocks).toHaveLength(1)
      expect(s2.panel.blocks[0].id).toBe('msg-1')
    }
  })

  test('deriveBlocks does NOT produce duplicate assistant blocks after BLOCKS_UPDATE', () => {
    // Simulates the bug: pendingText + server blocks both had the same text
    // After BLOCKS_UPDATE clears pendingText, only server blocks are shown
    const store: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 's1',
        controlId: 'c1',
        blocks: [
          {
            type: 'assistant',
            id: 'msg-1',
            segments: [{ kind: 'text', text: 'Hello world!' }],
            streaming: true,
            timestamp: 1,
          },
        ],
        pendingText: '', // Cleared by BLOCKS_UPDATE
        ephemeral: false,
        turn: { turn: 'streaming' },
        conn: { health: 'ok' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const blocks = deriveBlocks(store)
    const assistantBlocks = blocks.filter((b) => b.type === 'assistant')

    // Must have exactly ONE assistant block, not two (no __pending__ duplicate)
    expect(assistantBlocks).toHaveLength(1)
    expect(assistantBlocks[0].id).toBe('msg-1')
  })

  test('BLOCKS_SNAPSHOT also clears pendingText', () => {
    const liveStore: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 's1',
        controlId: 'c1',
        blocks: [],
        pendingText: 'stale text from before reconnect',
        ephemeral: false,
        turn: { turn: 'streaming' },
        conn: { health: 'ok' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }

    const { store } = drive(liveStore, [
      {
        type: 'BLOCKS_SNAPSHOT',
        blocks: [
          {
            type: 'assistant',
            id: 'msg-snap',
            segments: [{ kind: 'text', text: 'fresh from snapshot' }],
            streaming: false,
            timestamp: 1,
          },
        ],
      },
    ])

    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.pendingText).toBe('')
      expect(store.panel.blocks).toHaveLength(1)
    }
  })
})

// ── REGRESSION: exitAcquiring must enter 'pending' when pendingMessage is set ──
// Bug: outbox entries are pre-marked 'sent' in handle-empty/handle-nobody before
// entering acquiring. Checking outbox status === 'queued' always found zero →
// always entered 'idle' → 1-3s dead zone with no ThinkingIndicator.
// Fix: check p.pendingMessage (FSM's own field), not outbox status.

describe('integration: exitAcquiring enters pending turn when message was sent', () => {
  const mockInitEvent = {
    type: 'SESSION_INIT' as const,
    model: 'opus',
    permissionMode: 'default',
    slashCommands: [] as string[],
    mcpServers: [] as { name: string; status: string }[],
    skills: [] as string[],
    agents: [] as string[],
    capabilities: [] as string[],
  }

  test('new session (empty → create): outbox pre-marked sent, but pendingMessage → pending', () => {
    const { store } = drive(INITIAL, [
      { type: 'SEND_MESSAGE', text: 'hello', localId: 'l1' },
      { type: 'ACQUIRE_OK', controlId: 'c1', sessionId: 'new-1' },
      { type: 'WS_OPEN' },
      mockInitEvent,
    ])
    expect(store.panel.phase).toBe('sdk_owned')
    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.turn).toEqual({ turn: 'pending' })
    }
    // Outbox was pre-marked 'sent' — NOT 'queued'
    expect(store.outbox.messages[0]?.status).toBe('sent')
  })

  test('resume with message (nobody → resume): pendingMessage → pending', () => {
    const nobodyStore: ChatPanelStore = {
      panel: {
        phase: 'nobody',
        sessionId: 'abc',
        sub: { sub: 'ready', blocks: mockBlocks },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }
    const { store } = drive(nobodyStore, [
      { type: 'SEND_MESSAGE', text: 'continue', localId: 'l2' },
      { type: 'ACQUIRE_OK', controlId: 'c2' },
      { type: 'WS_OPEN' },
      mockInitEvent,
    ])
    expect(store.panel.phase).toBe('sdk_owned')
    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.turn).toEqual({ turn: 'pending' })
    }
  })

  test('resume without message (sidecar active): no pendingMessage → idle', () => {
    const nobodyStore: ChatPanelStore = {
      panel: {
        phase: 'nobody',
        sessionId: 'abc',
        sub: { sub: 'ready', blocks: mockBlocks },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }
    const { store } = drive(nobodyStore, [
      // Sidecar already active — no user message, just WS connect
      { type: 'SIDECAR_HAS_SESSION', controlId: 'c3' },
      { type: 'WS_OPEN' },
      mockInitEvent,
    ])
    expect(store.panel.phase).toBe('sdk_owned')
    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.turn).toEqual({ turn: 'idle' })
    }
  })

  test('SEND_MESSAGE in sdk_owned.idle → pending', () => {
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
      historyPagination: null,
    }
    const [store] = coordinate(liveStore, {
      type: 'SEND_MESSAGE',
      text: 'next question',
      localId: 'l3',
    })
    expect(store.panel.phase).toBe('sdk_owned')
    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.turn).toEqual({ turn: 'pending' })
    }
  })

  test('pending → streaming on STREAM_DELTA', () => {
    const pendingStore: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'abc',
        controlId: 'c1',
        blocks: [],
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'pending' },
        conn: { health: 'ok' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }
    const [store] = coordinate(pendingStore, { type: 'STREAM_DELTA', text: 'Hi' })
    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.turn).toEqual({ turn: 'streaming' })
    }
  })

  test('pending → idle on TURN_COMPLETE (empty response)', () => {
    const pendingStore: ChatPanelStore = {
      panel: {
        phase: 'sdk_owned',
        sessionId: 'abc',
        controlId: 'c1',
        blocks: [],
        pendingText: '',
        ephemeral: false,
        turn: { turn: 'pending' },
        conn: { health: 'ok' },
      },
      outbox: { messages: [] },
      meta: null,
      projectPath: null,
      lastModel: null,
      lastPermissionMode: null,
      historyPagination: null,
    }
    const [store] = coordinate(pendingStore, {
      type: 'TURN_COMPLETE',
      blocks: [],
      totalInputTokens: 100,
      contextWindowSize: 200000,
    })
    if (store.panel.phase === 'sdk_owned') {
      expect(store.panel.turn).toEqual({ turn: 'idle' })
    }
  })
})
