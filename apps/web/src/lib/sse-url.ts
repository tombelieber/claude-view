/**
 * Build an SSE-safe URL that bypasses Vite's HTTP proxy in dev mode.
 * Vite's http-proxy buffers SSE responses, defeating real-time feedback.
 */
export function sseUrl(path: string): string {
  if (typeof window !== 'undefined' && window.location.port === '5173') {
    return `http://localhost:47892${path}`
  }
  return path
}
