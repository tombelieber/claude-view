import fs from 'node:fs'
import { createAdaptorServer } from '@hono/node-server'
// sidecar/src/index.ts
import { Hono } from 'hono'
import { healthRouter } from './health.js'

const SOCKET_PATH = process.env.SIDECAR_SOCKET ?? `/tmp/claude-view-sidecar-${process.ppid}.sock`

const app = new Hono()

// Placeholder active session count — Task 6 wires the real SessionManager
const activeSessionCount = 0

app.route(
  '/health',
  healthRouter(() => activeSessionCount),
)

// Root health check (for quick connectivity tests)
app.get('/', (c) => c.json({ status: 'ok' }))

// Clean up stale socket from prior crash
if (fs.existsSync(SOCKET_PATH)) {
  fs.unlinkSync(SOCKET_PATH)
}

// Create HTTP server — createAdaptorServer accepts app directly or { fetch: app.fetch }
const server = createAdaptorServer(app)

server.listen(SOCKET_PATH, () => {
  console.log(`[sidecar] Listening on ${SOCKET_PATH}`)
  console.log(`[sidecar] PID: ${process.pid}, Parent PID: ${process.ppid}`)
})

// Parent process liveness check (2s interval).
// If the Rust server dies, the sidecar self-terminates.
const parentCheck = setInterval(() => {
  try {
    process.kill(process.ppid!, 0) // signal 0 = check if alive
  } catch {
    console.log('[sidecar] Parent process exited, shutting down')
    shutdown()
  }
}, 2000)

function shutdown() {
  clearInterval(parentCheck)
  server.close()
  if (fs.existsSync(SOCKET_PATH)) {
    fs.unlinkSync(SOCKET_PATH)
  }
  process.exit(0)
}

process.on('SIGTERM', shutdown)
process.on('SIGINT', shutdown)

export { app, server, SOCKET_PATH }
