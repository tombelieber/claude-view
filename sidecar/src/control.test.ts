import { Hono } from 'hono'
import { describe, expect, it, vi } from 'vitest'
import { controlRouter } from './control.js'
import type { SessionManager } from './session-manager.js'

const VALID_SESSION_ID = '123e4567-e89b-12d3-a456-426614174000'

function createApp(manager: unknown) {
  const app = new Hono()
  app.route('/control', controlRouter(manager as SessionManager))
  return app
}

describe('controlRouter', () => {
  it('rejects invalid session IDs on /resume', async () => {
    const app = createApp({
      hasSessionId: vi.fn().mockReturnValue(false),
    })

    const res = await app.request('http://localhost/control/resume', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ sessionId: 'not-a-uuid' }),
    })

    expect(res.status).toBe(400)
    await expect(res.json()).resolves.toMatchObject({
      error: 'Invalid session ID format',
    })
  })

  it('returns already_active for an existing resumed session', async () => {
    const app = createApp({
      hasSessionId: vi.fn().mockReturnValue(true),
      getBySessionId: vi.fn().mockReturnValue({ controlId: 'ctl-existing' }),
    })

    const res = await app.request('http://localhost/control/resume', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ sessionId: VALID_SESSION_ID }),
    })

    expect(res.status).toBe(200)
    await expect(res.json()).resolves.toMatchObject({
      controlId: 'ctl-existing',
      status: 'already_active',
      sessionId: VALID_SESSION_ID,
    })
  })

  it('returns 404 on /send when control session is missing', async () => {
    const app = createApp({
      getSession: vi.fn().mockReturnValue(undefined),
    })

    const res = await app.request('http://localhost/control/send', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ controlId: 'ctl-missing', message: 'hello' }),
    })

    expect(res.status).toBe(404)
    await expect(res.json()).resolves.toMatchObject({
      error: 'Session not found',
    })
  })

  it('returns active sessions from /sessions', async () => {
    const app = createApp({
      listSessions: vi.fn().mockReturnValue([
        {
          controlId: 'ctl-1',
          sessionId: VALID_SESSION_ID,
          status: 'active',
          turnCount: 3,
          totalCost: 0.12,
          startedAt: 1_700_000_000_000,
        },
      ]),
    })

    const res = await app.request('http://localhost/control/sessions')
    expect(res.status).toBe(200)

    await expect(res.json()).resolves.toEqual([
      {
        controlId: 'ctl-1',
        sessionId: VALID_SESSION_ID,
        status: 'active',
        turnCount: 3,
        totalCost: 0.12,
        startedAt: 1_700_000_000_000,
      },
    ])
  })
})
