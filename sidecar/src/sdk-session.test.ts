// sidecar/src/sdk-session.test.ts
// Tests for waitForSessionInit — the async gate that prevents returning
// empty sessionId from the create route.

import { EventEmitter } from 'node:events'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { MessageBridge } from './message-bridge.js'
import { waitForSessionInit } from './sdk-session.js'
import type { ControlSession } from './session-registry.js'

function makeStubCs(overrides: Partial<ControlSession> = {}): ControlSession {
  return {
    controlId: 'ctrl-test',
    sessionId: '',
    model: 'claude-haiku-4-5-20251001',
    // biome-ignore lint/suspicious/noExplicitAny: stub for testing
    query: {} as any,
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
    permissions: {} as any,
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
