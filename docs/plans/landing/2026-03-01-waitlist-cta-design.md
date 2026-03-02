# Referral Waitlist CTA — Design

> Resolves L1 blocker #2: "Mobile setup 'Sign up for early access' has no signup mechanism"

## Status: Done (implemented 2026-03-02)

## Goal

Add a viral referral waitlist to the landing site. Visitors enter their email, get a position number and unique referral link. Sharing the link bumps them up the queue. Social proof counter ("Join N developers") shown on all CTA placements.

## Proven Pattern

Referral waitlists amplify existing traffic (they don't create it from zero):

- **Robinhood** — 1M+ pre-launch signups via "Get ahead in line by inviting friends"
- **Linear** — Referral queue for project management tool
- **Monzo** — "Skip the queue" referral system

## Architecture

```
[Astro WaitlistForm component]
  → client-side JS (Turnstile + honeypot validation)
  → POST /api/waitlist
  → [Cloudflare Pages Function]
      → validate Turnstile token (server-side)
      → reject if honeypot filled
      → upsert into Supabase `waitlist` table
      → increment referrer's referral_count if ref code valid
      → return { position, referral_code, total_count }
  → inline success state (no redirect)
      → "You're #N on the waitlist!"
      → copy referral link button
      → "Share on X" button

[Social proof counter]
  → GET /api/waitlist/count
  → cached at edge (60s Cache-Control)
  → "Join N developers on the waitlist"
```

## Supabase Table: `waitlist`

```sql
CREATE TABLE public.waitlist (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email TEXT NOT NULL UNIQUE,
  referral_code TEXT NOT NULL UNIQUE,
  referred_by TEXT REFERENCES waitlist(referral_code),
  referral_count INTEGER NOT NULL DEFAULT 0,
  position INTEGER GENERATED ALWAYS AS IDENTITY,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- RLS: anon can INSERT only, no SELECT/UPDATE/DELETE
ALTER TABLE public.waitlist ENABLE ROW LEVEL SECURITY;

CREATE POLICY "anon_insert_only" ON public.waitlist
  FOR INSERT TO anon
  WITH CHECK (true);
```

Key decisions (from audit):
- `position` uses `GENERATED ALWAYS AS IDENTITY` — Postgres sequence, no race conditions
- `referral_count` denormalized — avoids COUNT(*) on every read (standard for counters at Twitter, Reddit, etc.)
- RLS restricts anon to INSERT only — position/code returned by the CF Function, not by direct DB query

## Cloudflare Pages Functions

### `POST /api/waitlist`

**Input:** `{ email, ref?, turnstile_token, company? }`

**Server-side logic:**
1. Reject if `company` field is non-empty (honeypot)
2. Validate Turnstile token via Cloudflare API
3. Validate email format (regex, no MX check — keep it fast)
4. Check if email exists → return existing position + code (idempotent)
5. Generate 8-char referral code (nanoid, URL-safe alphabet)
6. Insert row into Supabase
7. If `ref` provided and valid, increment referrer's `referral_count` (RPC or raw SQL)
8. Return `{ position, referral_code, total_count }`

**Error responses:**
- 400: Invalid email / missing Turnstile token
- 422: Honeypot triggered (silent — return 200 with fake success to not tip off bots)
- 429: Rate limited (Turnstile handles most abuse, but CF can add WAF rules if needed)
- 500: Supabase error

### `GET /api/waitlist/count`

**Returns:** `{ total_count }`

**Caching:** `Cache-Control: public, max-age=60` — stale by up to 60s, fine for social proof.

## Spam/Bot Protection (Audit Fix)

Two layers, zero user friction:

1. **Cloudflare Turnstile** — Invisible CAPTCHA-free challenge. Free tier. Cloudflare's recommended solution for form protection. Widget renders invisibly, token validated server-side.

2. **Honeypot field** — Hidden input `<input name="company" tabindex="-1" autocomplete="off" style="position:absolute;left:-9999px">`. If filled (only bots do this), silently return fake 200 to avoid tipping them off.

No IP-based rate limiting needed — Turnstile handles bot detection, and returning existing position for duplicate emails makes replay attacks useless.

## Component: `<WaitlistForm />`

Astro component with client-side JS (`<script>` tag, no React).

### Placement (4 locations)

| Location | File | Integration |
|----------|------|-------------|
| Homepage hero | `src/pages/index.astro` | Below InstallCommand + GitHubStars row |
| Mobile features section | `src/pages/index.astro` | Below AppStoreBadges in "Ship features from your phone" |
| Mobile-setup docs | `src/content/docs/docs/guides/mobile-setup.mdx` | Replace the bare status line |
| Pricing cards | `src/components/PricingCards.astro` | Replace disabled "Coming Soon" buttons on Pro/Team tiers |

### States

**Default:**
```
┌─────────────────────────────────────────────┐
│  Join 1,247 developers on the waitlist      │
│  ┌──────────────────────┐ ┌──────────────┐  │
│  │ your@email.com       │ │ Join waitlist │  │
│  └──────────────────────┘ └──────────────┘  │
└─────────────────────────────────────────────┘
```

**Success (inline transform, no redirect):**
```
┌─────────────────────────────────────────────┐
│  You're #347 on the waitlist!               │
│                                             │
│  Share to move up:                          │
│  ┌──────────────────────────────────┐       │
│  │ claudeview.ai?ref=Ab3xK9mQ    │ [Copy] │
│  └──────────────────────────────────┘       │
│                                             │
│  [Share on X]                               │
└─────────────────────────────────────────────┘
```

**Error:** Inline red text below input ("Please enter a valid email").

### Referral Flow

1. Visitor arrives via `claudeview.ai?ref=Ab3xK9mQ`
2. `ref` param captured from URL, stored in hidden field
3. On signup, their referrer's `referral_count` increments
4. Success state shows their own referral link
5. X share text: `"I just joined the claude-view waitlist — Mission Control for AI coding agents. Join me: https://claudeview.ai?ref={code}"`

## Constants (site.ts additions)

```ts
export const WAITLIST_API = '/api/waitlist'
export const TURNSTILE_SITE_KEY = '0x...' // Cloudflare Turnstile site key (public, safe to embed)
```

## What This Design Does NOT Include

- **No email confirmation / double opt-in** — Lowest friction, highest conversion. Trade-off: some garbage emails. Acceptable for growth-first launch. Can add verification later.
- **No admin dashboard** — Query Supabase directly for waitlist stats.
- **No "move up" position recalculation** — `referral_count` shown but position is fixed. Simpler, avoids confusing UX where your number changes.
- **No email service** — No transactional emails. The waitlist is collected for future manual outreach.

## Security Summary

| Concern | Mitigation |
|---------|------------|
| Bot spam | Cloudflare Turnstile (invisible) + honeypot field |
| Data exposure | RLS: anon INSERT only, no reads. Position returned only to submitter |
| Referral code enumeration | nanoid 8-char (281T combinations), no sequential IDs |
| Duplicate signups | Unique constraint on email, idempotent response |
| XSS | Server-side validation, no user input rendered as HTML |
| CSRF | Turnstile token acts as CSRF protection |

## Dependencies to Set Up

1. **Cloudflare Turnstile** — Create site in CF dashboard, get site key + secret key
2. **Supabase table** — Run migration SQL above
3. **Supabase Secret key** — Set as CF Pages environment variable (never in client code)
4. **CF Pages env vars** — `SUPABASE_URL`, `SUPABASE_SECRET_KEY`, `TURNSTILE_SECRET_KEY`
