// sidecar/src/sdk-session.test.ts
// Comprehensive V1 unit tests for sdk-session: waitForSessionInit, sendMessage,
// closeSession, setSessionMode, and lifecycle integration.

import { EventEmitter } from 'node:events'
import type { Query } from '@anthropic-ai/claude-agent-sdk'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { MessageBridge } from './message-bridge.js'
import { closeSession, sendMessage, setSessionMode, waitForSessionInit } from './sdk-session.js'
import { SessionRegistry } from './session-registry.js'
import type { ControlSession } from './session-registry.js'

type MockQuery = Query & { unblock: () => void }

function makeMockQuery(): MockQuery {
  let blockResolve: (() => void) | null = null
  const mock = {
    [Symbol.asyncIterator]() {
      return this
    },
    async next() {
      // Block indefinitely until unblock() or close() is called
      return new Promise<IteratorResult<unknown>>((resolve) => {
        blockResolve = () => resolve({ done: true, value: undefined })
      })
    },
    close: vi.fn(() => {
      blockResolve?.()
    }),
    unblock: () => {
      blockResolve?.()
    },
    return: vi.fn().mockResolvedValue({ done: true, value: undefined }),
    throw: vi.fn().mockResolvedValue({ done: true, value: undefined }),
    interrupt: vi.fn().mockResolvedValue(undefined),
    setPermissionMode: vi.fn().mockResolvedValue(undefined),
    setModel: vi.fn().mockResolvedValue(undefined),
    setMaxThinkingTokens: vi.fn().mockResolvedValue(undefined),
    supportedModels: vi.fn().mockResolvedValue([]),
    supportedCommands: vi.fn().mockResolvedValue([]),
    supportedAgents: vi.fn().mockResolvedValue([]),
    mcpServerStatus: vi.fn().mockResolvedValue([]),
    accountInfo: vi.fn().mockResolvedValue({}),
    rewindFiles: vi.fn().mockResolvedValue({}),
    reconnectMcpServer: vi.fn().mockResolvedValue(undefined),
    toggleMcpServer: vi.fn().mockResolvedValue(undefined),
    setMcpServers: vi.fn().mockResolvedValue({}),
    stopTask: vi.fn().mockResolvedValue(undefined),
    streamInput: vi.fn().mockResolvedValue(undefined),
    initializationResult: vi.fn().mockResolvedValue({}),
  } as unknown as MockQuery
  return mock
}

function makeStubCs(overrides: Partial<ControlSession> = {}): ControlSession {
  return {
    controlId: 'ctrl-test',
    sessionId: '',
    model: 'claude-haiku-4-5-20251001',
    query: makeMockQuery(),
    bridge: new MessageBridge(),
    abort: new AbortController(),
    closeReason: undefined,
    state: 'initializing',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter: new EventEmitter(),
    // biome-ignore lint/suspicious/noExplicitAny: stub for testing
    eventBuffer: { push: vi.fn() } as any,
    nextSeq: 0,
    // biome-ignore lint/suspicious/noExplicitAny: stub for testing
    permissions: { drainAll: vi.fn() } as any,
    permissionMode: 'default',
    activeWs: null,
    ...overrides,
  }
}

describe('waitForSessionInit', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  // --- Unit: resolves immediately if sessionId already populated ---
  it('resolves immediately when sessionId is already set', async () => {
    const cs = makeStubCs({ sessionId: 'existing-session-id' })
    await waitForSessionInit(cs)
    // No error — resolved synchronously
  })

  // --- Unit: resolves when session_id_ready event fires ---
  it('resolves when session_id_ready event fires and sessionId is populated', async () => {
    const cs = makeStubCs()
    const promise = waitForSessionInit(cs)

    // Simulate: SDK stream emits a message with session_id, updateSessionStateFromRawMsg
    // sets cs.sessionId and emits 'session_id_ready'
    setTimeout(() => {
      cs.sessionId = 'new-session-uuid'
      cs.emitter.emit('session_id_ready', 'new-session-uuid')
    }, 10)
    vi.advanceTimersByTime(10)

    await promise
    expect(cs.sessionId).toBe('new-session-uuid')
  })

  // --- Unit: rejects on fatal error event ---
  it('rejects when a fatal error event fires before session_id_ready', async () => {
    const cs = makeStubCs()
    const promise = waitForSessionInit(cs)

    setTimeout(() => {
      cs.emitter.emit('message', {
        type: 'error',
        message: 'authentication_failed',
        fatal: true,
        seq: 0,
      })
    }, 10)
    vi.advanceTimersByTime(10)

    await expect(promise).rejects.toThrow('authentication_failed')
  })

  // --- Unit: rejects on timeout ---
  it('rejects after timeout when session_id_ready never fires', async () => {
    const cs = makeStubCs()
    const promise = waitForSessionInit(cs, 100) // 100ms timeout

    vi.advanceTimersByTime(100)

    await expect(promise).rejects.toThrow('timed out')
  })

  // --- Edge: ignores non-fatal error events, waits for session_id_ready ---
  it('ignores non-fatal error events and keeps waiting', async () => {
    const cs = makeStubCs()
    const promise = waitForSessionInit(cs)

    // Non-fatal error — should be ignored
    setTimeout(() => {
      cs.emitter.emit('message', {
        type: 'error',
        message: 'rate limit warning',
        fatal: false,
        seq: 0,
      })
    }, 5)

    // session_id_ready fires after the non-fatal error
    setTimeout(() => {
      cs.sessionId = 'session-after-warning'
      cs.emitter.emit('session_id_ready', 'session-after-warning')
    }, 10)
    vi.advanceTimersByTime(10)

    await promise
    expect(cs.sessionId).toBe('session-after-warning')
  })

  // --- Edge: ignores unrelated event types ---
  it('ignores assistant_text and other unrelated events', async () => {
    const cs = makeStubCs()
    const promise = waitForSessionInit(cs)

    // Unrelated events — should be ignored
    setTimeout(() => {
      cs.emitter.emit('message', { type: 'assistant_text', text: 'hello', seq: 0 })
      cs.emitter.emit('message', { type: 'tool_use_start', toolName: 'Read', seq: 1 })
    }, 5)

    setTimeout(() => {
      cs.sessionId = 'session-id'
      cs.emitter.emit('session_id_ready', 'session-id')
    }, 10)
    vi.advanceTimersByTime(10)

    await promise
  })

  // --- Regression: listener cleanup after resolve ---
  it('removes emitter listeners after resolving', async () => {
    const cs = makeStubCs()
    const sessionIdListenersBefore = cs.emitter.listenerCount('session_id_ready')
    const messageListenersBefore = cs.emitter.listenerCount('message')

    const promise = waitForSessionInit(cs)
    expect(cs.emitter.listenerCount('session_id_ready')).toBe(sessionIdListenersBefore + 1)
    expect(cs.emitter.listenerCount('message')).toBe(messageListenersBefore + 1)

    cs.sessionId = 'sid'
    cs.emitter.emit('session_id_ready', 'sid')
    await promise

    expect(cs.emitter.listenerCount('session_id_ready')).toBe(sessionIdListenersBefore)
    expect(cs.emitter.listenerCount('message')).toBe(messageListenersBefore)
  })

  // --- Regression: listener cleanup after reject (fatal error) ---
  it('removes emitter listeners after rejecting on fatal error', async () => {
    const cs = makeStubCs()
    const sessionIdListenersBefore = cs.emitter.listenerCount('session_id_ready')
    const messageListenersBefore = cs.emitter.listenerCount('message')

    const promise = waitForSessionInit(cs)
    expect(cs.emitter.listenerCount('session_id_ready')).toBe(sessionIdListenersBefore + 1)
    expect(cs.emitter.listenerCount('message')).toBe(messageListenersBefore + 1)

    cs.emitter.emit('message', { type: 'error', message: 'boom', fatal: true, seq: 0 })
    await expect(promise).rejects.toThrow('boom')

    expect(cs.emitter.listenerCount('session_id_ready')).toBe(sessionIdListenersBefore)
    expect(cs.emitter.listenerCount('message')).toBe(messageListenersBefore)
  })

  // --- Regression: listener cleanup after timeout ---
  it('removes emitter listeners after timeout', async () => {
    const cs = makeStubCs()
    const sessionIdListenersBefore = cs.emitter.listenerCount('session_id_ready')
    const messageListenersBefore = cs.emitter.listenerCount('message')

    const promise = waitForSessionInit(cs, 50)
    expect(cs.emitter.listenerCount('session_id_ready')).toBe(sessionIdListenersBefore + 1)
    expect(cs.emitter.listenerCount('message')).toBe(messageListenersBefore + 1)

    vi.advanceTimersByTime(50)
    await expect(promise).rejects.toThrow('timed out')

    expect(cs.emitter.listenerCount('session_id_ready')).toBe(sessionIdListenersBefore)
    expect(cs.emitter.listenerCount('message')).toBe(messageListenersBefore)
  })

  // --- Regression: fatal error with no message field ---
  it('rejects with fallback message when fatal error has no message field', async () => {
    const cs = makeStubCs()
    const promise = waitForSessionInit(cs)

    setTimeout(() => {
      cs.emitter.emit('message', { type: 'error', fatal: true, seq: 0 })
    }, 10)
    vi.advanceTimersByTime(10)

    await expect(promise).rejects.toThrow('Session init failed')
  })

  // --- Regression: empty string sessionId treated as not-set ---
  it('waits for session_id_ready when sessionId is empty string', async () => {
    const cs = makeStubCs({ sessionId: '' })
    const promise = waitForSessionInit(cs)

    setTimeout(() => {
      cs.sessionId = 'populated'
      cs.emitter.emit('session_id_ready', 'populated')
    }, 10)
    vi.advanceTimersByTime(10)

    await promise
    expect(cs.sessionId).toBe('populated')
  })

  // --- Edge: rapid session_id_ready before any timers tick ---
  it('resolves when session_id_ready fires synchronously after listen', async () => {
    const cs = makeStubCs()

    // Pre-register an immediate emitter that fires session_id_ready
    // when waitForSessionInit attaches its listener
    const origOn = cs.emitter.on.bind(cs.emitter)
    cs.emitter.on = (event: string, handler: (...args: unknown[]) => void) => {
      origOn(event, handler)
      if (event === 'session_id_ready') {
        cs.sessionId = 'instant-id'
        cs.emitter.emit('session_id_ready', 'instant-id')
      }
      return cs.emitter
    }

    await waitForSessionInit(cs)
    expect(cs.sessionId).toBe('instant-id')
  })
})

describe('sendMessage', () => {
  it('pushes user message to bridge and sets state to active', async () => {
    const cs = makeStubCs({ sessionId: 'sess-1', state: 'waiting_input' })
    sendMessage(cs, 'Hello, Claude')

    expect(cs.state).toBe('active')

    // Bridge should yield the pushed message
    const result = await cs.bridge.next()
    expect(result.done).toBe(false)
    expect(result.value).toEqual({
      type: 'user',
      session_id: 'sess-1',
      message: {
        role: 'user',
        content: [{ type: 'text', text: 'Hello, Claude' }],
      },
      parent_tool_use_id: null,
    })
  })

  it('uses current sessionId in the message', async () => {
    const cs = makeStubCs({ sessionId: 'abc-123' })
    sendMessage(cs, 'test')

    const result = await cs.bridge.next()
    expect(result.value.session_id).toBe('abc-123')
  })

  it('sends multiple messages sequentially through bridge', async () => {
    const cs = makeStubCs({ sessionId: 'sess-1' })
    sendMessage(cs, 'first')
    sendMessage(cs, 'second')

    const r1 = await cs.bridge.next()
    const r2 = await cs.bridge.next()
    expect(r1.value.message.content[0].text).toBe('first')
    expect(r2.value.message.content[0].text).toBe('second')
  })
})

describe('closeSession', () => {
  it('sets closeReason to user_closed', () => {
    const registry = new SessionRegistry()
    const cs = makeStubCs({ sessionId: 'sess-1', state: 'waiting_input' })
    registry.register(cs)

    closeSession(cs, registry)

    expect(cs.closeReason).toBe('user_closed')
  })

  it('closes the bridge', () => {
    const registry = new SessionRegistry()
    const cs = makeStubCs({ sessionId: 'sess-1' })
    registry.register(cs)

    closeSession(cs, registry)

    // Bridge should be closed — next() returns done: true
    return cs.bridge.next().then((result) => {
      expect(result.done).toBe(true)
    })
  })

  it('calls query.return() to terminate the async generator', () => {
    const registry = new SessionRegistry()
    const mockQuery = makeMockQuery()
    const cs = makeStubCs({ query: mockQuery, sessionId: 'sess-1' })
    registry.register(cs)

    closeSession(cs, registry)

    expect(mockQuery.return).toHaveBeenCalledWith(undefined)
  })

  it('drains pending permissions', () => {
    const registry = new SessionRegistry()
    const drainAll = vi.fn()
    const cs = makeStubCs({
      sessionId: 'sess-1',
      // biome-ignore lint/suspicious/noExplicitAny: stub for testing
      permissions: { drainAll } as any,
    })
    registry.register(cs)

    closeSession(cs, registry)

    expect(drainAll).toHaveBeenCalled()
  })

  it('does not emit session_closed directly (left to runStreamLoop)', () => {
    const registry = new SessionRegistry()
    const cs = makeStubCs({ sessionId: 'sess-1' })
    registry.register(cs)

    const messages: unknown[] = []
    cs.emitter.on('message', (msg) => messages.push(msg))

    closeSession(cs, registry)

    // closeSession does NOT emit session_closed — runStreamLoop handles it
    const closedEvents = messages.filter(
      (m) =>
        typeof m === 'object' && m !== null && (m as { type: string }).type === 'session_closed',
    )
    expect(closedEvents).toHaveLength(0)
  })
})

describe('setSessionMode', () => {
  it('calls query.setPermissionMode when state is not active', async () => {
    const registry = new SessionRegistry()
    const mockQuery = makeMockQuery()
    const cs = makeStubCs({
      query: mockQuery,
      sessionId: 'sess-1',
      state: 'waiting_input',
    })
    registry.register(cs)

    const result = await setSessionMode(cs, 'plan', registry)

    expect(mockQuery.setPermissionMode).toHaveBeenCalledWith('plan')
    expect(result).toEqual({ ok: true, currentMode: 'plan' })
    expect(cs.permissionMode).toBe('plan')
  })

  it('emits error and returns failure when state is active', async () => {
    const registry = new SessionRegistry()
    const mockQuery = makeMockQuery()
    const cs = makeStubCs({
      query: mockQuery,
      sessionId: 'sess-1',
      state: 'active',
    })
    registry.register(cs)

    const messages: unknown[] = []
    cs.emitter.on('message', (msg) => messages.push(msg))

    const result = await setSessionMode(cs, 'plan', registry)

    // Should NOT have called setPermissionMode
    expect(mockQuery.setPermissionMode).not.toHaveBeenCalled()

    // Should have emitted a non-fatal error via registry.emitSequenced
    expect(messages).toHaveLength(1)
    const errorMsg = messages[0] as { type: string; fatal: boolean; message: string }
    expect(errorMsg.type).toBe('error')
    expect(errorMsg.fatal).toBe(false)
    expect(errorMsg.message).toContain('Cannot change mode')

    // Should return failure with current mode unchanged
    expect(result).toEqual({ ok: false, currentMode: 'default' })
    expect(cs.permissionMode).toBe('default')
  })

  it('allows mode change in waiting_permission state', async () => {
    const registry = new SessionRegistry()
    const mockQuery = makeMockQuery()
    const cs = makeStubCs({
      query: mockQuery,
      sessionId: 'sess-1',
      state: 'waiting_permission',
    })
    registry.register(cs)

    await setSessionMode(cs, 'auto', registry)

    expect(mockQuery.setPermissionMode).toHaveBeenCalledWith('auto')
  })

  it('allows mode change in initializing state', async () => {
    const registry = new SessionRegistry()
    const mockQuery = makeMockQuery()
    const cs = makeStubCs({
      query: mockQuery,
      sessionId: 'sess-1',
      state: 'initializing',
    })
    registry.register(cs)

    await setSessionMode(cs, 'default', registry)

    expect(mockQuery.setPermissionMode).toHaveBeenCalledWith('default')
  })
})
