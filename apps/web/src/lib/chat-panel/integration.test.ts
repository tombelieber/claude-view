import { describe, expect, test } from 'vitest'
import { coordinate } from './coordinator'
import { mapWsEvent } from './event-mapper'
import type { ChatPanelStore, Command, RawEvent } from './types'

const INITIAL: ChatPanelStore = {
  panel: { phase: 'empty' },
  outbox: { messages: [] },
  meta: null,
  projectPath: null,
}

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
      // 8. Session init → exits acquiring → sdk_owned(idle), emits CANCEL_TIMER + WS_SEND (queued msg)
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

    // Meta: SESSION_INIT during acquiring does NOT populate meta (consumed by acquiring leaf).
    // Meta stays null until a SESSION_INIT arrives while in sdk_owned.
    // But TURN_COMPLETE does update meta via TURN_USAGE if meta is non-null.
    // Since meta is null after acquiring, TURN_USAGE on null meta returns null.
    expect(store.meta).toBeNull()

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
      expect(store.panel.turn).toEqual({ turn: 'idle' })
    }

    expect(allCmds).toContainEqual(expect.objectContaining({ cmd: 'POST_FORK', sessionId: 'abc' }))
  })
})

// ── Takeover flow ─────────────────────────────────────────────────

describe('integration: takeover flow', () => {
  test('cc_cli(watching) → takeover → kill OK → resume → WS → SESSION_INIT → sdk_owned', () => {
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
    }

    const { store, allCmds } = drive(watchingStore, [
      // User clicks takeover → cc_cli(takeover_killing), emits KILL_CLI_SESSION
      { type: 'TAKEOVER_CLI' },
      // CLI killed → acquiring(posting), emits POST_RESUME
      { type: 'KILL_CLI_OK' },
      // Resume OK → acquiring(ws_connecting)
      { type: 'ACQUIRE_OK', controlId: 'c3' },
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
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'KILL_CLI_SESSION', sessionId: 'abc' }),
    )
    expect(allCmds).toContainEqual(
      expect.objectContaining({ cmd: 'POST_RESUME', sessionId: 'abc' }),
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

// ── CLI watching → inactive → back to nobody ──────────────────────

describe('integration: CLI session ends naturally', () => {
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
