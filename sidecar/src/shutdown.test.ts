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
