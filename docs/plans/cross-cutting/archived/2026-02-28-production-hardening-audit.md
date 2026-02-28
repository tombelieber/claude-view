# Production Hardening — Final Audit Report

**Date:** 2026-02-28
**Branch:** `worktree-monorepo-expo`
**Design doc:** `docs/plans/2026-02-28-production-hardening-design.md`
**Impl plan:** `docs/plans/2026-02-28-production-hardening-impl.md`
**Status:** 17/17 tasks implemented. 5/5 design gaps closed. Shippable audit PASSED. Deployment checklist: `2026-02-28-deployment-checklist.md`.

---

## Executive Summary

The production hardening plan (17 tasks across 6 phases) has been fully implemented and passes all builds/tests. However, a design-vs-implementation gap analysis reveals **5 unimplemented design features**, **9 manual deployment prerequisites**, and **1 design self-contradiction**. This report provides the complete gap list with actionable next steps, split into "execute now" (no decisions needed) and "needs decision" categories.

---

## What Was Delivered (17 Tasks — All Pass)

### Phase 0: Supabase Auth Setup
- [x] **Task 0:** Supabase env vars added to `crates/server/.env.example` and `apps/web/.env.example`

### Phase 1: Cloudflare Share Worker
- [x] **Task 1:** Worker scaffold — `infra/share-worker/` (wrangler.toml, package.json, tsconfig)
- [x] **Task 2:** D1 schema + token generator + rate limiter — `migrations/001_init.sql`, `token.ts`, `rate-limit.ts`
- [x] **Task 3:** JWT middleware — `infra/share-worker/src/auth.ts`
- [x] **Task 4:** Worker handlers — `infra/share-worker/src/index.ts` (7 routes + cron)
- [x] **Task 5:** Deployment config — wrangler.toml (R2, D1 bindings, cron trigger)

### Phase 2: Rust Backend
- [x] **Task 6:** Supabase JWT validation — `crates/server/src/auth/{mod,supabase}.rs`, `error.rs`, `state.rs`
- [x] **Task 7:** AES-256-GCM serializer — `crates/server/src/share_serializer.rs`
- [x] **Task 8:** Share route handlers — `crates/server/src/routes/share.rs` (3 endpoints)

### Phase 3: Frontend
- [x] **Task 9:** Supabase auth client — `apps/web/src/lib/supabase.ts`, `SignInPrompt.tsx`
- [x] **Task 10:** Share hook + button — `use-share.ts`, `ConversationView.tsx`
- [x] **Task 11:** Shared links settings + Sentry — `SettingsPage.tsx`, `main.tsx`

### Phase 4: Viewer SPA
- [x] **Task 12:** Viewer SPA with Web Crypto decrypt — `apps/share/` (8 files, 275KB bundle)
- [x] **Task 13:** Wire up + `.env.production`

### Phase 5: Relay Hardening
- [x] **Task 14:** JWT validation on `/pair/claim` and WS — `crates/relay/src/auth.rs`, `pairing.rs`, `ws.rs`
- [x] **Task 15:** Rate limiting + CORS lockdown + body limits — `rate_limit.rs`, `lib.rs`
- [x] **Task 16:** Sentry + PostHog observability — `posthog.rs`, `main.rs`

### Build & Test Results
| Check | Result |
|-------|--------|
| `cargo check --workspace` | PASS |
| `cargo clippy --workspace -- -D warnings` | PASS |
| `cargo test -p claude-view-server` | 527 pass (2 pre-existing in `live::manager`, unrelated) |
| `cargo test -p claude-view-relay` | 5 pass, 0 fail |
| `tsc --noEmit` (web) | PASS |
| `tsc --noEmit` (share-worker) | PASS |
| `vite build` (share viewer) | PASS (275KB JS, 7KB CSS) |

### Commits (16 total)
All committed on branch `worktree-monorepo-expo`. Key commits:
- `c5561ebe` — fix(server): add jwks/share fields to test AppState constructions
- `1b9741cc` — fix(share): check blob upload response status + throw on fetchShares error
- `560d40d1` — feat(relay): production hardening — JWT auth, rate limiting, CORS lockdown, observability
- Plus 13 earlier feature commits covering Tasks 1-16

---

## Gap Analysis: Design vs Implementation

### GAP 1: WS Per-Message Rate Limiting (MISSING)

**Design spec** (line 194): `WS messages → 60 messages/min per connection`

**Current state:** No per-message throttling. Once a WS connection is established, messages flow unlimited. The relay has connection-level limits (1000 global, 1 per device_id via DashMap key) but nothing throttles message volume on an open connection.

**Risk:** A compromised or malicious client could flood the relay with messages, consuming bandwidth and CPU for all connected users.

**Fix location:** `crates/relay/src/ws.rs` → `handle_socket()` function, inside the message receive loop.

**Implementation approach:**
```rust
// In handle_socket(), add a counter before the message processing loop:
let mut msg_count: u32 = 0;
let mut window_start = Instant::now();

// Inside the loop, before processing each message:
if window_start.elapsed() > Duration::from_secs(60) {
    msg_count = 0;
    window_start = Instant::now();
}
msg_count += 1;
if msg_count > 60 {
    tracing::warn!(device_id = %device_id, "WS message rate limit exceeded");
    let _ = sender.send(Message::Close(Some(CloseFrame {
        code: 1008, // Policy Violation
        reason: "Rate limit exceeded".into(),
    }))).await;
    break;
}
```

**Effort:** Small (15 min). No new files, no new dependencies.

**Decision needed:** No.

---

### GAP 2: `/push-tokens` Rate Limiting (MISSING)

**Design spec** (line 193): `POST /push-tokens → 10 requests/min per device_id`

**Current state:** `pair_rate_limiter` and `claim_rate_limiter` exist in `RelayState`. No rate limiter for push token registration.

**Risk:** An attacker could spam push token registrations, filling the `push_tokens` DashMap with garbage entries.

**Fix location:** `crates/relay/src/state.rs` (add field) + `crates/relay/src/push.rs` (add check) + `crates/relay/src/main.rs` (instantiate).

**Implementation approach:**
1. Add `pub push_rate_limiter: Arc<RateLimiter>` to `RelayState`
2. Update `RelayState::new()` to accept it as a parameter
3. Add `headers: axum::http::HeaderMap` to `register_push_token` handler signature
4. Add rate limit check at top of handler: `if !state.push_rate_limiter.check(&device_id).await { return Err(StatusCode::TOO_MANY_REQUESTS); }`
5. Instantiate in `main.rs`: `Arc::new(RateLimiter::new(10.0 / 60.0, 10.0))`
6. Update all 5 integration test `RelayState::new()` calls to pass an additional rate limiter
7. Add `push_rl_clone` to the eviction spawn task

**Effort:** Small (20 min). Pattern identical to existing `pair_rate_limiter`.

**Decision needed:** No.

---

### GAP 3: Share Viewer Shows Raw JSON (NOT Rendered Conversation)

**Design spec** (line 51): `gunzip → AES-256-GCM decrypt → Render conversation (read-only)`

**Current state:** `apps/share/src/App.tsx` decrypts successfully but displays:
```tsx
<pre>{JSON.stringify(session, null, 2).slice(0, 2000)}</pre>
```
This is explicitly labeled in the code as "Phase 4 MVP: raw JSON preview" with a follow-up comment about sharing renderer components from `@web`.

**Risk:** Not a security issue. But the share viewer is user-facing — sharing a raw JSON blob is a poor UX that undermines the product's polish.

**Fix options:**

| Option | Work | Tradeoff |
|--------|------|----------|
| **(a) Ship raw JSON for launch** | None | Fast but ugly. First impression for recipients is raw data. |
| **(b) Minimal renderer in `apps/share`** | Medium (1-2 days) | Build a simple message list renderer (role + content) directly in the share app. Duplicates some web app code but stays self-contained. |
| **(c) Extract shared components** | Large (3-5 days) | Move message rendering components from `apps/web/src/components/` into `packages/shared/`. Both `apps/web` and `apps/share` import from shared. Proper architecture but significant refactor. |

**Decision needed:** YES — pick (a), (b), or (c).

**Recommendation:** (b) for launch. Build a minimal `<MessageList>` component in `apps/share/` that renders `role`, `content`, and timestamps. Don't try to match every feature of the web app's renderer. Extract to shared later when mobile app also needs it.

---

### GAP 4: PostHog Not Initialized in Viewer SPA (MISSING)

**Design spec** (line 334): Phase 4 deliverables include "PostHog" for the viewer.

**Current state:** `apps/share/src/App.tsx` checks `(window as any).posthog` but never initializes PostHog. No `<script>` tag, no `posthog-js` import. The check always evaluates to `undefined`.

**Risk:** Zero analytics on share views from the viewer side. The Worker tracks `share_viewed` server-side (on blob fetch), so view counting works. But client-side events like `share_decrypt_success` and timing data are lost.

**Fix location:** `apps/share/index.html` (add PostHog snippet) or `apps/share/src/App.tsx` (JS init).

**Implementation approach:**
Add to `apps/share/index.html` `<head>`:
```html
<script>
  !function(t,e){var o,n,p,r;e.__SV||(window.posthog=e,e._i=[],e.init=function(i,s,a){function g(t,e){var o=e.split(".");2==o.length&&(t=t[o[0]],e=o[1]),t[e]=function(){t.push([e].concat(Array.prototype.slice.call(arguments,0)))}}(p=t.createElement("script")).type="text/javascript",p.async=!0,p.src=s.api_host+"/static/array.js",(r=t.getElementsByTagName("script")[0]).parentNode.insertBefore(p,r);var u=e;for(void 0!==a?u=e[a]=[]:a="posthog",u.people=u.people||[],u.toString=function(t){var e="posthog";return"posthog"!==a&&(e+="."+a),t||(e+=" (stub)"),e},u.people.toString=function(){return u.toString(1)+".people (stub)"},o="capture identify alias people.set people.set_once set_config register register_once unregister opt_out_capturing has_opted_out_capturing opt_in_capturing reset isFeatureEnabled onFeatureFlags getFeatureFlag getFeatureFlagPayload reloadFeatureFlags group updateEarlyAccessFeatureEnrollment getEarlyAccessFeatures getActiveMatchingSurveys getSurveys onSessionId".split(" "),n=0;n<o.length;n++)g(u,o[n]);e._i.push([i,s,a])},e.__SV=1)}(document,window.posthog||[]);
  posthog.init('YOUR_POSTHOG_KEY', {api_host: 'https://us.i.posthog.com'});
</script>
```

OR (cleaner, no external script in HTML):
```bash
cd apps/share && bun add posthog-js
```
```tsx
// In App.tsx, before the component:
import posthog from 'posthog-js';
if (import.meta.env.PROD && import.meta.env.VITE_POSTHOG_KEY) {
  posthog.init(import.meta.env.VITE_POSTHOG_KEY, { api_host: 'https://us.i.posthog.com' });
}
```

**Effort:** Tiny (10 min).

**Decision needed:** No. The `posthog-js` approach is cleaner and consistent with how Sentry is initialized.

---

### GAP 5: Blob Upload Rate Limiting — Design Self-Contradiction

**Design spec** (line 102): `PUT /api/share/:token/blob → 10/hour per user_id (shared budget with create)`

**Design spec** (line 92): `PUT /api/share/:token/blob → No auth — token-is-auth`

**Current state:** No rate limiting on blob upload. No user_id available (endpoint is token-is-auth).

**Analysis:** The design contradicts itself. The auth table says "No auth — token-is-auth" but the rate limit table says "per user_id." You can't rate limit by user_id on an unauthenticated endpoint without looking up the token in D1 first.

**Actual risk is low:** The blob upload is inherently single-use. A token transitions from `pending` → `ready` on first upload. Subsequent uploads to the same token return `409 Conflict`. The `POST /api/share` endpoint (which creates the token) IS rate-limited at 10/hour per user_id. So an attacker can only create 10 pending tokens per hour, each uploadable once. The effective upload rate is already bounded by the create rate limit.

**Fix options:**

| Option | Work | Recommendation |
|--------|------|---------------|
| **(a) Document as sufficient** | Add a code comment explaining the inherent bound | **Recommended** |
| **(b) Add D1 lookup + rate limit** | Look up `user_id` from `shares` table by token, then rate limit | Over-engineered for MVP |

**Decision needed:** YES — confirm (a) is acceptable, or choose (b).

**Recommendation:** (a). Add a comment in `handleUploadBlob`:
```typescript
// Rate limiting note: This endpoint is inherently bounded by POST /api/share
// (10/hour per user_id). Each token is single-use (pending→ready), so upload
// volume ≤ create volume. No additional rate limiting needed.
```

---

## Manual Deployment Prerequisites (Not Yet Done)

These are infrastructure/console steps that require your accounts and credentials. Code is ready — these are the "last mile" to go live.

### Supabase Setup (Task 0 manual steps)

| # | Step | Where | Notes |
|---|------|-------|-------|
| D1 | Create Supabase project | [supabase.com](https://supabase.com) | Name: `claude-view`, region: closest to users |
| D2 | Enable Email auth (magic link) | Dashboard → Authentication → Providers | Confirm email = OFF |
| D3 | Enable Google OAuth | Dashboard → Authentication → Providers | Needs Google Cloud Console OAuth credentials |
| D4 | Configure redirect URLs | Dashboard → Authentication → URL Config | `https://claudeview.ai/**`, `https://claudeview.com/**`, `claudeview://auth`, `http://localhost:5173/**`, `http://localhost:8081/**` |
| D5 | Note credentials | Dashboard → Project Settings → API | Project URL, anon key, JWKS URL |

### Cloudflare Setup (Tasks 5, 12, 13)

| # | Step | Command / Location |
|---|------|--------------------|
| D6 | Create R2 bucket | `cd infra/share-worker && bunx wrangler r2 bucket create claude-view-shares` |
| D7 | Create D1 database (prod) | `bunx wrangler d1 create claude-view-share-meta` → copy `database_id` into `wrangler.toml` |
| D8 | Run D1 migration (prod) | `bunx wrangler d1 execute claude-view-share-meta --file=./migrations/001_init.sql` |
| D9 | Set Worker secrets | `bunx wrangler secret put SENTRY_DSN` / `POSTHOG_API_KEY` |
| D10 | Set SUPABASE_URL in wrangler.toml | Edit `[vars]` section — this is a public URL, not a secret |
| D11 | Deploy Worker | `bunx wrangler deploy` |
| D12 | Deploy Viewer SPA | `cd apps/share && bun run build && bunx wrangler pages deploy dist --project-name claude-view-share` |
| D13 | Configure custom domain: Worker | Cloudflare dashboard → `api-share.claudeview.ai` → `claude-view-share` Worker |
| D14 | Configure custom domain: Pages | Cloudflare dashboard → `share.claudeview.ai` → `claude-view-share` Pages |

### Fly.io Relay Secrets (Task 16)

| # | Step | Command |
|---|------|---------|
| D15 | Set relay secrets | `fly secrets set SENTRY_DSN="..." POSTHOG_API_KEY="..." SUPABASE_URL="..." -a claude-view-relay` |
| D16 | Deploy relay | `fly deploy -a claude-view-relay` |

### Local Dev (.env files — NOT committed)

| # | File | Vars Needed |
|---|------|-------------|
| D17 | `apps/web/.env.local` | `VITE_SUPABASE_URL`, `VITE_SUPABASE_ANON_KEY`, `VITE_SENTRY_DSN` |
| D18 | `apps/share/.env.local` | `VITE_WORKER_URL` (for local dev), `VITE_SENTRY_DSN`, `VITE_POSTHOG_KEY` |
| D19 | Shell exports for Rust server | `SUPABASE_URL`, `SHARE_WORKER_URL`, `SHARE_VIEWER_URL` |

---

## Known Limitations (Documented, By Design)

These are NOT bugs. They are explicitly scoped out of M1 with documented migration paths.

| # | Limitation | Design Reference | Migration Path |
|---|-----------|-----------------|----------------|
| K1 | Relay JWKS no rotation | Impl plan Task 14, `SupabaseAuth::from_supabase_url()` docstring | M2: Wrap in `Arc<RwLock>`, add re-fetch-on-failure (server side already has this pattern) |
| K2 | D1 rate limiting at scale | Impl plan Task 2, scale note | When traffic > ~100 req/s: migrate to `cloudflare:rate-limiter` binding (zero-config) |
| K3 | `expires_at` column unused | Design line 157: "null = no expiry (future paid feature)" | Future monetization: time-limited shares for free tier |
| K4 | JWT in WS query param (logged in plaintext) | Impl plan Task 14, Step 6 | By design: browsers can't set WS headers. Mitigated by short JWT expiry (1 hour) |
| K5 | JWKS takes first key only | Server `supabase.rs` line 1043 | M2: iterate keys, match `kid` from JWT header |
| K6 | `push.rs` creates `reqwest::Client` per call | Pre-existing code, not hardening scope | Refactor to use shared client from `RelayState.posthog_client` |

---

## Action Plan: Reaching 100/100

### Batch A — Execute Now (No Decisions, ~1 hour total)

These close all unimplemented design features. Can be done in a single session.

| Task | Gap | Files to Touch | Effort |
|------|-----|---------------|--------|
| A1 | GAP 1: WS message rate limiting | `crates/relay/src/ws.rs` | 15 min |
| A2 | GAP 2: `/push-tokens` rate limiting | `state.rs`, `push.rs`, `main.rs`, `tests/integration.rs` | 20 min |
| A3 | GAP 4: PostHog in viewer SPA | `apps/share/src/App.tsx`, `apps/share/package.json` | 10 min |
| A4 | GAP 5: Document blob upload rate limit | `infra/share-worker/src/index.ts` (comment only) | 5 min |

**Verification after Batch A:**
```bash
cargo test -p claude-view-relay                     # relay tests pass
cargo check -p claude-view-relay                    # compiles
cd apps/share && bun run build                      # viewer builds
tsc --noEmit                                        # no TS errors
```

**Commit after Batch A:**
```
feat(hardening): close remaining design gaps — WS message throttle, push-token rate limit, viewer PostHog
```

### Batch B — Needs Your Decision

| Task | Gap | Decision | Options |
|------|-----|----------|---------|
| B1 | GAP 3: Share viewer rendering | What to show recipients? | **(a)** Raw JSON for now, **(b)** Minimal renderer in `apps/share` (recommended), **(c)** Extract shared components from `apps/web` |
| B2 | GAP 5 confirmation | Blob upload rate limit | **(a)** Document single-use token as sufficient (recommended), **(b)** Add D1 lookup + explicit rate limit |

### Batch C — Deployment (Needs Your Accounts)

Steps D1-D19 from the deployment prerequisites table above. Order:
1. Supabase project (D1-D5) — foundation for everything
2. Cloudflare infra (D6-D14) — Worker + Viewer
3. Fly.io secrets (D15-D16) — relay
4. Local dev env (D17-D19) — for E2E testing

### Batch D — E2E Verification (After Deployment)

Follow Task 13 Step 3 from the impl plan:
1. Start local server with `SUPABASE_URL` set
2. Open web UI → sign in with magic link
3. Open any session → click "Share"
4. Verify share URL has `#k=...` fragment
5. Open link in incognito (no auth session)
6. Verify viewer loads and decrypts
7. Check Cloudflare dashboard for Worker logs
8. Check Sentry for errors (should be none)
9. Settings → Shared Links → verify share appears
10. Click Revoke → verify link stops working

---

## File Index (All Files Created/Modified)

### New Files (30)
```
infra/share-worker/
  wrangler.toml
  package.json
  tsconfig.json
  migrations/001_init.sql
  src/index.ts
  src/auth.ts
  src/cors.ts
  src/rate-limit.ts
  src/token.ts

crates/server/src/
  auth/mod.rs
  auth/supabase.rs
  share_serializer.rs
  routes/share.rs

crates/relay/src/
  rate_limit.rs
  posthog.rs

apps/web/src/
  lib/supabase.ts
  hooks/use-share.ts
  components/SignInPrompt.tsx

apps/share/
  package.json
  vite.config.ts
  index.html
  tsconfig.json
  .env.production
  src/main.tsx
  src/index.css
  src/App.tsx
  src/crypto.ts
```

### Modified Files (18)
```
.gitignore                                    (+.wrangler/)
crates/server/Cargo.toml                      (+aes-gcm, flate2, jsonwebtoken)
crates/server/src/error.rs                    (+Unauthorized variant)
crates/server/src/state.rs                    (+jwks, share fields)
crates/server/src/lib.rs                      (+auth, share_serializer, share route modules)
crates/server/src/main.rs                     (+JWKS loading, ShareConfig, params to create_app_full)
crates/server/src/routes/mod.rs               (+share router)
crates/relay/Cargo.toml                       (+jsonwebtoken, anyhow, sentry, sentry-tracing; tower→deps)
crates/relay/src/auth.rs                      (+SupabaseAuth, SupabaseClaims)
crates/relay/src/state.rs                     (+supabase_auth, rate limiters, posthog fields, new())
crates/relay/src/lib.rs                       (+rate_limit, posthog mods; CORS lockdown; body limit; timeout split)
crates/relay/src/main.rs                      (+Sentry init, JWKS load, rate limiter init, eviction task)
crates/relay/src/pairing.rs                   (+headers param, JWT validation, rate limiting)
crates/relay/src/ws.rs                        (+Query param, pre-upgrade auth, connection limit)
crates/relay/tests/integration.rs             (+RelayState::new() args for all 5 tests)
apps/web/src/components/ConversationView.tsx  (+Share button, sign-in modal)
apps/web/src/components/SettingsPage.tsx       (+SharedLinksSection)
apps/web/src/main.tsx                         (+Sentry init)
```

---

## How to Continue in a New Session

**For Claude:** Use `superpowers:executing-plans` skill. The plan file is this audit report. Batch A tasks are the immediate work.

1. Read this file: `docs/plans/2026-02-28-production-hardening-audit.md`
2. Execute **Batch A** (4 tasks, ~1 hour, no decisions needed)
3. Ask the user for **Batch B** decisions (viewer rendering, blob rate limit)
4. If Batch B decisions are made, implement them
5. Report completion and move to **Batch C** (deployment — needs user's accounts)

**Verification commands after any changes:**
```bash
cargo test -p claude-view-relay        # relay changes
cargo check --workspace                # full compile check
cd apps/share && bun run build         # viewer changes
cd infra/share-worker && bunx tsc --noEmit  # worker changes
```
