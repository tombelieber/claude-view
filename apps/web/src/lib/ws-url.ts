/**
 * Construct WebSocket URL targeting the Rust server (:47892).
 * Bypasses Vite proxy in dev mode for endpoints served by the main server
 * (e.g. /api/live/sessions/:id/terminal).
 */
export function wsUrl(path: string): string {
  const loc = window.location
  // Dev mode: Vite runs on 5173, proxy to Rust on 47892
  if (loc.port === '5173') {
    return `ws://localhost:47892${path}`
  }
  // Production: same origin, upgrade protocol
  const protocol = loc.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${loc.host}${path}`
}

/**
 * Construct WebSocket URL for sidecar endpoints (:3001).
 * In dev, uses same-origin so Vite proxy routes /ws/chat/* -> ws://localhost:3001.
 * In prod, reverse proxy handles routing.
 */
export function sidecarWsUrl(path: string): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${window.location.host}${path}`
}
