// NOTE: D1Database is an ambient type from @cloudflare/workers-types (declared in tsconfig.json
// "types" array). Do NOT import it — it's a .d.ts file, not a runtime module.

interface RateLimitResult {
  allowed: boolean
  remaining: number
  resetAt: number // unix timestamp when window resets
}

/**
 * Sliding-window rate limiter backed by D1.
 * @param db      D1 database
 * @param key     Identifier string (e.g. "{user_id}:create")
 * @param limit   Max requests per window
 * @param windowSecs Window size in seconds
 */
export async function checkRateLimit(
  db: D1Database,
  key: string,
  limit: number,
  windowSecs: number,
): Promise<RateLimitResult> {
  const now = Math.floor(Date.now() / 1000)
  const window = Math.floor(now / windowSecs) * windowSecs
  const resetAt = window + windowSecs

  // Upsert counter for this window
  const result = await db
    .prepare(
      `INSERT INTO rate_limits (key, window, count) VALUES (?, ?, 1)
       ON CONFLICT (key, window) DO UPDATE SET count = count + 1
       RETURNING count`,
    )
    .bind(key, window)
    .first<{ count: number }>()

  const count = result?.count ?? 1
  const allowed = count <= limit
  const remaining = Math.max(0, limit - count)

  return { allowed, remaining, resetAt }
}

/** Periodic cleanup — call from scheduled handler. */
export async function cleanupExpiredWindows(db: D1Database): Promise<void> {
  const cutoff = Math.floor(Date.now() / 1000) - 3600 // keep 1 hour of history
  await db.prepare(`DELETE FROM rate_limits WHERE window < ?`).bind(cutoff).run()
}
