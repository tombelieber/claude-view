/**
 * Terminal WebSocket relay — bridges xterm.js ↔ tmux via node-pty.
 *
 * Protocol:
 *   Client → Server binary:  raw keystrokes → pty.write(data)
 *   Client → Server JSON:    { type: 'resize', cols: number, rows: number }
 *   Server → Client binary:  raw terminal output from pty
 *   Server → Client JSON:    { type: 'exit', code: number }
 *                             { type: 'error', message: string }
 *
 * Multi-client: multiple browser tabs can observe the same tmux session.
 * When the last client disconnects, the pty is killed and cleaned up.
 */

import { spawn } from 'node-pty'
import type { IPty } from 'node-pty'
import type { WebSocket } from 'ws'

// ── Types ───────────────────────────────────────────────────────────

interface TerminalSession {
  pty: IPty
  clients: Set<WebSocket>
}

interface ResizeMessage {
  type: 'resize'
  cols: number
  rows: number
}

type ClientMessage = ResizeMessage

// ── Constants ───────────────────────────────────────────────────────

/** Pause pty output when any client's WS buffer exceeds this (bytes). */
const BACKPRESSURE_HIGH = 128 * 1024
/** Resume pty output when all clients drop below this (bytes). */
const BACKPRESSURE_LOW = 16 * 1024

// ── State ───────────────────────────────────────────────────────────

const activeSessions = new Map<string, TerminalSession>()

// ── Helpers (exported for testing) ──────────────────────────────────

/**
 * Parse a JSON text message from the client.
 * Returns null if the message is not valid JSON or not a known type.
 */
export function parseClientMessage(data: string): ClientMessage | null {
  try {
    const msg = JSON.parse(data) as Record<string, unknown>
    if (msg.type === 'resize') {
      const cols = Number(msg.cols)
      const rows = Number(msg.rows)
      if (cols > 0 && rows > 0 && cols <= 500 && rows <= 200) {
        return { type: 'resize', cols, rows }
      }
    }
    return null
  } catch {
    return null
  }
}

/**
 * Check backpressure across all clients in a session.
 * Returns true if ANY client's buffer exceeds the high watermark.
 */
export function hasBackpressure(clients: Set<WebSocket>): boolean {
  for (const ws of clients) {
    if (ws.bufferedAmount > BACKPRESSURE_HIGH) {
      return true
    }
  }
  return false
}

/**
 * Check if all clients are below the low watermark (safe to resume).
 */
export function canResume(clients: Set<WebSocket>): boolean {
  for (const ws of clients) {
    if (ws.bufferedAmount > BACKPRESSURE_LOW) {
      return false
    }
  }
  return true
}

// ── Core ────────────────────────────────────────────────────────────

/**
 * Handle a new WebSocket connection for a terminal session.
 * If a pty already exists for this tmux session, the client joins it.
 * Otherwise, spawns `tmux attach-session -t {tmuxSessionId}`.
 */
export function handleTerminalWebSocket(
  ws: WebSocket,
  tmuxSessionId: string,
): void {
  let session = activeSessions.get(tmuxSessionId)

  if (!session) {
    // Spawn a new pty attached to the tmux session
    let pty: IPty
    try {
      pty = spawn('tmux', ['attach-session', '-t', tmuxSessionId], {
        cols: 120,
        rows: 40,
        name: 'xterm-256color',
      })
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Failed to spawn pty'
      sendJson(ws, { type: 'error', message })
      ws.close()
      return
    }

    session = { pty, clients: new Set() }
    activeSessions.set(tmuxSessionId, session)

    // Track paused state to avoid redundant pause/resume calls
    let paused = false

    // PTY → all clients (binary)
    pty.onData((data: string) => {
      const buf = Buffer.from(data, 'utf8')
      for (const client of session!.clients) {
        if (client.readyState === 1 /* WebSocket.OPEN */) {
          client.send(buf)
        }
      }

      // Backpressure: pause pty if any client is slow
      if (!paused && hasBackpressure(session!.clients)) {
        pty.pause()
        paused = true
      }
    })

    // PTY exit → notify all clients, clean up
    pty.onExit(({ exitCode }) => {
      for (const client of session!.clients) {
        sendJson(client, { type: 'exit', code: exitCode })
      }
      activeSessions.delete(tmuxSessionId)
    })

    // Periodically check if we can resume (drain check)
    const drainInterval = setInterval(() => {
      if (paused && session && canResume(session.clients)) {
        session.pty.resume()
        paused = false
      }
      // Stop interval when session is gone
      if (!activeSessions.has(tmuxSessionId)) {
        clearInterval(drainInterval)
      }
    }, 50)
  }

  // Add this client to the session
  session.clients.add(ws)

  // Client → PTY
  ws.on('message', (data: Buffer | string, isBinary: boolean) => {
    if (!session) return

    if (isBinary) {
      // Binary frame: raw keystrokes → pty
      const str = typeof data === 'string' ? data : data.toString('utf8')
      session.pty.write(str)
    } else {
      // Text frame: JSON control message
      const str = typeof data === 'string' ? data : data.toString('utf8')
      const msg = parseClientMessage(str)
      if (msg?.type === 'resize') {
        session.pty.resize(msg.cols, msg.rows)
      }
    }
  })

  // Client disconnect → remove from session, maybe kill pty
  ws.on('close', () => {
    if (!session) return
    session.clients.delete(ws)
    if (session.clients.size === 0) {
      session.pty.kill()
      activeSessions.delete(tmuxSessionId)
    }
  })

  ws.on('error', () => {
    // Error will be followed by close event — cleanup happens there
  })
}

/**
 * Kill all active terminal sessions. Called during sidecar shutdown.
 */
export function closeAllTerminals(): void {
  for (const [_id, session] of activeSessions) {
    session.pty.kill()
    for (const ws of session.clients) {
      ws.close()
    }
  }
  activeSessions.clear()
}

/**
 * Get the number of active terminal sessions (for health/debugging).
 */
export function activeTerminalCount(): number {
  return activeSessions.size
}

// ── Internal ────────────────────────────────────────────────────────

function sendJson(ws: WebSocket, payload: Record<string, unknown>): void {
  if (ws.readyState === 1 /* WebSocket.OPEN */) {
    ws.send(JSON.stringify(payload))
  }
}
