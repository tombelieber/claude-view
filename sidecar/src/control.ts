// sidecar/src/control.ts
import { Hono } from 'hono'
import type { SessionManager } from './session-manager.js'
import type { ResumeRequest, SendRequest } from './types.js'

export function controlRouter(sessions: SessionManager) {
  const router = new Hono()

  // Resume (or get existing) a session
  router.post('/resume', async (c) => {
    const body = await c.req.json<ResumeRequest>()

    if (!body.sessionId?.match(/^[0-9a-f-]{36}$/)) {
      return c.json({ error: 'Invalid session ID format' }, 400)
    }

    // Check if already resumed
    if (sessions.hasSessionId(body.sessionId)) {
      const existing = sessions.getBySessionId(body.sessionId)!
      return c.json({
        controlId: existing.controlId,
        status: 'already_active',
        sessionId: body.sessionId,
      })
    }

    try {
      const cs = await sessions.resume(body.sessionId, body.model, body.projectPath)
      return c.json({
        controlId: cs.controlId,
        status: 'active',
        sessionId: body.sessionId,
      })
    } catch (err) {
      return c.json(
        {
          error: `Failed to resume: ${err instanceof Error ? err.message : err}`,
        },
        500,
      )
    }
  })

  // Send a message to an active session
  router.post('/send', async (c) => {
    const body = await c.req.json<SendRequest>()
    const session = sessions.getSession(body.controlId)
    if (!session) {
      return c.json({ error: 'Session not found' }, 404)
    }

    try {
      // Fire-and-forget: actual streaming goes via WebSocket
      sessions.sendMessage(body.controlId, body.message).catch((err) => {
        console.error(`[sidecar] sendMessage error: ${err}`)
      })
      return c.json({ status: 'sent' })
    } catch (err) {
      return c.json({ error: `Send failed: ${err}` }, 500)
    }
  })

  // List all active control sessions
  router.get('/sessions', (c) => {
    return c.json(sessions.listSessions())
  })

  // Terminate a control session
  router.delete('/sessions/:controlId', async (c) => {
    const { controlId } = c.req.param()
    await sessions.close(controlId)
    return c.json({ status: 'terminated' })
  })

  return router
}
