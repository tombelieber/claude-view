// sidecar/src/routes.ts
import { Hono } from 'hono'
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

  // Create new session
  app.post('/sessions', async (c) => {
    const body = await c.req.json<CreateSessionRequest>()
    if (!body.model) return c.json({ error: 'model is required' }, 400)

    try {
      const cs = createControlSession(body, registry)
      await waitForSessionInit(cs, 15_000)
      return c.json({
        controlId: cs.controlId,
        sessionId: cs.sessionId,
        status: 'created',
      })
    } catch (err) {
      return c.json({ error: `Create failed: ${err instanceof Error ? err.message : err}` }, 500)
    }
  })

  // Resume existing session
  app.post('/sessions/resume', async (c) => {
    const body = await c.req.json<ResumeSessionRequest>()
    if (!body.sessionId?.match(/^[0-9a-f-]{36}$/)) {
      return c.json({ error: 'Invalid session ID format' }, 400)
    }

    // Check if already resumed
    if (registry.hasSessionId(body.sessionId)) {
      const existing = registry.getBySessionId(body.sessionId)!
      return c.json({
        controlId: existing.controlId,
        sessionId: body.sessionId,
        status: 'already_active',
      })
    }

    try {
      const cs = await resumeControlSession(body, registry)
      await waitForSessionInit(cs, 15_000)
      return c.json({
        controlId: cs.controlId,
        sessionId: cs.sessionId || body.sessionId,
        status: 'resumed',
      })
    } catch (err) {
      return c.json({ error: `Resume failed: ${err instanceof Error ? err.message : err}` }, 500)
    }
  })

  // Fork existing session — creates a new session branching from an existing one
  app.post('/sessions/fork', async (c) => {
    const body = await c.req.json<ForkSessionRequest>()
    if (!body.sessionId) return c.json({ error: 'sessionId is required' }, 400)

    try {
      const cs = forkControlSession(body, registry)
      await waitForSessionInit(cs, 15_000)
      return c.json({
        controlId: cs.controlId,
        sessionId: cs.sessionId,
        status: 'forked',
      })
    } catch (err) {
      return c.json({ error: `Fork failed: ${err instanceof Error ? err.message : err}` }, 500)
    }
  })

  // Send message — synchronous bridge.push, fire-and-forget by design.
  // Returns immediately after queuing; the SDK processes the message and emits
  // response events (assistant_text, tool_use_start, turn_complete, etc.) over
  // the WS stream.
  app.post('/send', async (c) => {
    const body = await c.req.json<{ controlId: string; message: string }>()
    const cs = registry.get(body.controlId)
    if (!cs) return c.json({ error: 'Session not found' }, 404)

    sendMessage(cs, body.message)
    return c.json({ status: 'sent' })
  })

  // List active control sessions
  app.get('/sessions', (c) => c.json(registry.list()))

  // List available Claude Code sessions
  app.get('/available-sessions', async (c) => {
    try {
      const sessions = await listAvailableSessions()
      return c.json(sessions)
    } catch (err) {
      return c.json({ error: `List failed: ${err instanceof Error ? err.message : err}` }, 500)
    }
  })

  // One-shot prompt (no session lifecycle)
  app.post('/prompt', async (c) => {
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

  // Terminate session
  app.delete('/sessions/:controlId', async (c) => {
    const controlId = c.req.param('controlId')
    const cs = registry.get(controlId)
    if (cs) closeSession(cs, registry)
    return c.json({ status: 'terminated' })
  })

  // Supported models (cached from SDK, refreshed on every session create/resume)
  app.get('/supported-models', (c) => c.json(getCacheState()))

  return app
}
