import { checkHoneypot, generateReferralCode, isValidEmail } from '../_lib/waitlist-utils'

interface Env {
  SUPABASE_URL: string
  SUPABASE_SECRET_KEY: string
  TURNSTILE_SECRET_KEY: string
}

interface WaitlistRow {
  position: number
  referral_code: string
}

const CORS_HEADERS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
  'Access-Control-Allow-Headers': 'Content-Type',
}

/** Supabase REST helper — uses Secret key (bypasses RLS). */
async function supabaseRequest(
  env: Env,
  path: string,
  options: RequestInit = {},
): Promise<Response> {
  const headers = new Headers(options.headers)
  headers.set('apikey', env.SUPABASE_SECRET_KEY)
  headers.set('Authorization', `Bearer ${env.SUPABASE_SECRET_KEY}`)
  if (!headers.has('Content-Type')) headers.set('Content-Type', 'application/json')
  return fetch(`${env.SUPABASE_URL}${path}`, { ...options, headers })
}

/** Validate Cloudflare Turnstile token server-side. */
async function verifyTurnstile(token: string, secret: string, ip: string): Promise<boolean> {
  const res = await fetch('https://challenges.cloudflare.com/turnstile/v0/siteverify', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({ secret, response: token, remoteip: ip }),
  })
  const data = (await res.json()) as { success: boolean }
  return data.success
}

/** Get total waitlist count.
 *  Uses GET + Range: 0-0 (fetches at most 1 row) with count=exact to get total via Content-Range.
 *  Avoids sending Content-Type on a HEAD request, which is malformed HTTP and can confuse PostgREST.
 */
async function getCount(env: Env): Promise<number> {
  const res = await supabaseRequest(env, '/rest/v1/waitlist?select=id', {
    headers: { Prefer: 'count=exact', Range: '0-0' },
  })
  const range = res.headers.get('Content-Range') // "0-0/347" or "*/0" if empty table
  if (!range) return 0
  const match = range.match(/\/(\d+)$/)
  return match ? Number.parseInt(match[1], 10) : 0
}

// ─── POST /api/waitlist ──────────────────────────────────────────────
export const onRequestPost: PagesFunction<Env> = async (context) => {
  const { env, request } = context

  // Parse body
  let body: { email?: string; ref?: string; turnstile_token?: string; company?: string }
  try {
    body = await request.json()
  } catch {
    return Response.json({ error: 'Invalid JSON' }, { status: 400, headers: CORS_HEADERS })
  }

  const { email, ref, turnstile_token, company } = body

  // Honeypot — if filled, silently return fake success (don't tip off bots).
  // Uses real total_count to avoid fingerprinting (hardcoded 999 was detectable).
  if (checkHoneypot(company)) {
    const fakeCount = await getCount(env)
    return Response.json(
      {
        position: Math.floor(Math.random() * 500) + 100,
        referral_code: generateReferralCode(),
        total_count: fakeCount,
      },
      { headers: CORS_HEADERS },
    )
  }

  // Validate Turnstile
  if (!turnstile_token) {
    return Response.json(
      { error: 'Missing verification token' },
      { status: 400, headers: CORS_HEADERS },
    )
  }
  const ip = request.headers.get('CF-Connecting-IP') || ''
  const turnstileValid = await verifyTurnstile(turnstile_token, env.TURNSTILE_SECRET_KEY, ip)
  if (!turnstileValid) {
    return Response.json({ error: 'Verification failed' }, { status: 403, headers: CORS_HEADERS })
  }

  // Validate email
  if (!email || !isValidEmail(email)) {
    return Response.json({ error: 'Invalid email address' }, { status: 400, headers: CORS_HEADERS })
  }

  const normalizedEmail = email.toLowerCase().trim()

  // Sanitize ref — must be exactly 8 alphanumeric chars (our referral code format).
  // Rejects oversized/malformed values before they reach Supabase.
  const normalizedRef = typeof ref === 'string' && /^[A-Za-z0-9]{8}$/.test(ref) ? ref : null

  // Check if email already exists (idempotent)
  const existingRes = await supabaseRequest(
    env,
    `/rest/v1/waitlist?email=eq.${encodeURIComponent(normalizedEmail)}&select=position,referral_code`,
    { headers: { Accept: 'application/json' } },
  )
  if (!existingRes.ok) {
    console.error('Supabase query error:', await existingRes.text())
    return Response.json({ error: 'Something went wrong' }, { status: 500, headers: CORS_HEADERS })
  }
  const existingRows = (await existingRes.json()) as WaitlistRow[]
  if (existingRows.length > 0) {
    const total = await getCount(env)
    return Response.json(
      {
        position: existingRows[0].position,
        referral_code: existingRows[0].referral_code,
        total_count: total,
      },
      { headers: CORS_HEADERS },
    )
  }

  // Insert new entry
  const referral_code = generateReferralCode()
  const insertRes = await supabaseRequest(env, '/rest/v1/waitlist', {
    method: 'POST',
    headers: { Prefer: 'return=representation' },
    body: JSON.stringify({
      email: normalizedEmail,
      referral_code,
      referred_by: normalizedRef,
    }),
  })

  if (!insertRes.ok) {
    const errBody = await insertRes.text()

    // PostgREST returns HTTP 409 for BOTH unique violations (23505) AND FK
    // violations (23503). We must parse the error body to distinguish them.
    // Ref: https://docs.postgrest.org/en/v14/references/errors.html
    if (insertRes.status === 409) {
      let pgCode = ''
      try {
        pgCode = (JSON.parse(errBody) as { code?: string }).code || ''
      } catch {}

      // 23505 = unique constraint (email already exists) — race condition with
      // concurrent signup. Re-fetch the winning row and return it.
      if (pgCode === '23505') {
        const retryRes = await supabaseRequest(
          env,
          `/rest/v1/waitlist?email=eq.${encodeURIComponent(normalizedEmail)}&select=position,referral_code`,
        )
        const retryRows = (await retryRes.json()) as WaitlistRow[]
        if (retryRows.length > 0) {
          const total = await getCount(env)
          return Response.json(
            {
              position: retryRows[0].position,
              referral_code: retryRows[0].referral_code,
              total_count: total,
            },
            { headers: CORS_HEADERS },
          )
        }
      }

      // 23503 = FK violation (referred_by code doesn't exist in waitlist table).
      // This happens when a user arrives via a stale or mistyped ?ref= link.
      // Fix: strip the invalid referred_by and retry the insert so the user's
      // signup is NOT silently lost.
      if (pgCode === '23503') {
        console.warn('Invalid referral code, retrying without referred_by:', normalizedRef)
        const retryInsert = await supabaseRequest(env, '/rest/v1/waitlist', {
          method: 'POST',
          headers: { Prefer: 'return=representation' },
          body: JSON.stringify({
            email: normalizedEmail,
            referral_code,
            referred_by: null,
          }),
        })
        if (retryInsert.ok) {
          const [retryInserted] = (await retryInsert.json()) as WaitlistRow[]
          const total = await getCount(env)
          return Response.json(
            {
              position: retryInserted.position,
              referral_code: retryInserted.referral_code,
              total_count: total,
            },
            { headers: CORS_HEADERS },
          )
        }
        // Double-race: 23503 retry also hit 23505 (concurrent signup with same email).
        // Apply the same re-fetch pattern as the top-level 23505 handler.
        const retryErrBody = await retryInsert.text()
        let retryPgCode = ''
        try {
          retryPgCode = (JSON.parse(retryErrBody) as { code?: string }).code || ''
        } catch {}
        if (retryInsert.status === 409 && retryPgCode === '23505') {
          const reFetchRes = await supabaseRequest(
            env,
            `/rest/v1/waitlist?email=eq.${encodeURIComponent(normalizedEmail)}&select=position,referral_code`,
          )
          const reFetchRows = (await reFetchRes.json()) as WaitlistRow[]
          if (reFetchRows.length > 0) {
            const total = await getCount(env)
            return Response.json(
              {
                position: reFetchRows[0].position,
                referral_code: reFetchRows[0].referral_code,
                total_count: total,
              },
              { headers: CORS_HEADERS },
            )
          }
        }
      }
    }

    console.error('Supabase insert error:', errBody)
    return Response.json({ error: 'Something went wrong' }, { status: 500, headers: CORS_HEADERS })
  }

  const [inserted] = (await insertRes.json()) as WaitlistRow[]

  // Increment referrer's count (background — must use waitUntil to survive past response).
  // Without waitUntil, CF Workers cancel non-awaited fetches after the response is sent.
  // Ref: https://developers.cloudflare.com/workers/runtime-apis/context/#waituntil
  if (normalizedRef) {
    context.waitUntil(
      supabaseRequest(env, '/rest/v1/rpc/increment_referral_count', {
        method: 'POST',
        body: JSON.stringify({ ref_code: normalizedRef }),
      }).catch((err) => console.error('Referral increment failed:', err)),
    )
  }

  const total = await getCount(env)
  return Response.json(
    { position: inserted.position, referral_code: inserted.referral_code, total_count: total },
    { headers: CORS_HEADERS },
  )
}

// ─── GET /api/waitlist (count) ───────────────────────────────────────
export const onRequestGet: PagesFunction<Env> = async (context) => {
  const total = await getCount(context.env)
  return Response.json(
    { total_count: total },
    {
      headers: {
        ...CORS_HEADERS,
        'Cache-Control': 'public, max-age=60',
      },
    },
  )
}

// ─── OPTIONS (CORS preflight) ────────────────────────────────────────
export const onRequestOptions: PagesFunction = async () => {
  return new Response(null, { status: 204, headers: CORS_HEADERS })
}
