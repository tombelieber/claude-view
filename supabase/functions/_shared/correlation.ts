// Single mint point for correlation IDs inside Supabase Edge Functions.
// Per design spec §3.4.4, grep for `crypto.randomUUID` should return exactly
// one hit in this directory — this line.

/**
 * Mint a new correlation ID (UUID v7, time-ordered for k-sorting).
 *
 * Deno's crypto.randomUUID currently emits v4. Until v7 lands in stable Deno,
 * we use v4 and sort by occurred_at in queries. The wire-level contract does
 * not depend on the version byte.
 */
export function newCorrelationId(): string {
  return crypto.randomUUID()
}

/**
 * Extract the correlation ID from an incoming request, or mint a fresh one.
 * Clients SHOULD include `x-correlation-id` on every request; servers honor it.
 */
export function correlationIdFrom(req: Request): string {
  const incoming = req.headers.get('x-correlation-id')
  if (incoming && incoming.length >= 8 && incoming.length <= 64) {
    return incoming
  }
  return newCorrelationId()
}
