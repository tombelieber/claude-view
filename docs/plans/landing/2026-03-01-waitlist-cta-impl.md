# Referral Waitlist CTA — Implementation Plan

> **Status:** DONE (2026-03-02) — all 8 tasks implemented, shippable audit passed (SHIP IT)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a viral referral waitlist with Cloudflare Turnstile bot protection to the landing site, resolving L1 blocker #2.

**Architecture:** Static Astro site submits to CF Pages Function (serverless), which validates Turnstile token + honeypot, upserts into Supabase `waitlist` table via REST API, and returns position + referral code. Inline success state shows share buttons.

**Tech Stack:** Astro 5 (static), Cloudflare Pages Functions, Supabase (Postgres + RLS), Cloudflare Turnstile, vanilla JS

**Design doc:** `docs/plans/landing/2026-03-01-waitlist-cta-design.md`

### Completion Summary

| Task | Commit | Description |
|------|--------|-------------|
| 1 | `7d7ba49b` | Supabase waitlist table migration |
| 2 | `693698e4` | Waitlist utility functions with tests |
| 3 | `0b61bce4` | CF Pages Function for waitlist signup + count |
| 4 | `6cbb3a31` | Waitlist constants to site.ts |
| 5 | `cf0eface` | WaitlistForm component with Turnstile + referral |
| 6 | `bae5609d` | Integrate waitlist form into homepage hero + pricing cards |
| 7 | `9345d25a` | Waitlist CTA to mobile-setup docs page |
| 8 | `0e0f6327` | Mark L1 blocker #2 as done |

Shippable audit: 10/10 tests pass, 19-page build clean, tsc clean, 0 blockers. Pre-deploy: replace test Turnstile key + set CF secrets.

---

### Task 1: Supabase Migration — Create Waitlist Table

**Files:**
- Create: `supabase/migrations/20260301_create_waitlist.sql`

**Step 1: Write the migration SQL**

```sql
-- Waitlist table for early access signups with referral tracking
CREATE TABLE public.waitlist (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email TEXT NOT NULL,
  referral_code TEXT NOT NULL,
  referred_by TEXT,
  referral_count INTEGER NOT NULL DEFAULT 0,
  position INTEGER GENERATED ALWAYS AS IDENTITY,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

  CONSTRAINT waitlist_email_unique UNIQUE (email),
  CONSTRAINT waitlist_referral_code_unique UNIQUE (referral_code),
  CONSTRAINT waitlist_referred_by_fk FOREIGN KEY (referred_by) REFERENCES public.waitlist(referral_code)
);

-- Index for referral lookups
CREATE INDEX idx_waitlist_referral_code ON public.waitlist (referral_code);

-- RLS: anon can INSERT only (CF Function uses service_role key, bypasses RLS)
ALTER TABLE public.waitlist ENABLE ROW LEVEL SECURITY;

CREATE POLICY "anon_insert_only" ON public.waitlist
  FOR INSERT TO anon
  WITH CHECK (true);

-- RPC for atomic referral count increment.
-- SECURITY DEFINER runs as function owner (bypasses RLS). search_path pinned
-- to prevent search-path hijack (Supabase best practice for DEFINER functions).
CREATE OR REPLACE FUNCTION increment_referral_count(ref_code TEXT)
RETURNS void
LANGUAGE sql
SECURITY DEFINER
SET search_path = public
AS $$
  UPDATE public.waitlist
  SET referral_count = referral_count + 1
  WHERE referral_code = ref_code;
$$;

-- Lock down RPC access: only service_role can call this function.
-- Without this, anon/authenticated roles (public API keys) could call
-- POST /rest/v1/rpc/increment_referral_count with arbitrary ref_codes.
-- Ref: https://supabase.com/docs/guides/troubleshooting/how-can-i-revoke-execution-of-a-postgresql-function-2GYb0A
REVOKE EXECUTE ON FUNCTION increment_referral_count FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION increment_referral_count FROM anon;
REVOKE EXECUTE ON FUNCTION increment_referral_count FROM authenticated;
GRANT EXECUTE ON FUNCTION increment_referral_count TO service_role;
```

**Step 2: Link Supabase project (one-time setup)**

The repo has no linked Supabase project yet. Initialize and link to enable CLI-driven migrations:

```bash
npx supabase init  # creates supabase/config.toml
npx supabase link --project-ref iebjyftoadahqptmfcio  # claude-view project
```

You'll be prompted for your database password. This is the password you set when creating the Supabase project (NOT the service role key).

**Step 3: Apply migration via CLI**

```bash
npx supabase db push
```

This reads `supabase/migrations/20260301_create_waitlist.sql` and applies it to the remote database. Use `--dry-run` first if you want to preview.

**Step 4: Verify with a test insert**

```bash
npx supabase db execute --sql "INSERT INTO public.waitlist (email, referral_code) VALUES ('test@example.com', 'TESTCODE'); SELECT position, referral_code FROM public.waitlist WHERE email = 'test@example.com';"
# Expected: position=1, referral_code='TESTCODE'
npx supabase db execute --sql "DELETE FROM public.waitlist WHERE email = 'test@example.com';"
```

(Or run in Supabase Dashboard → SQL Editor if you prefer a visual check.)

**Step 5: Commit**

```bash
git add supabase/
git commit -m "feat(landing): add Supabase waitlist table migration + project link"
```

Note: `supabase init` creates `supabase/config.toml` and the `migrations/` directory. Commit the whole `supabase/` directory so future migrations work out of the box.

---

### Task 2: CF Pages Function — Waitlist Utilities + Tests

**Files:**
- Create: `apps/landing/functions/_lib/waitlist-utils.ts`
- Create: `apps/landing/functions-tests/waitlist-utils.test.ts`
- Create: `apps/landing/vitest.config.ts`
- Create: `apps/landing/functions/tsconfig.json`
- Modify: `apps/landing/package.json`

**Step 1: Add dev dependencies + test script**

```bash
cd apps/landing && bun add -d vitest @cloudflare/workers-types wrangler
```

(`wrangler` is needed for `bun run deploy` (`wrangler pages deploy dist`) and local function testing (`wrangler pages dev dist`). Without it in `devDependencies`, `bun run deploy` fails in clean CI environments where wrangler is not globally installed.)

Then add `"test": "vitest run"` to the `scripts` block in `apps/landing/package.json`:

```json
"test": "vitest run"
```

(Without this, `bun run test` at the repo root via Turbo will silently skip the landing tests.)

**Step 1b: Create functions TypeScript config**

Create `apps/landing/functions/tsconfig.json`:

```json
{
  "extends": "../tsconfig.json",
  "compilerOptions": {
    "types": ["@cloudflare/workers-types"],
    "lib": ["ES2022"]
  },
  "include": ["./**/*.ts", "../functions-tests/**/*.ts"]
}
```

This is required because `apps/landing/tsconfig.json` only `include`s `src/` and does not reference `@cloudflare/workers-types`. Without this, `PagesFunction<Env>` will be unresolvable and the `tsc` verification in Task 3 will fail with "Cannot find name 'PagesFunction'".

**Step 2: Create vitest config**

Create `apps/landing/vitest.config.ts`:

```ts
import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    include: ['functions-tests/**/*.test.ts'],
  },
})
```

**Step 3: Write failing tests**

Create `apps/landing/functions-tests/waitlist-utils.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import { generateReferralCode, isValidEmail, checkHoneypot, buildShareText } from '../functions/_lib/waitlist-utils'

describe('generateReferralCode', () => {
  it('generates an 8-character code', () => {
    const code = generateReferralCode()
    expect(code).toHaveLength(8)
  })

  it('uses only URL-safe characters', () => {
    const code = generateReferralCode()
    expect(code).toMatch(/^[A-Za-z0-9]+$/)
  })

  it('generates unique codes', () => {
    const codes = new Set(Array.from({ length: 100 }, () => generateReferralCode()))
    expect(codes.size).toBe(100)
  })
})

describe('isValidEmail', () => {
  it('accepts valid emails', () => {
    expect(isValidEmail('user@example.com')).toBe(true)
    expect(isValidEmail('a.b+tag@sub.domain.co')).toBe(true)
  })

  it('rejects invalid emails', () => {
    expect(isValidEmail('')).toBe(false)
    expect(isValidEmail('not-an-email')).toBe(false)
    expect(isValidEmail('@no-local.com')).toBe(false)
    expect(isValidEmail('no-domain@')).toBe(false)
    expect(isValidEmail('spaces in@email.com')).toBe(false)
  })

  it('rejects emails longer than 254 characters', () => {
    const long = 'a'.repeat(245) + '@test.com'
    expect(isValidEmail(long)).toBe(false)
  })
})

describe('checkHoneypot', () => {
  it('returns true (bot) when honeypot field is filled', () => {
    expect(checkHoneypot('some value')).toBe(true)
  })

  it('returns false (human) when honeypot is empty', () => {
    expect(checkHoneypot('')).toBe(false)
    expect(checkHoneypot(undefined)).toBe(false)
    expect(checkHoneypot(null)).toBe(false)
  })
})

describe('buildShareText', () => {
  it('includes referral link', () => {
    const text = buildShareText('Ab3xK9mQ', 'https://claudeview.ai')
    expect(text).toContain('https://claudeview.ai?ref=Ab3xK9mQ')
  })

  it('is URL-encodable for tweet intent', () => {
    const text = buildShareText('Ab3xK9mQ', 'https://claudeview.ai')
    expect(() => encodeURIComponent(text)).not.toThrow()
  })
})
```

**Step 4: Run tests to verify they fail**

```bash
cd apps/landing && bun run test
```

Expected: FAIL — module not found

**Step 5: Write implementations**

Create `apps/landing/functions/_lib/waitlist-utils.ts`:

```ts
/**
 * Pure utility functions for the waitlist CF Pages Function.
 * Underscore prefix in functions/_lib/ = not treated as a CF Pages route.
 */

const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
const CODE_LENGTH = 8

/** Generate an 8-char URL-safe referral code using crypto.getRandomValues. */
export function generateReferralCode(): string {
  const values = crypto.getRandomValues(new Uint8Array(CODE_LENGTH))
  return Array.from(values, (v) => ALPHABET[v % ALPHABET.length]).join('')
}

/** Validate email format. No MX check — keep it fast. */
export function isValidEmail(email: string): boolean {
  if (!email || email.length > 254) return false
  // RFC 5322 simplified: local@domain, no spaces, at least one dot in domain
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)
}

/** Returns true if the honeypot field was filled (= bot). */
export function checkHoneypot(value: string | undefined | null): boolean {
  return typeof value === 'string' && value.length > 0
}

/** Build share text for X/Twitter intent. */
export function buildShareText(referralCode: string, siteUrl: string): string {
  return `I just joined the claude-view waitlist — Mission Control for AI coding agents. Join me: ${siteUrl}?ref=${referralCode}`
}
```

**Step 6: Run tests to verify they pass**

```bash
cd apps/landing && bun run test
```

Expected: 4 suites, all PASS

**Step 7: Commit**

```bash
git add apps/landing/functions/_lib/waitlist-utils.ts apps/landing/functions-tests/waitlist-utils.test.ts apps/landing/vitest.config.ts apps/landing/functions/tsconfig.json apps/landing/package.json bun.lock
git commit -m "feat(landing): add waitlist utility functions with tests"
```

---

### Task 3: CF Pages Function — Waitlist Handler

**Files:**
- Create: `apps/landing/functions/api/waitlist.ts`

**Step 1: Write the POST handler (signup)**

Create `apps/landing/functions/api/waitlist.ts`:

```ts
import { generateReferralCode, isValidEmail, checkHoneypot } from '../_lib/waitlist-utils'

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
  options: RequestInit = {}
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
      { position: Math.floor(Math.random() * 500) + 100, referral_code: generateReferralCode(), total_count: fakeCount },
      { headers: CORS_HEADERS }
    )
  }

  // Validate Turnstile
  if (!turnstile_token) {
    return Response.json({ error: 'Missing verification token' }, { status: 400, headers: CORS_HEADERS })
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
    { headers: { Accept: 'application/json' } }
  )
  const existingRows = (await existingRes.json()) as WaitlistRow[]
  if (existingRows.length > 0) {
    const total = await getCount(env)
    return Response.json(
      { position: existingRows[0].position, referral_code: existingRows[0].referral_code, total_count: total },
      { headers: CORS_HEADERS }
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
      try { pgCode = (JSON.parse(errBody) as { code?: string }).code || '' } catch {}

      // 23505 = unique constraint (email already exists) — race condition with
      // concurrent signup. Re-fetch the winning row and return it.
      if (pgCode === '23505') {
        const retryRes = await supabaseRequest(
          env,
          `/rest/v1/waitlist?email=eq.${encodeURIComponent(normalizedEmail)}&select=position,referral_code`
        )
        const retryRows = (await retryRes.json()) as WaitlistRow[]
        if (retryRows.length > 0) {
          const total = await getCount(env)
          return Response.json(
            { position: retryRows[0].position, referral_code: retryRows[0].referral_code, total_count: total },
            { headers: CORS_HEADERS }
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
            { position: retryInserted.position, referral_code: retryInserted.referral_code, total_count: total },
            { headers: CORS_HEADERS }
          )
        }
        // Double-race: 23503 retry also hit 23505 (concurrent signup with same email).
        // Apply the same re-fetch pattern as the top-level 23505 handler.
        const retryErrBody = await retryInsert.text()
        let retryPgCode = ''
        try { retryPgCode = (JSON.parse(retryErrBody) as { code?: string }).code || '' } catch {}
        if (retryInsert.status === 409 && retryPgCode === '23505') {
          const reFetchRes = await supabaseRequest(
            env,
            `/rest/v1/waitlist?email=eq.${encodeURIComponent(normalizedEmail)}&select=position,referral_code`
          )
          const reFetchRows = (await reFetchRes.json()) as WaitlistRow[]
          if (reFetchRows.length > 0) {
            const total = await getCount(env)
            return Response.json(
              { position: reFetchRows[0].position, referral_code: reFetchRows[0].referral_code, total_count: total },
              { headers: CORS_HEADERS }
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
      }).catch((err) => console.error('Referral increment failed:', err))
    )
  }

  const total = await getCount(env)
  return Response.json(
    { position: inserted.position, referral_code: inserted.referral_code, total_count: total },
    { headers: CORS_HEADERS }
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
    }
  )
}

// ─── OPTIONS (CORS preflight) ────────────────────────────────────────
export const onRequestOptions: PagesFunction = async () => {
  return new Response(null, { status: 204, headers: CORS_HEADERS })
}
```

**Step 2: Verify function compiles**

```bash
cd apps/landing && npx tsc --noEmit --project functions/tsconfig.json 2>&1 || echo "Type errors found — fix before proceeding"
```

Note: The raw `tsc ... waitlist.ts` command without `--project` fails with "Cannot find name 'PagesFunction'" because it ignores the `@cloudflare/workers-types` types. Use the `functions/tsconfig.json` created in Task 2 Step 1b instead.

**Step 3: Commit**

```bash
git add apps/landing/functions/api/waitlist.ts
git commit -m "feat(landing): add CF Pages Function for waitlist signup + count"
```

Note: `functions/_lib/` uses underscore prefix because Cloudflare Pages Functions only register routes for files that export `onRequest*` handlers. The `waitlist-utils.ts` utility file exports no handlers and will be silently skipped by the CF router. If `wrangler pages dev` throws errors about `_lib/` files, move utilities to `apps/landing/src/lib/waitlist-utils.ts` and update all import paths accordingly.

---

### Task 4: Constants + Turnstile Site Key

**Files:**
- Modify: `apps/landing/src/data/site.ts`

**Step 1: Add waitlist constants to site.ts**

Add to the end of `apps/landing/src/data/site.ts`:

```ts
// ---------------------------------------------------------------------------
// Waitlist
// ---------------------------------------------------------------------------

export const WAITLIST_API = '/api/waitlist'

/**
 * Cloudflare Turnstile site key (public, safe to embed in client code).
 * ⚠️  REPLACE BEFORE DEPLOYING TO PRODUCTION — see Task 8 Step 2.
 * Current value '1x00000000000000000000AA' is Cloudflare's always-passes TEST key.
 * Deploying with this key means ALL bot submissions pass Turnstile verification.
 */
export const TURNSTILE_SITE_KEY = '1x00000000000000000000AA' // TODO(deploy): replace with real site key
```

**Step 2: Commit**

```bash
git add apps/landing/src/data/site.ts
git commit -m "feat(landing): add waitlist constants to site.ts"
```

---

### Task 5: WaitlistForm Astro Component

**Files:**
- Create: `apps/landing/src/components/WaitlistForm.astro`

**Step 1: Create the component**

Create `apps/landing/src/components/WaitlistForm.astro`:

```astro
---
import { WAITLIST_API, SITE_URL, TURNSTILE_SITE_KEY } from '../data/site';

interface Props {
  /** 'default' for hero/feature sections, 'compact' for docs pages */
  variant?: 'default' | 'compact'
}

const { variant = 'default' } = Astro.props
const isCompact = variant === 'compact'
---

<div
  class:list={['waitlist-form', { 'max-w-md mx-auto': isCompact, 'max-w-lg mx-auto': !isCompact }]}
  data-api={WAITLIST_API}
  data-site-url={SITE_URL}
  data-site-key={TURNSTILE_SITE_KEY}
  id="waitlist"
>
  <!-- NOTE: id="waitlist" is used as the scroll target for all '#waitlist' anchors.
       Only place ONE WaitlistForm per page — duplicate ids violate HTML uniqueness.
       If you need a second CTA on the same page, use <a href="#waitlist"> to scroll to this one. -->
  <!-- Social proof counter -->
  <p class:list={['waitlist-counter text-slate-400 mb-3', isCompact ? 'text-xs' : 'text-sm']}>
    <span class="waitlist-count-text">Join the early access waitlist</span>
  </p>

  <!-- Form state: default -->
  <form class="waitlist-default flex flex-col sm:flex-row gap-2">
    <input
      type="email"
      name="email"
      required
      autocomplete="email"
      placeholder="you@example.com"
      class:list={[
        'flex-1 rounded-lg border border-slate-700 bg-slate-900/80 text-white placeholder-slate-500 focus:border-green-500 focus:outline-none focus:ring-1 focus:ring-green-500 transition-colors',
        isCompact ? 'px-3 py-2 text-sm' : 'px-4 py-3',
      ]}
    />
    <!-- Honeypot: invisible to humans, bots fill it -->
    <input type="text" name="company" tabindex="-1" autocomplete="off" aria-hidden="true" style="position:absolute;left:-9999px;opacity:0;height:0;width:0;" />
    <!-- Turnstile widget container -->
    <div class="turnstile-container"></div>
    <button
      type="submit"
      class:list={[
        'waitlist-submit rounded-lg font-medium transition-colors bg-green-600 text-white hover:bg-green-500 disabled:opacity-50 disabled:cursor-not-allowed whitespace-nowrap',
        isCompact ? 'px-4 py-2 text-sm' : 'px-6 py-3',
      ]}
    >
      Join waitlist
    </button>
  </form>

  <!-- Error message -->
  <p class="waitlist-error text-red-400 text-sm mt-2 hidden"></p>

  <!-- Success state (hidden by default) -->
  <div class="waitlist-success hidden">
    <p class:list={['font-semibold text-green-400 mb-3', isCompact ? 'text-base' : 'text-lg']}>
      You're <span class="waitlist-position font-bold">#—</span> on the waitlist!
    </p>
    <p class="text-slate-400 text-sm mb-3">Share to help us grow:</p>
    <div class="flex flex-col sm:flex-row gap-2 mb-3">
      <div class="flex-1 flex items-center gap-2 rounded-lg border border-slate-700 bg-slate-900/80 px-3 py-2">
        <code class="waitlist-referral-link text-green-400 text-sm truncate flex-1"></code>
        <button
          type="button"
          class="waitlist-copy text-slate-400 hover:text-white transition-colors text-sm font-medium whitespace-nowrap"
          aria-label="Copy referral link"
        >
          Copy
        </button>
      </div>
    </div>
    <a
      class="waitlist-share-x inline-flex items-center gap-2 rounded-lg border border-slate-700 px-4 py-2 text-sm text-slate-300 hover:text-white hover:border-slate-500 transition-colors"
      href="#"
      target="_blank"
      rel="noopener noreferrer"
    >
      <svg class="w-4 h-4" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/>
      </svg>
      Share on X
    </a>
  </div>
</div>

<script>
  function initWaitlistForms() {
    const forms = document.querySelectorAll<HTMLDivElement>('.waitlist-form')
    const refParam = new URLSearchParams(window.location.search).get('ref')

    forms.forEach((container) => {
      // Guard: skip containers already initialized.
      // Without this, `astro:after-swap` fires on every page navigation re-run
      // and adds duplicate submit/copy event listeners, causing N API calls per submit
      // after N page navigations.
      if (container.dataset.initialized) return
      container.dataset.initialized = 'true'

      const api = container.dataset.api!
      const siteUrl = container.dataset.siteUrl!
      const siteKey = container.dataset.siteKey!

      const form = container.querySelector<HTMLFormElement>('.waitlist-default')!
      const emailInput = form.querySelector<HTMLInputElement>('input[name="email"]')!
      const honeypot = form.querySelector<HTMLInputElement>('input[name="company"]')!
      const submitBtn = form.querySelector<HTMLButtonElement>('.waitlist-submit')!
      const errorEl = container.querySelector<HTMLParagraphElement>('.waitlist-error')!
      const successEl = container.querySelector<HTMLDivElement>('.waitlist-success')!
      const counterEl = container.querySelector<HTMLParagraphElement>('.waitlist-counter')!
      const countTextEl = container.querySelector<HTMLSpanElement>('.waitlist-count-text')!
      const positionEl = container.querySelector<HTMLSpanElement>('.waitlist-position')!
      const linkEl = container.querySelector<HTMLElement>('.waitlist-referral-link')!
      const copyBtn = container.querySelector<HTMLButtonElement>('.waitlist-copy')!
      const shareXLink = container.querySelector<HTMLAnchorElement>('.waitlist-share-x')!
      const turnstileContainer = container.querySelector<HTMLDivElement>('.turnstile-container')!

      // Load Turnstile SDK and render widget
      function loadTurnstile() {
        if ((window as any).turnstile) {
          renderTurnstile()
          return
        }
        // Guard: only inject Turnstile SDK once per page (multiple forms would each call this).
        // The script tag guard also ensures window.onTurnstileLoad is only set once —
        // by whichever form runs loadTurnstile() first. That callback iterates ALL
        // .waitlist-form containers, so every form on the page gets a widget regardless
        // of which form registered the callback.
        if (document.querySelector('script[src*="challenges.cloudflare.com/turnstile"]')) {
          return
        }
        const script = document.createElement('script')
        script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit&onload=onTurnstileLoad'
        script.async = true
        ;(window as any).onTurnstileLoad = () => {
          document.querySelectorAll<HTMLDivElement>('.waitlist-form').forEach((c) => {
            const tc = c.querySelector<HTMLDivElement>('.turnstile-container')
            if (tc && !tc.dataset.rendered) {
              tc.dataset.rendered = 'true'
              // Capture widgetId to enable reset on token expiry
              const widgetId = (window as any).turnstile.render(tc, {
                sitekey: c.dataset.siteKey,
                callback: (token: string) => {
                  c.dataset.turnstileToken = token
                },
                'expired-callback': () => {
                  c.dataset.turnstileToken = ''
                  // Re-issue a fresh token automatically — user doesn't see an error
                  ;(window as any).turnstile.reset(widgetId)
                },
                size: 'flexible',
                theme: 'dark',
              })
            }
          })
        }
        document.head.appendChild(script)
      }

      function renderTurnstile() {
        if (turnstileContainer.dataset.rendered) return
        turnstileContainer.dataset.rendered = 'true'
        const widgetId = (window as any).turnstile.render(turnstileContainer, {
          sitekey: siteKey,
          callback: (token: string) => { container.dataset.turnstileToken = token },
          'expired-callback': () => {
            container.dataset.turnstileToken = ''
            ;(window as any).turnstile.reset(widgetId)
          },
          size: 'flexible',
          theme: 'dark',
        })
      }

      loadTurnstile()

      // Fetch waitlist count for social proof
      fetch(api)
        .then((r) => r.json())
        .then((data: any) => {
          if (data.total_count > 0) {
            countTextEl.textContent = `Join ${data.total_count.toLocaleString()} developers on the waitlist`
          }
        })
        .catch(() => {}) // Silent fail — counter is nice-to-have

      // Form submission
      form.addEventListener('submit', async (e) => {
        e.preventDefault()
        errorEl.classList.add('hidden')
        errorEl.textContent = ''

        const email = emailInput.value.trim()
        if (!email) {
          errorEl.textContent = 'Please enter your email'
          errorEl.classList.remove('hidden')
          return
        }

        const token = container.dataset.turnstileToken
        if (!token) {
          errorEl.textContent = 'Verification loading — please try again in a moment'
          errorEl.classList.remove('hidden')
          return
        }

        submitBtn.disabled = true
        submitBtn.textContent = 'Joining...'

        try {
          const res = await fetch(api, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
              email,
              ref: refParam || undefined,
              turnstile_token: token,
              company: honeypot.value, // Honeypot value (empty for humans)
            }),
          })

          const data = await res.json()

          if (!res.ok) {
            errorEl.textContent = data.error || 'Something went wrong. Please try again.'
            errorEl.classList.remove('hidden')
            submitBtn.disabled = false
            submitBtn.textContent = 'Join waitlist'
            return
          }

          // Success — transform to success state
          const referralUrl = `${siteUrl}?ref=${data.referral_code}`
          positionEl.textContent = `#${data.position}`
          linkEl.textContent = referralUrl

          const shareText = encodeURIComponent(
            `I just joined the claude-view waitlist — Mission Control for AI coding agents. Join me:`
          )
          shareXLink.href = `https://x.com/intent/tweet?text=${shareText}&url=${encodeURIComponent(referralUrl)}`

          form.classList.add('hidden')
          counterEl.classList.add('hidden')
          successEl.classList.remove('hidden')
        } catch {
          errorEl.textContent = 'Network error. Please check your connection and try again.'
          errorEl.classList.remove('hidden')
          submitBtn.disabled = false
          submitBtn.textContent = 'Join waitlist'
        }
      })

      // Copy referral link
      copyBtn.addEventListener('click', async () => {
        const link = linkEl.textContent || ''
        try {
          await navigator.clipboard.writeText(link)
          copyBtn.textContent = 'Copied!'
          setTimeout(() => { copyBtn.textContent = 'Copy' }, 2000)
        } catch {
          // Fallback
          const textarea = document.createElement('textarea')
          textarea.value = link
          textarea.style.position = 'fixed'
          textarea.style.opacity = '0'
          document.body.appendChild(textarea)
          textarea.select()
          document.execCommand('copy')
          document.body.removeChild(textarea)
          copyBtn.textContent = 'Copied!'
          setTimeout(() => { copyBtn.textContent = 'Copy' }, 2000)
        }
      })
    })
  }

  // Run on initial load and after Astro view transitions
  initWaitlistForms()
  document.addEventListener('astro:after-swap', initWaitlistForms)
</script>
```

**Step 2: Verify component renders**

```bash
cd apps/landing && bun run build 2>&1 | tail -5
```

Expected: Build succeeds (component isn't used yet, but should compile)

**Step 3: Commit**

```bash
git add apps/landing/src/components/WaitlistForm.astro
git commit -m "feat(landing): add WaitlistForm component with Turnstile + referral"
```

---

### Task 6: Homepage Integration

**Prerequisite: Task 5 must be complete** — `apps/landing/src/components/WaitlistForm.astro` must exist before this task's build step.

**Files:**
- Modify: `apps/landing/src/pages/index.astro` (imports block, hero CTA div ~line 45, mobile section ~lines 104-106)
- Modify: `apps/landing/src/components/PricingCards.astro` (comingSoon `<button>` block ~lines 34-39)

**Step 1: Replace AppStoreBadges import with WaitlistForm import in index.astro**

In `apps/landing/src/pages/index.astro`, replace the `AppStoreBadges` import:

```astro
import AppStoreBadges from '../components/AppStoreBadges.astro';
```

With:

```astro
import WaitlistForm from '../components/WaitlistForm.astro';
```

(The `AppStoreBadges` component is being replaced by a scroll link in Step 3 — remove the import now to avoid an Astro build warning about an unused import.)

**Step 2: Add WaitlistForm to hero section**

In `apps/landing/src/pages/index.astro`, after the existing hero CTA row (after line 45, after the `</div>` that closes the `flex` row with InstallCommand + GitHubStars), add:

```astro
      <div class="mt-6">
        <WaitlistForm />
      </div>
```

**Step 3: Replace AppStoreBadges with scroll-to link in mobile section**

In `apps/landing/src/pages/index.astro`, replace the AppStoreBadges block (lines 104-106):

```astro
  <div class="mt-6">
    <AppStoreBadges />
  </div>
```

With:

```astro
  <div class="mt-6 text-center">
    <a
      href="#waitlist"
      class="inline-flex items-center gap-2 rounded-lg border border-green-500/30 bg-green-500/10 px-6 py-3 text-sm font-medium text-green-400 hover:bg-green-500/20 hover:border-green-500/50 transition-colors"
    >
      Join the mobile waitlist
      <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <path d="M7 17L17 7M17 7H7M17 7V17" />
      </svg>
    </a>
  </div>
```

**Step 4: Update PricingCards — replace disabled Coming Soon with waitlist link**

In `apps/landing/src/components/PricingCards.astro`, replace only the `<button>` element inside the `comingSoon` branch (the inner lines 34-39 — keep line 33 `{tier.comingSoon ? (` and everything from line 40 onward intact):

Replace:

```astro
        <button
          disabled
          class="block w-full text-center py-3 px-6 rounded-lg font-medium text-sm bg-slate-800 text-slate-400 cursor-not-allowed opacity-70"
        >
          {tier.cta}
        </button>
```

With:

```astro
        <a
          href="#waitlist"
          class="block w-full text-center py-3 px-6 rounded-lg font-medium text-sm transition-colors border border-green-500/30 bg-green-500/10 text-green-400 hover:bg-green-500/20 hover:border-green-500/50"
        >
          Join Waitlist
        </a>
```

Important: Do NOT replace line 33 (`{tier.comingSoon ? (`) or line 40 (`) : (`). Only the `<button>...</button>` body changes. The ternary structure is preserved.

**Step 5: Update PricingCards site.ts — change CTA text**

In `apps/landing/src/data/site.ts`, update the Pro and Team tier `cta` values:

Change `cta: 'Coming Soon'` to `cta: 'Join Waitlist'` for both Pro and Team tiers. (Note: the PricingCards hardcodes "Join Waitlist" in the anchor text above, so the `tier.cta` value in site.ts won't be displayed for comingSoon tiers anymore. Update anyway for consistency.)

**Step 6: Fix "No signup" text conflict**

In `apps/landing/src/pages/index.astro`, the Install CTA section contains text that contradicts the new waitlist form. Update it:

Replace:

```astro
No signup. No config. Just run one command.
```

With:

```astro
No account needed. Just run one command.
```

(The original "No signup" claim directly contradicts the waitlist form. "No account needed" is still accurate — the waitlist is for early access, not for using the local tool.)

**Step 7: Build and verify**

```bash
cd apps/landing && bun run build && echo "Build OK"
```

Expected: Build succeeds. Open `dist/index.html` in browser to visually verify:
- Waitlist form visible in hero section
- "Join the mobile waitlist" link in mobile features section
- "Join Waitlist" buttons in Pro/Team pricing cards (linking to #waitlist)

**Step 8: Commit**

```bash
git add apps/landing/src/pages/index.astro apps/landing/src/components/PricingCards.astro apps/landing/src/data/site.ts
git commit -m "feat(landing): integrate waitlist form into homepage hero + pricing cards"
```

---

### Task 7: Mobile-Setup Docs Integration

**Files:**
- Modify: `apps/landing/src/content/docs/docs/guides/mobile-setup.mdx`

Pre-check: confirm the file exists before editing (it does as of plan authoring — audited 2026-03-01):

```bash
ls apps/landing/src/content/docs/docs/guides/mobile-setup.mdx
```

**Step 1: Add WaitlistForm to mobile-setup docs page**

MDX imports must come before ANY prose content — they cannot appear mid-document. This requires two separate edits:

**Edit A:** Insert the import on the blank line immediately after the frontmatter closing `---`. The file currently looks like:

```mdx
---
title: Mobile App Setup
description: Set up the claude-view mobile app on iOS or Android.
---

The claude-view mobile app lets you...
```

Replace the blank line between `---` and the first paragraph with:

```mdx
import WaitlistForm from '../../../../components/WaitlistForm.astro';
```

Result:

```mdx
---
title: Mobile App Setup
description: Set up the claude-view mobile app on iOS or Android.
---
import WaitlistForm from '../../../../components/WaitlistForm.astro';

The claude-view mobile app lets you...
```

(Four `../` levels up from `src/content/docs/docs/guides/` reaches `src/`, then `components/WaitlistForm.astro`. Confirmed correct.)

**Edit B:** Replace the status callout (currently reads `> **Status:** The mobile app is currently in development.`):

```mdx
> **Status:** The mobile app is currently in development.
```

With:

```mdx
> **Status:** The mobile app is currently in development. Join the waitlist to be notified when it launches.

<WaitlistForm variant="compact" />
```

Note: Starlight's MDX pipeline (via `@astrojs/mdx`) supports importing `.astro` components inside content collection MDX files. The `bun run build` in Step 2 will confirm this works in this project's specific configuration.

**Fallback if build fails with MDX/Astro component error:** If Starlight rejects the direct `.astro` import, replace the `<WaitlistForm variant="compact" />` usage with a plain anchor link instead:

```mdx
<a href="/#waitlist" style="display:inline-block;padding:0.6rem 1.2rem;border-radius:0.5rem;background:rgba(34,197,94,0.1);border:1px solid rgba(34,197,94,0.3);color:rgb(74,222,128);font-size:0.875rem;font-weight:500;text-decoration:none;">
  Join the waitlist →
</a>
```

This links back to the hero section form on the homepage and requires no component import. Use this if the component import approach fails the build.

**Step 2: Build and verify**

```bash
cd apps/landing && bun run build && echo "Build OK"
```

Expected: Build succeeds. Navigate to `/docs/guides/mobile-setup/` to verify the compact waitlist form renders.

**Step 3: Commit**

```bash
git add apps/landing/src/content/docs/docs/guides/mobile-setup.mdx
git commit -m "feat(landing): add waitlist CTA to mobile-setup docs page

Resolves L1 blocker #2: mobile setup CTA now wired to real signup."
```

---

### Task 8: Deployment Config + E2E Verification

**Files:**
- Modify: `apps/landing/wrangler.toml` (optional, for local dev)
- Modify: `docs/plans/landing/2026-03-01-landing-page-followup.md` (mark item #2 done)

**Step 1: Create Turnstile widget via CF API**

Create the Turnstile widget using the Cloudflare API (no dashboard needed):

```bash
# Your account ID (from `wrangler whoami`)
CF_ACCOUNT_ID="96887e7bf8b696172bc5cbed241ed409"

# Create widget — returns sitekey + secret
curl -s -X POST "https://api.cloudflare.com/client/v4/accounts/$CF_ACCOUNT_ID/challenges/widgets" \
  -H "Authorization: Bearer $(cat ~/.wrangler/config/default.toml 2>/dev/null | grep oauth_token | cut -d'"' -f2 || echo 'YOUR_API_TOKEN')" \
  -H "Content-Type: application/json" \
  -d '{
    "domains": ["claudeview.ai"],
    "mode": "managed",
    "name": "claudeview-waitlist"
  }' | jq '.result | {sitekey, secret}'
```

If the `Authorization` header doesn't work with the OAuth token file, use a CF API token instead:

```bash
# Alternative: use wrangler to list existing widgets (if already created via dashboard)
curl -s "https://api.cloudflare.com/client/v4/accounts/$CF_ACCOUNT_ID/challenges/widgets" \
  -H "Authorization: Bearer $CF_API_TOKEN" | jq '.result[] | {name, sitekey, secret}'
```

From the output, save:
- `sitekey` → update `TURNSTILE_SITE_KEY` in `apps/landing/src/data/site.ts`
- `secret` → used in Step 2 below

Ref: [CF Turnstile Widget Management API](https://developers.cloudflare.com/turnstile/get-started/widget-management/api/)

**Step 2: Set CF Pages environment secrets via CLI**

```bash
cd apps/landing

# Supabase URL (your project: iebjyftoadahqptmfcio)
echo "https://iebjyftoadahqptmfcio.supabase.co" | wrangler pages secret put SUPABASE_URL --project-name claude-view-landing

# Supabase Secret key — get from: Dashboard → Settings → API → Secret keys tab
wrangler pages secret put SUPABASE_SECRET_KEY --project-name claude-view-landing
# (paste the sb_secret_* key when prompted)

# Turnstile secret key (from Step 1 output)
wrangler pages secret put TURNSTILE_SECRET_KEY --project-name claude-view-landing
# (paste the secret when prompted)
```

Verify all 3 secrets are set:

```bash
wrangler pages secret list --project-name claude-view-landing
```

**Step 2b: Local function testing (before production deploy)**

The `astro dev` server does NOT serve `functions/` routes — it only serves static HTML. To test `/api/waitlist` locally before deploying:

```bash
cd apps/landing
bun run build
# Create .dev.vars for local secrets (gitignored):
cat > .dev.vars << 'EOF'
SUPABASE_URL=https://your-project.supabase.co
SUPABASE_SECRET_KEY=your-sb-secret-key
TURNSTILE_SECRET_KEY=1x0000000000000000000000000000000AA
EOF
bunx wrangler pages dev dist
```

This starts a local CF Pages server at `http://localhost:8788` that serves both the static site AND the CF Functions. Use the Turnstile secret `1x0000000000000000000000000000000AA` (always-passes test secret) for local testing.

**Step 3: Deploy**

```bash
cd apps/landing && bun run build && bun run deploy
```

(Uses the existing `deploy` script in `apps/landing/package.json` which runs `wrangler pages deploy dist`. The project name `claude-view-landing` is read from `wrangler.toml`'s `name` field.)

**Step 4: E2E verification**

Test the following on `https://claudeview.ai`:

1. **Homepage hero** — Waitlist form visible below install command
2. **Social proof** — Counter shows "Join N developers" (or "Join the early access waitlist" if count is 0)
3. **Submit email** — Enter test email, verify success state appears with position + referral link
4. **Copy referral link** — Click Copy, paste in new tab, verify `?ref=` param is in URL
5. **Referral flow** — Open referral link in incognito, submit a different email, verify the original user's referral_count incremented (check in Supabase dashboard)
6. **Share on X** — Click "Share on X", verify tweet intent URL is correct
7. **Duplicate submit** — Submit the same email again, verify same position + code returned
8. **Pricing cards** — Click "Join Waitlist" on Pro/Team, verify smooth scroll to hero form
9. **Mobile section** — Click "Join the mobile waitlist", verify smooth scroll
10. **Mobile-setup docs** — Navigate to `/docs/guides/mobile-setup/`, verify compact form works
11. **Bot protection** — Verify Turnstile widget renders (small badge in corner)
12. **Stale referral link** — Open `?ref=NONEXISTENT` in incognito, submit an email. Verify signup succeeds (user gets a position), NOT a 500 error. Check Supabase: `referred_by` should be NULL for that row.

**Step 5: Update follow-up doc**

In `docs/plans/landing/2026-03-01-landing-page-followup.md`, update item #2 from BLOCKED to DONE:

```markdown
| 2 | ~~**Mobile setup "Sign up for early access" has no signup mechanism**~~ | `src/content/docs/docs/guides/mobile-setup.mdx` | **DONE** — Referral waitlist with Cloudflare Turnstile. Form in homepage hero + mobile-setup docs. Pricing cards link to waitlist. Supabase `waitlist` table with referral tracking. |
```

**Step 6: Commit**

```bash
git add docs/plans/landing/2026-03-01-landing-page-followup.md
git commit -m "docs(landing): mark L1 blocker #2 as done — waitlist deployed"
```

Note: `apps/landing/src/data/site.ts` was already committed in Task 4 (WAITLIST_API/TURNSTILE_SITE_KEY) and Task 6 (cta text). Do not re-add it here — it would have no staged changes by this point.

---

## Changelog of Fixes Applied (Audit → Final Plan)

Audited 2026-03-01 via `auditing-plans` (round 1: 24 fixes). Re-audited via `prove-it` + `auditing-plans` (round 2: 3 fixes). Re-audited 2026-03-02 via `prove-it` + `auditing-plans` (round 3: 7 fixes). Round 4 (2026-03-02): CLI-first infra automation + domain fix (4 fixes). All issues resolved before execution.

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `supabase/` directory doesn't exist — `git add` would fail | Blocker | Added `mkdir -p supabase/migrations` step before Task 1 Step 4 commit; added note about manual-only migration (no Supabase CLI linked) |
| 2 | No `"test"` script in `apps/landing/package.json` — Turbo silently skips landing tests | Blocker | Task 2 Step 1 now explicitly adds `"test": "vitest run"` to package.json scripts |
| 3 | `PagesFunction<Env>` type unresolvable — `tsconfig.json` excludes `functions/` and has no `@cloudflare/workers-types` reference | Blocker | Added Task 2 Step 1b: create `apps/landing/functions/tsconfig.json` with `"types": ["@cloudflare/workers-types"]` |
| 4 | Task 3 Step 2 `tsc` command ignores `@cloudflare/workers-types` — "Cannot find name 'PagesFunction'" | Blocker | Changed to `npx tsc --noEmit --project functions/tsconfig.json` |
| 5 | `AppStoreBadges` import never removed after its usage is replaced in Task 6 Step 3 | Blocker | Task 6 Step 1 now replaces the `AppStoreBadges` import with `WaitlistForm` import (not additive insert) |
| 6 | PricingCards replacement range said "lines 33-39" but line 33 (`{tier.comingSoon ? (`) must be preserved | Blocker | Rewrote Step 4 to show exact `<button>...</button>` replacement (not the ternary opener); added explicit "do not touch line 33" note |
| 7 | MDX import at "line 8" invalid — MDX imports must precede ALL prose content | Blocker | Task 7 Step 1 split into Edit A (import after frontmatter `---`) + Edit B (replace status callout) |
| 8 | Task 8 Step 6 commit re-added `site.ts` which was already committed in Tasks 4 and 6 | Blocker | Removed `apps/landing/src/data/site.ts` from Task 8 Step 6 commit |
| 9 | `bunx vitest run` could pull a different version than locally installed vitest | Warning | Changed all `bunx vitest run` to `bun run test` |
| 10 | No local dev workflow for CF Functions — `astro dev` serves no `functions/` routes; first test would be in production | Warning | Added Task 8 Step 2b: `wrangler pages dev dist` with `.dev.vars` setup |
| 11 | Deploy command bypassed `package.json` deploy script | Warning | Changed `bunx wrangler pages deploy dist --project-name=...` to `bun run deploy` |
| 12 | `functions/tsconfig.json` not staged in Task 2 Step 7 commit | Minor | Added `apps/landing/functions/tsconfig.json` to Task 2 Step 7 git add |
| 13 | Task 6 had no explicit dependency on Task 5; out-of-order execution would fail the build | Warning | Added "Prerequisite: Task 5 must be complete" to Task 6 header |
| 14 | No fallback if Starlight MDX rejects `.astro` component import in docs page | Warning | Task 7 Step 1 now includes a plain-anchor fallback strategy |
| 15 | Task 6 file header line numbers inaccurate (said "103-107" for mobile section, actual is 104-106) | Minor | Updated file header annotation to use `~` approximations rather than exact fragile line numbers |
| 16 | `astro:after-swap` listener accumulates on every navigation — duplicate submit handlers send N API calls | Blocker | Added `container.dataset.initialized` guard at top of `initWaitlistForms` forEach (proof: ClientRouter confirmed in MarketingLayout.astro) |
| 17 | Turnstile `expired-callback` clears token but never calls `turnstile.reset(widgetId)` — user stuck after 5 min | Blocker | Captured `widgetId` from `turnstile.render()` in both `renderTurnstile()` and `onTurnstileLoad`; added `.reset(widgetId)` in expired-callback |
| 18 | `loadTurnstile()` appends a second SDK `<script>` tag if called by 2+ forms before SDK loads | Blocker | Added `document.querySelector('script[src*="challenges.cloudflare.com/turnstile"]')` guard before `appendChild` |
| 19 | `getCount()` uses `method: 'HEAD'` which causes `supabaseRequest` helper to set `Content-Type: application/json` on a bodyless request | Warning | Changed to default GET + `Range: 0-0` header (documented Supabase count pattern) |
| 20 | `TURNSTILE_SITE_KEY` test value `1x00000000000000000000AA` has no deployment guard — silent bot bypass if Task 8 Step 2 skipped | Warning | Added `⚠️ REPLACE BEFORE DEPLOYING` JSDoc + inline `// TODO(deploy):` comment |
| 21 | `mobile-setup.mdx` Edit A instruction ambiguous about blank-line handling — import could land after first paragraph | Warning | Rewrote to show exact before/after diff including the blank line |
| 22 | `id="waitlist"` in reusable component has no warning — future editors could place two forms on one page breaking anchor scroll | Minor | Added HTML comment inside component div documenting the one-form-per-page constraint |
| 23 | `onTurnstileLoad` shared-callback pattern unintuitive — could be misread as a bug | Minor | Added explanatory comment clarifying why single callback + all-forms iteration is correct |
| 24 | Task 7 edits a file without verifying it exists first — `git add` would silently fail if file moved | Minor | Added `ls` pre-check step to Task 7 |
| 25 | FK violation on invalid `referred_by` (stale `?ref=` link) returns 409, handler re-fetches by email, finds nothing, falls to 500 — user signup silently lost | **Blocker** | Task 3: rewrote 409 handler to parse PostgREST `code` field. `23505` (duplicate email) → re-fetch existing row. `23503` (FK violation / bad ref) → strip `referred_by` and retry insert. Added E2E test #12 for stale referral links. Evidence: [PostgREST error docs](https://docs.postgrest.org/en/v14/references/errors.html) confirm both map to HTTP 409 |
| 26 | `increment_referral_count` RPC callable by `anon` role — referral counts gameable via public anon key. Also missing `search_path` on `SECURITY DEFINER` function | **Blocker** | Task 1: added `REVOKE EXECUTE FROM PUBLIC, anon, authenticated` + `GRANT EXECUTE TO service_role` + `SET search_path = public`. Evidence: [Supabase docs](https://supabase.com/docs/guides/troubleshooting/how-can-i-revoke-execution-of-a-postgresql-function-2GYb0A) |
| 27 | `wrangler` not in `devDependencies` — `bun run deploy` and `wrangler pages dev` fail in clean CI/fresh clone | Warning | Task 2 Step 1: added `wrangler` to `bun add -d` command |
| 28 | Fire-and-forget `supabaseRequest` for referral increment is canceled by CF Workers runtime after response is sent — referral counts silently lost | **Blocker** | Task 3: wrapped with `context.waitUntil()`. CF Workers cancel non-awaited fetches after response body is sent. `waitUntil()` extends execution up to 30s. Evidence: [CF Workers Context docs](https://developers.cloudflare.com/workers/runtime-apis/context/#waituntil) |
| 29 | Turnstile SDK URL missing `render=explicit` — manual `turnstile.render()` call is unsupported without it | **Blocker** | Task 5: added `render=explicit` to SDK script URL. CF docs require `render=explicit` for programmatic widget rendering via `turnstile.render()`. Evidence: [CF Turnstile client-side rendering](https://developers.cloudflare.com/turnstile/get-started/client-side-rendering/) |
| 30 | `bun.lock` modified by `bun add -d` in Task 2 Step 1 but not staged in Task 2 Step 7 commit — permanently dirty working tree | **Blocker** | Task 2 Step 7: added `bun.lock` to `git add` command |
| 31 | Honeypot response uses hardcoded `total_count: 999` — detectable fingerprint vs real count | Warning | Task 3: replaced with `await getCount(env)` to return real count |
| 32 | `ref` field has no format validation — oversized/malformed strings reach Supabase before FK check rejects them | Warning | Task 3: added `normalizedRef` guard — must be exactly 8 alphanumeric chars (our code format). All `ref` references changed to `normalizedRef` |
| 33 | Nested 23503→23505 double-race: FK retry hits unique constraint, falls to 500 | Warning | Task 3: added 23505 re-fetch after the 23503 retry insert, mirroring the top-level 23505 handler |
| 34 | "No signup. No config." text in index.astro Install CTA section contradicts waitlist form | Warning | Task 6: added Step 6 to update text to "No account needed. Just run one command." |
| 35 | Task 1 says "paste in Supabase Dashboard SQL Editor" — automatable via `supabase link` + `supabase db push` CLI | Improvement | Task 1: replaced manual SQL Editor steps with `supabase init` + `supabase link --project-ref` + `supabase db push`. Commits `supabase/` directory (config.toml + migrations) |
| 36 | Task 8 says "create Turnstile site in CF Dashboard" — automatable via CF API `POST /challenges/widgets` | Improvement | Task 8 Step 1: added `curl` command to create Turnstile widget via CF API, returning sitekey + secret. No dashboard needed. Ref: [CF Turnstile API](https://developers.cloudflare.com/turnstile/get-started/widget-management/api/) |
| 37 | Task 8 Step 1 presented Dashboard as primary path, CLI as alternative — should be CLI-first | Improvement | Task 8 Step 2: CLI (`wrangler pages secret put`) is now the primary path. Dashboard instructions removed. Added `wrangler pages secret list` verification step |
| 38 | `SITE_URL` and all plan references used `claude-view.dev` instead of production domain `claudeview.ai` | **Blocker** | Fixed `SITE_URL` in `site.ts`, `astro.config.mjs`, `robots.txt`, `README.md`, both plan files, and audit prompt. 7 files, 29 occurrences. Archived plans intentionally untouched (historical record) |
