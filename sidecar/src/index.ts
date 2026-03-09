import fs from 'node:fs'
import { createAdaptorServer } from '@hono/node-server'
// sidecar/src/index.ts — updated
import { Hono } from 'hono'
import { WebSocketServer } from 'ws'
import { controlRouter } from './control.js'
import { healthRouter } from './health.js'
import { SessionManager } from './session-manager.js'
import { runWorkflow } from './workflow-runner.js'
import type { WorkflowEvent } from './workflow-runner.js'
import { handleWebSocket } from './ws-handler.js'

const SOCKET_PATH = process.env.SIDECAR_SOCKET ?? `/tmp/claude-view-sidecar-${process.ppid}.sock`

const sessionManager = new SessionManager()
const app = new Hono()

app.route(
  '/health',
  healthRouter(() => sessionManager.getActiveCount()),
)
app.route('/control', controlRouter(sessionManager))
app.get('/', (c) => c.json({ status: 'ok' }))

// Workflow runner — POST /workflows/run
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

// Clean up stale socket from prior crash
if (fs.existsSync(SOCKET_PATH)) {
  fs.unlinkSync(SOCKET_PATH)
}

// Create HTTP server — createAdaptorServer accepts app directly
const server = createAdaptorServer(app)

server.listen(SOCKET_PATH, () => {
  console.log(`[sidecar] Listening on ${SOCKET_PATH}`)
  console.log(`[sidecar] PID: ${process.pid}, Parent PID: ${process.ppid}`)
})

// WS upgrade handler on the HTTP server (not a separate net.Server)
const wss = new WebSocketServer({ noServer: true })

server.on('upgrade', (request, socket, head) => {
  // Extract controlId from URL: /control/sessions/:controlId/stream
  const match = request.url?.match(/\/control\/sessions\/([^/]+)\/stream/)
  if (!match?.[1]) {
    socket.destroy()
    return
  }

  const controlId = match[1]
  wss.handleUpgrade(request, socket, head, (ws) => {
    handleWebSocket(ws, controlId, sessionManager)
  })
})

const parentCheck = setInterval(() => {
  try {
    process.kill(process.ppid!, 0)
  } catch {
    console.log('[sidecar] Parent process exited, shutting down')
    shutdown()
  }
}, 2000)

async function shutdown() {
  clearInterval(parentCheck)
  await sessionManager.shutdownAll()
  server.close()
  if (fs.existsSync(SOCKET_PATH)) {
    fs.unlinkSync(SOCKET_PATH)
  }
  process.exit(0)
}

process.on('SIGTERM', () => void shutdown())
process.on('SIGINT', () => void shutdown())

export { app, server, sessionManager, SOCKET_PATH, runWorkflow }
