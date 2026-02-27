import fs from 'node:fs'
import { createAdaptorServer } from '@hono/node-server'
// sidecar/src/index.ts — updated
import { Hono } from 'hono'
import { controlRouter } from './control.js'
import { healthRouter } from './health.js'
import { SessionManager } from './session-manager.js'

const SOCKET_PATH = process.env.SIDECAR_SOCKET ?? `/tmp/claude-view-sidecar-${process.ppid}.sock`

const sessionManager = new SessionManager()
const app = new Hono()

app.route(
  '/health',
  healthRouter(() => sessionManager.getActiveCount()),
)
app.route('/control', controlRouter(sessionManager))
app.get('/', (c) => c.json({ status: 'ok' }))

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

export { app, server, sessionManager, SOCKET_PATH }
