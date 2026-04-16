// Extract the caller's user_id from a verified Supabase JWT.
//
// Supabase Edge Functions automatically verify the JWT from the
// Authorization header IF verify_jwt = true in config.toml. We then read
// the claims via the JWT field in the incoming request. This helper wraps
// the pattern so every function uses it identically.
//
// The SupabaseClient is parameterized with Database so every .from() call
// across every edge function gets compile-time schema validation. A new
// column in public.devices → regenerate types → every consumer updates.

import { createClient, type SupabaseClient } from 'https://esm.sh/@supabase/supabase-js@2.45.0'

// Import the generated Database type from the shared workspace package.
// This is the single source of truth for schema shape across the whole
// monorepo — regenerate via `supabase gen types typescript ...` after
// any migration.
import type { Database } from '../../../packages/shared/src/types/supabase.generated.ts'

/** Type-alias so edge functions don't have to write `<Database>` everywhere. */
export type TypedSupabaseClient = SupabaseClient<Database>

export interface AuthenticatedCaller {
  user_id: string
  email: string | null
  jwt: string
}

/**
 * Extract the authenticated caller from a request. Returns null if the
 * Authorization header is missing or malformed. The JWT itself is already
 * verified by Supabase's verify_jwt=true gate before this function runs,
 * so we only need to decode the payload.
 */
export function extractCaller(req: Request): AuthenticatedCaller | null {
  const auth = req.headers.get('authorization')
  if (!auth || !auth.startsWith('Bearer ')) return null
  const jwt = auth.slice(7)
  // Base64url decode the payload (middle segment).
  const [, payloadB64] = jwt.split('.')
  if (!payloadB64) return null
  try {
    const payloadJson = atob(payloadB64.replaceAll('-', '+').replaceAll('_', '/'))
    const claims = JSON.parse(payloadJson) as { sub?: string; email?: string }
    if (!claims.sub) return null
    return {
      user_id: claims.sub,
      email: claims.email ?? null,
      jwt,
    }
  } catch {
    return null
  }
}

/**
 * Build a schema-typed service-role Supabase client for a request. The
 * service-role key is available to Edge Functions via Deno.env. This client
 * bypasses RLS and must only be used inside the function — never returned
 * to callers, never used outside a request that has already called
 * extractCaller() to verify the JWT.
 */
export function serviceRoleClient(): TypedSupabaseClient {
  const url = Deno.env.get('SUPABASE_URL')
  const key = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY')
  if (!url || !key) {
    throw new Error(
      'SUPABASE_URL or SUPABASE_SERVICE_ROLE_KEY missing from Edge Function environment',
    )
  }
  return createClient<Database>(url, key, { auth: { persistSession: false } })
}
