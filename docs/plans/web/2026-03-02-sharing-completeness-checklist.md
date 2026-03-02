# Conversation Sharing — Completeness Verification Checklist

**Purpose:** Verify every layer of the sharing feature is truly wired up — not just committed, but functional. Run each check manually in a new session.

**How to use:** Open a new Claude Code session, reference this file, and run through each section. Mark items as you go.

---

## A. Code-Level Wiring (read files, confirm integration)

### A1. Rust Server — Serializer exists and is imported

```
- [ ] File exists: `crates/server/src/share_serializer.rs`
- [ ] Exports: `serialize_and_encrypt()`, `key_to_base64url()`
- [ ] Imported in: `crates/server/src/routes/share.rs` (check `use crate::share_serializer`)
- [ ] Module declared in: `crates/server/src/lib.rs` or `main.rs` (check `mod share_serializer`)
- [ ] Cargo deps present: `aes-gcm`, `flate2` in `crates/server/Cargo.toml`
```

### A2. Rust Server — Share routes registered

```
- [ ] File exists: `crates/server/src/routes/share.rs`
- [ ] Exports `pub fn router() -> Router<Arc<AppState>>`
- [ ] Routes: POST /sessions/{session_id}/share, DELETE /sessions/{session_id}/share, GET /shares
- [ ] Imported in `crates/server/src/routes/mod.rs` (`pub mod share;`)
- [ ] Nested in router: `.nest("/api", share::router())` or equivalent in mod.rs
- [ ] `require_auth()` calls `validate_jwt_with_rotation()` (not hardcoded algorithm)
```

### A3. Rust Server — ShareConfig in AppState

```
- [ ] `ShareConfig` struct defined in `crates/server/src/state.rs`
  - [ ] Has `worker_url: String`
  - [ ] Has `viewer_url: String`
  - [ ] Has `http_client: reqwest::Client`
- [ ] `AppState` has field `pub share: Option<ShareConfig>`
- [ ] `main.rs` reads `SHARE_WORKER_URL` and `SHARE_VIEWER_URL` env vars
- [ ] `main.rs` constructs `ShareConfig` only when both env vars are set
- [ ] `main.rs` passes `share` field into `AppState` construction
```

### A4. Cloudflare Worker — Handlers wired

```
- [ ] File exists: `infra/share-worker/src/index.ts`
- [ ] Routes defined in `route()` function:
  - [ ] POST /api/share → handleCreateShare
  - [ ] PUT /api/share/:token/blob → handleUploadBlob
  - [ ] GET /api/share/:token → handleGetShare
  - [ ] DELETE /api/share/:token → handleDeleteShare
  - [ ] DELETE /api/shares/by-session/:sessionId → handleDeleteShareBySession
  - [ ] GET /api/shares → handleListShares
- [ ] Security modules imported: auth.ts, rate-limit.ts, cors.ts, token.ts
  - [ ] Each file exists in `infra/share-worker/src/`
- [ ] Token validation: `TOKEN_PATTERN` regex applied before DB queries
- [ ] Security headers: `SECURITY_HEADERS` constant with X-Frame-Options, Referrer-Policy, X-Content-Type-Options
- [ ] No `void` fire-and-forget calls (should use `.catch(logTrackingError)`)
- [ ] `scheduled()` handler has per-item try-catch in cleanup loop
```

### A5. Cloudflare Worker — wrangler.toml bindings

```
- [ ] File exists: `infra/share-worker/wrangler.toml`
- [ ] Production config:
  - [ ] Worker name: `claude-view-share-worker-prod`
  - [ ] R2 binding: `SHARE_BUCKET` → `claude-view-share-r2-prod`
  - [ ] D1 binding: `DB` → `claude-view-share-d1-prod` (with real database_id UUID)
  - [ ] Custom domain or route for `share.claudeview.ai`
  - [ ] Cron trigger for scheduled cleanup
- [ ] Dev config (`[env.dev]`):
  - [ ] Worker name: `claude-view-share-worker-dev`
  - [ ] R2 binding → `claude-view-share-r2-dev`
  - [ ] D1 binding → `claude-view-share-d1-dev` (with real database_id UUID)
- [ ] D1 migration exists: `infra/share-worker/migrations/001_init.sql`
  - [ ] Creates `shares` table with: token, user_id, session_id, title, size_bytes, status, created_at, view_count
  - [ ] Creates `rate_limits` table
```

### A6. CORS module

```
- [ ] File exists: `infra/share-worker/src/cors.ts`
- [ ] `getCorsHeaders()` does NOT return ALLOWED_ORIGINS[0] as fallback for unknown origins
- [ ] `getPublicCorsHeaders()` returns Access-Control-Allow-Origin: * (for public read endpoint)
- [ ] ALLOWED_ORIGINS includes: share.claudeview.ai, claudeview.ai, share.claudeview.com, claudeview.com
```

### A7. Web Frontend — Share hook

```
- [ ] File exists: `apps/web/src/hooks/use-share.ts`
- [ ] Exports: `useCreateShare()`, `useRevokeShare()`, `useShares()`
- [ ] `createShare()` calls POST /api/sessions/{id}/share
- [ ] `revokeShare()` calls DELETE /api/sessions/{id}/share
- [ ] `fetchShares()` calls GET /api/shares
- [ ] All three attach JWT via `authHeaders()` → `getAccessToken()`
- [ ] 401 response throws `Error('AUTH_REQUIRED')`
```

### A8. Web Frontend — Share button in ConversationView

```
- [ ] `apps/web/src/components/ConversationView.tsx` imports `useCreateShare`
- [ ] `handleShare()` function exists:
  - [ ] Calls `createShare.mutateAsync(sessionId)`
  - [ ] Copies URL to clipboard via `navigator.clipboard.writeText()`
  - [ ] Shows toast on success
  - [ ] Catches AUTH_REQUIRED → calls `openSignIn()` with retry callback
- [ ] Share button rendered in header area with:
  - [ ] Loading spinner (Loader2) when pending
  - [ ] Check icon when shared
  - [ ] Link2 icon default state
```

### A9. Web Frontend — Shared Links in Settings

```
- [ ] Settings page (find via `grep -r "SharedLinks\|useShares" apps/web/src/components/`)
- [ ] SharedLinksSection component:
  - [ ] Calls `useShares()` to list active shares
  - [ ] Shows title, date, view count
  - [ ] Copy link button
  - [ ] Revoke button with confirmation
```

### A10. Web Frontend — Auth wiring

```
- [ ] `apps/web/src/lib/supabase.ts` exists
  - [ ] Exports `supabase` client and `getAccessToken()`
  - [ ] Reads VITE_SUPABASE_URL and VITE_SUPABASE_PUBLISHABLE_KEY
- [ ] `apps/web/src/components/SignInPrompt.tsx` exists
  - [ ] Google OAuth button
  - [ ] Magic link email flow
- [ ] AuthProvider wraps app in `apps/web/src/main.tsx`
- [ ] `useAuth()` hook provides `openSignIn()` used by share flow
```

### A11. Share Viewer SPA — Crypto + Rendering

```
- [ ] `apps/share/src/crypto.ts` exists
  - [ ] Exports `decryptShareBlob(blob, keyBase64url)`
  - [ ] Uses Web Crypto API: AES-GCM decrypt
  - [ ] Decompresses gzip via DecompressionStream
  - [ ] Wire format: [12-byte nonce][ciphertext]
- [ ] `apps/share/src/App.tsx` exists
  - [ ] Parses token from URL path: /s/{token}
  - [ ] Parses key from URL fragment: #k={base64url}
  - [ ] Fetches blob from Worker: GET /api/share/{token}
  - [ ] Calls decryptShareBlob()
  - [ ] Renders SharedConversationView
  - [ ] Fallback URL is NOT *.workers.dev (should be share.claudeview.ai or api-share.claudeview.ai)
- [ ] `apps/share/src/SharedConversationView.tsx` exists
  - [ ] Imports from @claude-view/shared (MessageTyped, etc.)
  - [ ] Runtime type guard on metadata before cast
- [ ] `apps/share/public/_redirects` exists with SPA catch-all
- [ ] `apps/share/vite.config.ts` exists
- [ ] `apps/share/package.json` has correct deps (react, @claude-view/shared, @sentry/react)
```

---

## B. CLI / Build Verification (run these commands)

### B1. Type checks (all must pass with zero errors)

```bash
# Worker
cd infra/share-worker && npx tsc --noEmit

# Share viewer
cd apps/share && npx tsc --noEmit

# Web app
cd apps/web && npx tsc --noEmit

# Rust server
cargo check -p claude-view-server
```

### B2. Builds

```bash
# Share viewer builds
cd apps/share && bun run build
# Should produce dist/index.html + assets

# Web app builds
cd apps/web && bun run build

# Rust compiles
cargo build -p claude-view-server
```

### B3. Worker local dev smoke test

```bash
cd infra/share-worker && bun run dev
# In another terminal:

# Expect 401 (JWT required):
curl -s -X POST http://localhost:8787/api/share \
  -H "Content-Type: application/json" \
  -d '{"session_id":"test"}' | jq .
# Expected: {"error":"Missing Authorization header"}

# Expect 400 (invalid token format):
curl -s http://localhost:8787/api/share/invalid! | jq .
# Expected: {"error":"Invalid token"}

# Expect 404 (valid format, doesn't exist):
curl -s http://localhost:8787/api/share/aaaaaaaaaaaaaaaaaaaaaa | jq .
# Expected: "Not found"

# Check security headers present:
curl -sI http://localhost:8787/api/share/aaaaaaaaaaaaaaaaaaaaaa | grep -E "X-Frame|Referrer|X-Content"
# Expected: X-Frame-Options: DENY, Referrer-Policy: no-referrer, X-Content-Type-Options: nosniff
```

### B4. Cloudflare production verification

```bash
cd infra/share-worker

# List Workers — confirm prod worker exists
npx wrangler deployments list

# Check D1 has tables
npx wrangler d1 execute claude-view-share-d1-prod --command "SELECT name FROM sqlite_master WHERE type='table'"

# Check R2 bucket exists
npx wrangler r2 bucket list | grep share

# Check secrets are set (won't show values, just confirms they exist)
npx wrangler secret list
# Should show: SUPABASE_URL (and optionally SENTRY_DSN, POSTHOG_API_KEY)

# Hit prod endpoint — expect 401:
curl -s -X POST https://share.claudeview.ai/api/share \
  -H "Content-Type: application/json" \
  -d '{"session_id":"test"}' | jq .

# Check security headers on prod:
curl -sI https://share.claudeview.ai/api/share/aaaaaaaaaaaaaaaaaaaaaa | grep -E "X-Frame|Referrer|X-Content"
```

### B5. Rust server env var check

```bash
# Verify the Rust server recognizes share env vars:
grep -n "SHARE_WORKER_URL\|SHARE_VIEWER_URL" crates/server/src/main.rs
# Should find env::var reads for both

# Verify share routes are registered:
grep -n "share::router\|share::" crates/server/src/routes/mod.rs
```

---

## C. End-to-End Flow (manual, requires running app)

```
- [ ] 1. Start Rust server with SHARE_WORKER_URL and SHARE_VIEWER_URL set
- [ ] 2. Open web UI → sign in (Supabase magic link or Google)
- [ ] 3. Navigate to any session → click "Share" button
- [ ] 4. Verify: URL copied to clipboard, format is https://share.claudeview.ai/s/{token}#k={key}
- [ ] 5. Open share link in incognito (no auth session)
- [ ] 6. Verify: viewer loads, decrypts, shows conversation messages
- [ ] 7. Go to Settings → Shared Links → verify share appears in list
- [ ] 8. Click "Revoke" → confirm → verify link returns 404
- [ ] 9. Try sharing again after revoke → should generate new link
```

---

## D. Security Spot Checks

```
- [ ] Share URL fragment (#k=...) is NOT logged anywhere server-side
- [ ] Worker GET /api/share/:token returns raw encrypted bytes (not JSON, not plaintext)
- [ ] CORS: `curl -sI -H "Origin: https://evil.com" https://share.claudeview.ai/api/share` — should NOT have Access-Control-Allow-Origin in response
- [ ] Rate limiting: rapid-fire 61+ GET requests to same token — should get 429
- [ ] Token enumeration: `curl https://share.claudeview.ai/api/share/a` — should get 400 (not 404)
```
