// sidecar/src/control-binding.ts
// Fire-and-forget notifications to the Rust server's Live Monitor
// when the sidecar binds/unbinds control of a session.
// This sets LiveSession.control in the Rust server, which flows to SSE clients.

const RUST_SERVER_URL = process.env.RUST_SERVER_URL ?? 'http://localhost:47892'

export function notifyBindControl(sessionId: string, controlId: string): void {
  fetch(`${RUST_SERVER_URL}/api/live/sessions/${sessionId}/bind-control`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ controlId }),
  }).catch(() => {
    // Fire-and-forget — Rust server may not be running yet (startup race)
  })
}

export function notifyUnbindControl(sessionId: string, controlId: string): void {
  fetch(`${RUST_SERVER_URL}/api/live/sessions/${sessionId}/unbind-control`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ controlId }),
  }).catch(() => {
    // Fire-and-forget
  })
}
