/**
 * Construct WebSocket URL, bypassing Vite proxy in dev mode.
 * Vite proxies WS correctly (unlike SSE), but we keep the bypass
 * option for consistency and as a fallback.
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
