/** Real-time rate-limit data derived from statusline SSE events. */
export interface StatuslineRateLimit {
  /** 5-hour session usage percentage (0–100). */
  pct5h: number
  /** 5-hour reset timestamp (Unix seconds). */
  reset5h: number
  /** 7-day usage percentage (0–100), if available. */
  pct7d?: number
  /** 7-day reset timestamp (Unix seconds), if available. */
  reset7d?: number
}
