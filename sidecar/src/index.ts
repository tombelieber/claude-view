// sidecar/src/index.ts
// CLAUDECODE=1 blocks nested SDK sessions (anti-recursion guard in Claude Code).
// The sidecar spawns SDK child processes that must NOT inherit this flag.
// Must be deleted before any query() call — do it at startup.
process.env.CLAUDECODE = undefined

import { createAdaptorServer } from '@hono/node-server'
import { Hono } from 'hono'
import { WebSocketServer } from 'ws'
import { healthRouter } from './health.js'
import { startModelCacheRefresh, stopModelCacheRefresh } from './model-cache.js'
import { createRoutes } from './routes.js'
import { SessionRegistry } from './session-registry.js'
import { runWorkflow } from './workflow-runner.js'
import type { WorkflowEvent } from './workflow-runner.js'
import { handleWebSocket } from './ws-handler.js'

const SIDECAR_PORT = Number(process.env.SIDECAR_PORT ?? '3001')

const registry = new SessionRegistry()
const app = new Hono()

app.route(
  '/health',
  healthRouter(() => registry.activeCount),
)
app.route('/api/sidecar', createRoutes(registry))
app.get('/', (c) => c.json({ status: 'ok' }))

// Workflow runner — POST /workflows/run (preserved from existing sidecar)
app.post('/workflows/run', async (c) => {
  const body = await c.req.json<{ workflowId: string; inputs?: Record<string, string> }>()
  if (!body.workflowId) {
    return c.json({ error: 'Missing workflowId' }, 400)
  }

  const events: WorkflowEvent[] = []
  await runWorkflow(body.workflowId, body.inputs ?? {}, (event) => {
    events.push(event)
  })

  const lastEvent = events[events.length - 1]
  return c.json({
    status: lastEvent?.type === 'workflow_complete' ? 'complete' : 'failed',
    events,
  })
})

const server = createAdaptorServer(app)

// Fail-fast port-conflict guard. The parent `bun dev` runs `predev: cleanupport`
// to kill stale holders, but races are still possible (e.g. concurrent restart,
// external process grabbing :3001). Surface an actionable message instead of an
// opaque uncaught error, then exit so concurrently --kill-others tears everything
// down cleanly — better than a half-alive stack where frontend hangs at Connecting.
server.on('error', (err: NodeJS.ErrnoException) => {
  if (err.code === 'EADDRINUSE') {
    console.error(`[sidecar] ✗ Port ${SIDECAR_PORT} is already in use.`)
    console.error('[sidecar]   Another sidecar may be running. To clear:')
    console.error(`[sidecar]     lsof -ti :${SIDECAR_PORT} | xargs kill -9`)
    process.exit(1)
  }
  console.error('[sidecar] ✗ Server error:', err)
  process.exit(1)
})

server.listen(SIDECAR_PORT, () => {
  console.log(`[sidecar] Listening on :${SIDECAR_PORT}`)
  console.log(`[sidecar] PID: ${process.pid}`)
  // Populate supported models cache (fire-and-forget, refreshes hourly)
  startModelCacheRefresh()
})

// WS upgrade — /ws/chat/:sessionId
const wss = new WebSocketServer({ noServer: true })
server.on('upgrade', (request, socket, head) => {
  const match = request.url?.match(/\/ws\/chat\/([^/?]+)/)
  if (!match?.[1]) {
    socket.destroy()
    return
  }

  const sessionId = match[1]
  const cs = registry.getBySessionId(sessionId)
  if (!cs) {
    socket.destroy()
    return
  }

  wss.handleUpgrade(request, socket, head, (ws) => {
    handleWebSocket(ws, cs.controlId, registry)
  })
})

async function shutdown() {
  stopModelCacheRefresh()
  wss.close()
  await registry.closeAll()
  server.close()
  process.exit(0)
}

process.on('SIGTERM', () => void shutdown())
process.on('SIGINT', () => void shutdown())

export { app, registry, server, SIDECAR_PORT, runWorkflow }
