// sidecar/src/session-manager-api.integration.test.ts
// Integration tests for the REST API surface exposed by routes.ts.
// Mocks the sdk-session layer so no real Claude API calls are made.
// Tests the HTTP layer: status codes, response shapes, error paths.

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

const { createRoutes } = await import('./routes.js')
const sdkSession = await import('./sdk-session.js')

// ─── Helpers ────────────────────────────────────────────────────────────────

function makeStubCs(overrides: Partial<ControlSession> = {}): ControlSession {
  return {
    controlId: 'ctrl-test',
    sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
    model: 'claude-haiku-4-5-20251001',
    // biome-ignore lint/suspicious/noExplicitAny: stub
    query: { return: vi.fn(), close: vi.fn(), setPermissionMode: vi.fn() } as any,
    // biome-ignore lint/suspicious/noExplicitAny: stub
    bridge: { push: vi.fn(), close: vi.fn(), next: vi.fn() } as any,
    abort: new AbortController(),
    closeReason: undefined,
    state: 'waiting_input',
    totalCostUsd: 1.23,
    turnCount: 3,
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

describe('SessionManager REST API', () => {
  let registry: SessionRegistry

  beforeEach(() => {
    registry = new SessionRegistry()
    vi.clearAllMocks()
  })

  describe('POST /sessions', () => {
    it('returns 200 with { controlId, sessionId, status: created }', async () => {
      const app = createRoutes(registry)
      const cs = makeStubCs({ controlId: 'ctrl-new', sessionId: 'new-session-id' })

      vi.mocked(sdkSession.createControlSession).mockReturnValue(cs)
      vi.mocked(sdkSession.waitForSessionInit).mockResolvedValue(undefined)

      const res = await app.request('/sessions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ model: 'claude-haiku-4-5-20251001' }),
      })

      expect(res.status).toBe(200)
      const body = await res.json()
      expect(body.status).toBe('created')
      expect(body.controlId).toBe('ctrl-new')
      expect(body.sessionId).toBe('new-session-id')
    })

    it('returns 400 if model is missing', async () => {
      const app = createRoutes(registry)

      const res = await app.request('/sessions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      })

      expect(res.status).toBe(400)
      const body = await res.json()
      expect(body.error).toContain('model is required')
    })

    it('returns 500 if waitForSessionInit times out', async () => {
      const app = createRoutes(registry)
      const cs = makeStubCs()

      vi.mocked(sdkSession.createControlSession).mockReturnValue(cs)
      vi.mocked(sdkSession.waitForSessionInit).mockRejectedValue(
        new Error('Session initialization timed out'),
      )

      const res = await app.request('/sessions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ model: 'claude-haiku-4-5-20251001' }),
      })

      expect(res.status).toBe(500)
      const body = await res.json()
      expect(body.error).toContain('timed out')
    })
  })

  describe('GET /sessions', () => {
    it('returns 200 with { active: [], available: [] } when no sessions', async () => {
      const app = createRoutes(registry)

      const res = await app.request('/sessions')

      expect(res.status).toBe(200)
      const body = await res.json()
      expect(body).toHaveProperty('active')
      expect(body).toHaveProperty('available')
      expect(Array.isArray(body.active)).toBe(true)
      expect(Array.isArray(body.available)).toBe(true)
    })

    it('returns active sessions with correct shape', async () => {
      const app = createRoutes(registry)
      const cs = makeStubCs({
        controlId: 'ctrl-list',
        sessionId: 'list-session-id',
        state: 'waiting_input',
        turnCount: 2,
        totalCostUsd: 0.05,
      })
      registry.register(cs)

      const res = await app.request('/sessions')

      expect(res.status).toBe(200)
      const body = await res.json()
      expect(body.active).toHaveLength(1)
      const session = body.active[0]
      expect(session.controlId).toBe('ctrl-list')
      expect(session.sessionId).toBe('list-session-id')
      expect(session.state).toBe('waiting_input')
      expect(session.turnCount).toBe(2)
    })
  })

  describe('POST /sessions/:id/resume', () => {
    it('returns 200 with status: resumed for a valid session', async () => {
      const app = createRoutes(registry)
      const cs = makeStubCs({
        controlId: 'ctrl-resume',
        sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      })

      vi.mocked(sdkSession.resumeControlSession).mockResolvedValue(cs)
      vi.mocked(sdkSession.waitForSessionInit).mockResolvedValue(undefined)

      const res = await app.request('/sessions/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee/resume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ model: 'claude-haiku-4-5-20251001' }),
      })

      expect(res.status).toBe(200)
      const body = await res.json()
      expect(body.status).toBe('resumed')
      expect(body.controlId).toBe('ctrl-resume')
    })

    it('returns 200 with status: already_active for a live session', async () => {
      const app = createRoutes(registry)
      const cs = makeStubCs({
        controlId: 'ctrl-existing',
        sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      })
      registry.register(cs)

      const res = await app.request('/sessions/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee/resume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      })

      expect(res.status).toBe(200)
      const body = await res.json()
      expect(body.status).toBe('already_active')
      // Should not call resumeControlSession since it's already live
      expect(sdkSession.resumeControlSession).not.toHaveBeenCalled()
    })

    it('returns 400 for invalid session ID format', async () => {
      const app = createRoutes(registry)

      const res = await app.request('/sessions/not-a-uuid/resume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({}),
      })

      expect(res.status).toBe(400)
      const body = await res.json()
      expect(body.error).toContain('Invalid session ID')
    })
  })

  describe('DELETE /sessions/:id', () => {
    it('returns 200 with status: terminated when session exists', async () => {
      const app = createRoutes(registry)
      const cs = makeStubCs({
        controlId: 'ctrl-del',
        sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      })
      registry.register(cs)

      const res = await app.request('/sessions/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee', {
        method: 'DELETE',
      })

      expect(res.status).toBe(200)
      const body = await res.json()
      expect(body.status).toBe('terminated')
      expect(sdkSession.closeSession).toHaveBeenCalledWith(cs, registry)
    })

    it('returns 200 with status: terminated even if session not found (idempotent)', async () => {
      const app = createRoutes(registry)

      const res = await app.request('/sessions/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee', {
        method: 'DELETE',
      })

      expect(res.status).toBe(200)
      const body = await res.json()
      expect(body.status).toBe('terminated')
      // Should not call closeSession if session not found
      expect(sdkSession.closeSession).not.toHaveBeenCalled()
    })
  })

  describe('GET /sessions/:id/status', () => {
    it('returns 200 with model, state, permissionMode, cost for existing session', async () => {
      const app = createRoutes(registry)
      const cs = makeStubCs({
        controlId: 'ctrl-status',
        sessionId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
        model: 'claude-sonnet-4-20250514',
        state: 'active',
        permissionMode: 'acceptEdits',
        turnCount: 5,
        totalCostUsd: 2.5,
      })
      registry.register(cs)

      const res = await app.request('/sessions/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee/status')

      expect(res.status).toBe(200)
      const body = await res.json()
      expect(body.sessionId).toBe('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')
      expect(body.model).toBe('claude-sonnet-4-20250514')
      expect(body.state).toBe('active')
      expect(body.permissionMode).toBe('acceptEdits')
      expect(body.turnCount).toBe(5)
      expect(body.totalCostUsd).toBe(2.5)
    })

    it('returns 404 when session not found', async () => {
      const app = createRoutes(registry)

      const res = await app.request('/sessions/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee/status')

      expect(res.status).toBe(404)
      const body = await res.json()
      expect(body.error).toContain('not found')
    })
  })
})
