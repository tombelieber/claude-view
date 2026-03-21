// sidecar/src/routes.ts
import { Hono } from 'hono'
import { notifyBindControl, notifyUnbindControl } from './control-binding.js'
import { getCacheState } from './model-cache.js'
import type {
  CreateSessionRequest,
  ForkSessionRequest,
  PromptRequest,
  ResumeSessionRequest,
} from './protocol.js'
import {
  closeSession,
  createControlSession,
  forkControlSession,
  listAvailableSessions,
  resumeControlSession,
  sendMessage,
  waitForSessionInit,
} from './sdk-session.js'
import type { SessionRegistry } from './session-registry.js'

export function createRoutes(registry: SessionRegistry) {
  const app = new Hono()

  // Create new session (cold start — MCP servers connect on demand)
  app.post('/sessions', async (c) => {
    const body = await c.req.json<CreateSessionRequest>()
    if (!body.model) return c.json({ error: 'model is required' }, 400)

    try {
      const cs = createControlSession(body, registry)
      await waitForSessionInit(cs, 60_000)
      notifyBindControl(cs.sessionId, cs.controlId)
      return c.json({
        controlId: cs.controlId,
        sessionId: cs.sessionId,
        status: 'created',
      })
    } catch (err) {
      return c.json({ error: `Create failed: ${err instanceof Error ? err.message : err}` }, 500)
    }
  })

  // Resume existing session (path param)
  app.post('/sessions/:id/resume', async (c) => {
    const sessionId = c.req.param('id')
    if (!sessionId?.match(/^[0-9a-f-]{36}$/)) {
      return c.json({ error: 'Invalid session ID format' }, 400)
    }

    // Check if already resumed — still notify Rust server in case
    // a prior bind was lost (startup race, server restart, etc.)
    if (registry.hasSessionId(sessionId)) {
      const existing = registry.getBySessionId(sessionId)!
      notifyBindControl(sessionId, existing.controlId)
      return c.json({
        controlId: existing.controlId,
        sessionId,
        status: 'already_active',
      })
    }

    const body = await c.req.json<Omit<ResumeSessionRequest, 'sessionId'>>().catch(() => ({}))

    try {
      const cs = await resumeControlSession({ sessionId, ...body }, registry)
      await waitForSessionInit(cs, 15_000)
      notifyBindControl(cs.sessionId || sessionId, cs.controlId)
      return c.json({
        controlId: cs.controlId,
        sessionId: cs.sessionId || sessionId,
        status: 'resumed',
      })
    } catch (err) {
      return c.json({ error: `Resume failed: ${err instanceof Error ? err.message : err}` }, 500)
    }
  })

  // Fork existing session (path param)
  app.post('/sessions/:id/fork', async (c) => {
    const sessionId = c.req.param('id')
    if (!sessionId) return c.json({ error: 'sessionId is required' }, 400)

    const body = await c.req.json<Omit<ForkSessionRequest, 'sessionId'>>().catch(() => ({}))

    try {
      const cs = forkControlSession({ sessionId, ...body }, registry)
      await waitForSessionInit(cs, 15_000)
      notifyBindControl(cs.sessionId, cs.controlId)
      return c.json({
        controlId: cs.controlId,
        sessionId: cs.sessionId,
        status: 'forked',
      })
    } catch (err) {
      return c.json({ error: `Fork failed: ${err instanceof Error ? err.message : err}` }, 500)
    }
  })

  // Send message to session (path param)
  app.post('/sessions/:id/send', async (c) => {
    const sessionId = c.req.param('id')
    const cs = registry.getBySessionId(sessionId)
    if (!cs) return c.json({ error: 'Session not found' }, 404)

    const body = await c.req.json<{ message: string }>()
    sendMessage(cs, body.message)
    return c.json({ status: 'sent' })
  })

  // One-shot prompt for session (path param)
  app.post('/sessions/:id/prompt', async (c) => {
    const body = await c.req.json<PromptRequest>()
    if (!body.message || !body.model)
      return c.json({ error: 'message and model are required' }, 400)

    try {
      const cs = createControlSession(
        { model: body.model, permissionMode: body.permissionMode, initialMessage: body.message },
        registry,
      )
      // Wait for turn_complete by listening to the emitter
      return new Promise<Response>((resolve) => {
        const timeout = setTimeout(() => {
          closeSession(cs, registry)
          resolve(c.json({ error: 'Prompt timed out' }, 504))
        }, 120_000) // 2 min max

        cs.emitter.on('message', (event) => {
          if (event.type === 'turn_complete' || event.type === 'turn_error') {
            clearTimeout(timeout)
            closeSession(cs, registry)
            resolve(c.json(event))
          }
        })
      })
    } catch (err) {
      return c.json({ error: `Prompt failed: ${err instanceof Error ? err.message : err}` }, 500)
    }
  })

  // Session status
  app.get('/sessions/:id/status', (c) => {
    const sessionId = c.req.param('id')
    const cs = registry.getBySessionId(sessionId)
    if (!cs) return c.json({ error: 'Session not found' }, 404)
    return c.json({
      sessionId: cs.sessionId,
      state: cs.state,
      model: cs.model,
      permissionMode: cs.permissionMode,
      turnCount: cs.turnCount,
      totalCostUsd: cs.totalCostUsd,
      startedAt: cs.startedAt,
    })
  })

  // List active control sessions + available sessions merged
  app.get('/sessions', async (c) => {
    const active = registry.list()
    try {
      const available = await listAvailableSessions()
      return c.json({ active, available })
    } catch {
      return c.json({ active, available: [] })
    }
  })

  // Terminate session (by sessionId)
  app.delete('/sessions/:id', (c) => {
    const sessionId = c.req.param('id')
    const cs = registry.getBySessionId(sessionId)
    if (cs) {
      notifyUnbindControl(cs.sessionId, cs.controlId)
      closeSession(cs, registry)
    }
    return c.json({ status: 'terminated' })
  })

  // Supported models (cached from SDK, refreshed on every session create/resume)
  app.get('/sessions/models', (c) => c.json(getCacheState()))

  return app
}
