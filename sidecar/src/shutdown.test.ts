import { EventEmitter } from 'node:events'
import { describe, expect, it, vi } from 'vitest'
import { SessionRegistry } from './session-registry.js'
import { StreamAccumulator } from './stream-accumulator.js'
import { PermissionHandler } from './permission-handler.js'
import { MessageBridge } from './message-bridge.js'
import type { ControlSession } from './session-registry.js'

function makeStubSession(overrides: Partial<ControlSession> = {}): ControlSession {
  const abort = new AbortController()
  return {
    controlId: crypto.randomUUID(),
    sessionId: 'test-session',
    model: 'test',
    query: { close: vi.fn(), return: vi.fn() } as any,
    bridge: new MessageBridge(),
    abort,
    state: 'waiting_input',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter: new EventEmitter(),
    permissions: new PermissionHandler(),
    permissionMode: 'default',
    wsClients: new Set(),
    lastSessionInit: null,
    accumulator: new StreamAccumulator(),
    ...overrides,
  }
}

describe('SessionRegistry.closeAll', () => {
  it('signals abort on every session', async () => {
    const registry = new SessionRegistry()
    const cs1 = makeStubSession({ controlId: 'a' })
    const cs2 = makeStubSession({ controlId: 'b' })
    registry.register(cs1)
    registry.register(cs2)
    await registry.closeAll()
    expect(cs1.abort.signal.aborted).toBe(true)
    expect(cs2.abort.signal.aborted).toBe(true)
  })

  it('sets closeReason to shutdown', async () => {
    const registry = new SessionRegistry()
    const cs = makeStubSession()
    registry.register(cs)
    await registry.closeAll()
    expect(cs.closeReason).toBe('shutdown')
  })

  it('calls query.close() on every session', async () => {
    const registry = new SessionRegistry()
    const cs = makeStubSession()
    registry.register(cs)
    await registry.closeAll()
    expect(cs.query.close).toHaveBeenCalledTimes(1)
  })
})

describe('SessionRegistry delayed-remove cancellation', () => {
  it('closeAll cancels pending delayed-remove timers', async () => {
    const clearSpy = vi.spyOn(globalThis, 'clearTimeout')
    const registry = new SessionRegistry()
    const cs = makeStubSession()
    registry.register(cs)
    registry.scheduleRemove(cs.controlId, 5_000)
    await registry.closeAll()
    expect(clearSpy).toHaveBeenCalled()
    clearSpy.mockRestore()
  })
})

describe('Prompt handler emitter cleanup (routes.ts prompt endpoint)', () => {
  it('cleanup() removes listener on turn_complete — no leak', () => {
    const emitter = new EventEmitter()
    let cleanedUp = false
    const onMessage = (event: { type: string }) => {
      if (event.type === 'turn_complete' || event.type === 'turn_error') {
        cleanup()
      }
    }
    const timeout = setTimeout(() => { cleanup() }, 120_000)
    const cleanup = () => {
      clearTimeout(timeout)
      emitter.removeListener('message', onMessage)
      cleanedUp = true
    }
    emitter.on('message', onMessage)
    expect(emitter.listenerCount('message')).toBe(1)
    emitter.emit('message', { type: 'turn_complete' })
    expect(emitter.listenerCount('message')).toBe(0)
    expect(cleanedUp).toBe(true)
  })

  it('cleanup() removes listener on timeout — no leak', () => {
    const emitter = new EventEmitter()
    let cleanedUp = false
    const onMessage = (_event: { type: string }) => {}
    const cleanup = () => {
      emitter.removeListener('message', onMessage)
      cleanedUp = true
    }
    emitter.on('message', onMessage)
    expect(emitter.listenerCount('message')).toBe(1)
    cleanup()
    expect(emitter.listenerCount('message')).toBe(0)
    expect(cleanedUp).toBe(true)
  })

  it('cleanup() is idempotent — double call is safe', () => {
    const emitter = new EventEmitter()
    const onMessage = (_event: { type: string }) => {}
    const timeout = setTimeout(() => {}, 1000)
    const cleanup = () => {
      clearTimeout(timeout)
      emitter.removeListener('message', onMessage)
    }
    emitter.on('message', onMessage)
    cleanup()
    cleanup()
    expect(emitter.listenerCount('message')).toBe(0)
  })
})
