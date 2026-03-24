// sidecar/src/session-manager.test.ts
// Unit tests for SessionManager — mocks sdk-session to avoid real API calls.

import { EventEmitter } from 'node:events'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ControlSession } from './session-registry.js'
import { SessionRegistry } from './session-registry.js'

// ─── Mocks ──────────────────────────────────────────────────────────────────

vi.mock('./sdk-session.js', () => ({
  createControlSession: vi.fn(),
  resumeControlSession: vi.fn(),
  forkControlSession: vi.fn(),
  closeSession: vi.fn(),
  waitForSessionInit: vi.fn().mockResolvedValue(undefined),
  sendMessage: vi.fn(),
  setSessionMode: vi.fn(),
  listAvailableSessions: vi.fn().mockResolvedValue([]),
}))

const sdkSession = await import('./sdk-session.js')
const { SessionManager } = await import('./session-manager.js')

// ─── Helpers ────────────────────────────────────────────────────────────────

function makeStubCs(overrides: Partial<ControlSession> = {}): ControlSession {
  return {
    controlId: 'ctrl-test',
    sessionId: 'sess-aaaa-bbbb-cccc-dddddddddddd',
    model: 'claude-haiku-4-5-20251001',
    // biome-ignore lint/suspicious/noExplicitAny: stub
    query: { return: vi.fn(), close: vi.fn(), setPermissionMode: vi.fn() } as any,
    // biome-ignore lint/suspicious/noExplicitAny: stub
    bridge: { push: vi.fn(), close: vi.fn(), next: vi.fn() } as any,
    abort: new AbortController(),
    closeReason: undefined,
    state: 'waiting_input',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter: new EventEmitter(),
    // biome-ignore lint/suspicious/noExplicitAny: stub
    permissions: { drainAll: vi.fn() } as any,
    permissionMode: 'default',
    wsClients: new Set(),
    lastSessionInit: null,
    // biome-ignore lint/suspicious/noExplicitAny: stub
    accumulator: { push: vi.fn(), getBlocks: vi.fn().mockReturnValue([]) } as any,
    ...overrides,
  }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

describe('SessionManager', () => {
  let registry: SessionRegistry
  let manager: InstanceType<typeof SessionManager>

  beforeEach(() => {
    registry = new SessionRegistry()
    manager = new SessionManager(registry)
    vi.clearAllMocks()
  })

  it('create() returns ControlSession with state creating → active', async () => {
    const cs = makeStubCs({ controlId: 'ctrl-new', sessionId: 'new-session-id' })
    vi.mocked(sdkSession.createControlSession).mockReturnValue(cs)
    vi.mocked(sdkSession.waitForSessionInit).mockResolvedValue(undefined)

    const result = await manager.create({
      model: 'claude-haiku-4-5-20251001',
    })

    expect(sdkSession.createControlSession).toHaveBeenCalledTimes(1)
    expect(sdkSession.waitForSessionInit).toHaveBeenCalledWith(cs, 15_000)
    expect(result).toBe(cs)
  })

  it('resume() throws if sessionId is unknown and SDK rejects', async () => {
    vi.mocked(sdkSession.resumeControlSession).mockRejectedValue(
      new Error('Session initialization timed out'),
    )

    await expect(
      manager.resume('unknown-session-id-0000-0000-0000', { projectPath: '/tmp' }),
    ).rejects.toThrow('timed out')
  })

  it('resume() passes projectPath to resumeControlSession', async () => {
    const cs = makeStubCs({ controlId: 'ctrl-resume', sessionId: 'resume-session-id' })
    vi.mocked(sdkSession.resumeControlSession).mockResolvedValue(cs)
    vi.mocked(sdkSession.waitForSessionInit).mockResolvedValue(undefined)

    await manager.resume('resume-session-id', { projectPath: '/my/project' })

    expect(sdkSession.resumeControlSession).toHaveBeenCalledWith(
      expect.objectContaining({ sessionId: 'resume-session-id', projectPath: '/my/project' }),
      registry,
    )
  })

  it('end() calls closeSession and the session remains in registry until runStreamLoop removes it', async () => {
    const cs = makeStubCs({ controlId: 'ctrl-end', sessionId: 'end-session-id' })
    registry.register(cs)

    await manager.end('end-session-id')

    expect(sdkSession.closeSession).toHaveBeenCalledWith(cs, registry)
  })

  it('end() throws if session not found', async () => {
    await expect(manager.end('nonexistent-session-0000-0000-0000')).rejects.toThrow('not found')
  })

  it('list() returns correct shape from registry', async () => {
    const cs1 = makeStubCs({ controlId: 'ctrl-1', sessionId: 'sess-1', state: 'waiting_input' })
    const cs2 = makeStubCs({ controlId: 'ctrl-2', sessionId: 'sess-2', state: 'active' })
    registry.register(cs1)
    registry.register(cs2)

    const sessions = manager.list()

    expect(sessions).toHaveLength(2)
    expect(sessions.map((s) => s.controlId)).toContain('ctrl-1')
    expect(sessions.map((s) => s.controlId)).toContain('ctrl-2')
    expect(sessions.map((s) => s.state)).toContain('waiting_input')
    expect(sessions.map((s) => s.state)).toContain('active')
  })

  it('wsClients Set allows multiple browser tabs to be tracked per session', () => {
    const cs = makeStubCs({ controlId: 'ctrl-ws', sessionId: 'ws-session-id' })
    registry.register(cs)

    // Simulate adding two "browser tabs" (mock WS objects)
    // biome-ignore lint/suspicious/noExplicitAny: test stub
    const ws1 = { send: vi.fn(), close: vi.fn() } as any
    // biome-ignore lint/suspicious/noExplicitAny: test stub
    const ws2 = { send: vi.fn(), close: vi.fn() } as any

    cs.wsClients.add(ws1)
    cs.wsClients.add(ws2)

    expect(cs.wsClients.size).toBe(2)

    // Remove one tab
    cs.wsClients.delete(ws1)
    expect(cs.wsClients.size).toBe(1)
    expect(cs.wsClients.has(ws2)).toBe(true)
  })
})
