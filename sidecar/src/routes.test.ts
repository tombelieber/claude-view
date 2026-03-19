// sidecar/src/routes.test.ts
// Unit tests for HTTP route handlers: resume, fork, create.
// Mocks sdk-session.js to test route logic in isolation.

import { EventEmitter } from 'node:events'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ControlSession } from './session-registry.js'
import { SessionRegistry } from './session-registry.js'

// --- Mocks ---

// Track what buildQueryOptions received (captured via createControlSession/forkControlSession mocks)
let capturedForkRequest: Record<string, unknown> | undefined
let capturedResumeRequest: Record<string, unknown> | undefined

const mockWaitForSessionInit = vi.fn()

vi.mock('./sdk-session.js', () => ({
  createControlSession: vi.fn(),
  resumeControlSession: vi.fn(),
  forkControlSession: vi.fn((req: Record<string, unknown>) => {
    capturedForkRequest = req
    const cs = makeStubCs({ controlId: 'fork-ctrl', sessionId: '' })
    return cs
  }),
  closeSession: vi.fn(),
  sendMessage: vi.fn(),
  waitForSessionInit: (...args: unknown[]) => mockWaitForSessionInit(...args),
  listAvailableSessions: vi.fn().mockResolvedValue([]),
}))

// Import routes AFTER mocks are set up
const { createRoutes } = await import('./routes.js')
const sdkSession = await import('./sdk-session.js')

// --- Helpers ---

function makeStubCs(overrides: Partial<ControlSession> = {}): ControlSession {
  return {
    controlId: 'ctrl-test',
    sessionId: '',
    model: 'claude-haiku-4-5-20251001',
    // biome-ignore lint/suspicious/noExplicitAny: stub
    query: { return: vi.fn(), close: vi.fn() } as any,
    // biome-ignore lint/suspicious/noExplicitAny: stub
    bridge: { push: vi.fn(), close: vi.fn(), next: vi.fn() } as any,
    abort: new AbortController(),
    closeReason: undefined,
    state: 'initializing',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter: new EventEmitter(),
    // biome-ignore lint/suspicious/noExplicitAny: stub
    permissions: { drainAll: vi.fn() } as any,
    permissionMode: 'default',
    activeWs: null,
    ...overrides,
  }
}

// --- Tests ---

describe('routes', () => {
  let registry: SessionRegistry

  beforeEach(() => {
    registry = new SessionRegistry()
    capturedForkRequest = undefined
    capturedResumeRequest = undefined
    vi.clearAllMocks()
  })

  describe('POST /sessions/resume', () => {
    it('calls waitForSessionInit before responding', async () => {
      const app = createRoutes(registry)
      const cs = makeStubCs({
        controlId: 'resume-ctrl',
        sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      })

      vi.mocked(sdkSession.resumeControlSession).mockResolvedValue(cs)
      mockWaitForSessionInit.mockResolvedValue(undefined)

      const res = await app.request('/sessions/resume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
          model: 'claude-haiku-4-5-20251001',
        }),
      })

      expect(res.status).toBe(200)
      // waitForSessionInit must have been called with the ControlSession
      expect(mockWaitForSessionInit).toHaveBeenCalledTimes(1)
      expect(mockWaitForSessionInit).toHaveBeenCalledWith(cs, 15_000)
    })

    it('returns 500 when waitForSessionInit times out', async () => {
      const app = createRoutes(registry)
      const cs = makeStubCs({
        controlId: 'resume-ctrl',
        sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      })

      vi.mocked(sdkSession.resumeControlSession).mockResolvedValue(cs)
      mockWaitForSessionInit.mockRejectedValue(new Error('Session initialization timed out'))

      const res = await app.request('/sessions/resume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
        }),
      })

      expect(res.status).toBe(500)
      const body = await res.json()
      expect(body.error).toContain('timed out')
    })

    it('returns already_active for session already in registry', async () => {
      const app = createRoutes(registry)
      const cs = makeStubCs({
        controlId: 'existing-ctrl',
        sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      })
      registry.register(cs)

      const res = await app.request('/sessions/resume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
        }),
      })

      expect(res.status).toBe(200)
      const body = await res.json()
      expect(body.status).toBe('already_active')
      expect(body.controlId).toBe('existing-ctrl')
      // Should NOT have called resumeControlSession or waitForSessionInit
      expect(sdkSession.resumeControlSession).not.toHaveBeenCalled()
      expect(mockWaitForSessionInit).not.toHaveBeenCalled()
    })

    it('rejects invalid session ID format', async () => {
      const app = createRoutes(registry)

      const res = await app.request('/sessions/resume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ sessionId: 'not-a-uuid' }),
      })

      expect(res.status).toBe(400)
      const body = await res.json()
      expect(body.error).toContain('Invalid session ID')
    })
  })

  describe('POST /sessions/fork', () => {
    it('forwards projectPath in the request body', async () => {
      const app = createRoutes(registry)
      const cs = makeStubCs({ controlId: 'fork-ctrl', sessionId: '' })

      vi.mocked(sdkSession.forkControlSession).mockReturnValue(cs)
      mockWaitForSessionInit.mockImplementation(async (target: ControlSession) => {
        target.sessionId = 'new-forked-session-id'
      })

      const res = await app.request('/sessions/fork', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
          model: 'claude-haiku-4-5-20251001',
          projectPath: '/Users/test/my-project',
        }),
      })

      expect(res.status).toBe(200)
      // Verify forkControlSession received the projectPath
      expect(sdkSession.forkControlSession).toHaveBeenCalledTimes(1)
      const forkCall = vi.mocked(sdkSession.forkControlSession).mock.calls[0]
      expect(forkCall[0].projectPath).toBe('/Users/test/my-project')
    })

    it('calls waitForSessionInit before responding', async () => {
      const app = createRoutes(registry)
      const cs = makeStubCs({ controlId: 'fork-ctrl', sessionId: '' })

      vi.mocked(sdkSession.forkControlSession).mockReturnValue(cs)
      mockWaitForSessionInit.mockImplementation(async (target: ControlSession) => {
        target.sessionId = 'new-forked-id'
      })

      const res = await app.request('/sessions/fork', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
        }),
      })

      expect(res.status).toBe(200)
      expect(mockWaitForSessionInit).toHaveBeenCalledTimes(1)
      expect(mockWaitForSessionInit).toHaveBeenCalledWith(cs, 15_000)
    })

    it('returns error when sessionId is missing', async () => {
      const app = createRoutes(registry)

      const res = await app.request('/sessions/fork', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      })

      expect(res.status).toBe(400)
      const body = await res.json()
      expect(body.error).toContain('sessionId')
    })
  })
})
