// sidecar/src/index.ts
import fs from 'node:fs'
import { createAdaptorServer } from '@hono/node-server'
import { Hono } from 'hono'
import { WebSocketServer } from 'ws'
import { healthRouter } from './health.js'
import { startModelCacheRefresh } from './model-cache.js'
import { createRoutes } from './routes.js'
import { SessionRegistry } from './session-registry.js'
import { runWorkflow } from './workflow-runner.js'
import type { WorkflowEvent } from './workflow-runner.js'
import { handleWebSocket } from './ws-handler.js'

const SOCKET_PATH = process.env.SIDECAR_SOCKET ?? `/tmp/claude-view-sidecar-${process.ppid}.sock`

const registry = new SessionRegistry()
const app = new Hono()

app.route(
  '/health',
  healthRouter(() => registry.activeCount),
)
app.route('/control', createRoutes(registry))
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

// Clean up stale socket
if (fs.existsSync(SOCKET_PATH)) fs.unlinkSync(SOCKET_PATH)

const server = createAdaptorServer(app)
server.listen(SOCKET_PATH, () => {
  console.log(`[sidecar] Listening on ${SOCKET_PATH}`)
  console.log(`[sidecar] PID: ${process.pid}, Parent PID: ${process.ppid}`)
  // Populate supported models cache (fire-and-forget, refreshes hourly)
  startModelCacheRefresh()
})

// WS upgrade
const wss = new WebSocketServer({ noServer: true })
server.on('upgrade', (request, socket, head) => {
  const match = request.url?.match(/\/control\/sessions\/([^/]+)\/stream/)
  if (!match?.[1]) {
    socket.destroy()
    return
  }

  const controlId = match[1]
  wss.handleUpgrade(request, socket, head, (ws) => {
    handleWebSocket(ws, controlId, registry)
  })
})

// Parent process check
const parentCheck = setInterval(() => {
  try {
    process.kill(process.ppid!, 0)
  } catch {
    console.log('[sidecar] Parent exited, shutting down')
    void shutdown()
  }
}, 2000)

async function shutdown() {
  clearInterval(parentCheck)
  await registry.closeAll()
  server.close()
  if (fs.existsSync(SOCKET_PATH)) fs.unlinkSync(SOCKET_PATH)
  process.exit(0)
}

process.on('SIGTERM', () => void shutdown())
process.on('SIGINT', () => void shutdown())

export { app, registry, server, SOCKET_PATH, runWorkflow }
