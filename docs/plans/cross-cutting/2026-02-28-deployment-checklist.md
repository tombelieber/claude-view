# Production Deployment Checklist

**Date:** 2026-02-28
**Prerequisite:** All code is shipped and passing (see `2026-02-28-production-hardening-audit.md`).
**Who:** You (human). These are cloud console + CLI steps requiring your accounts.

---

## Phase 1: Supabase Project (Foundation)

Everything depends on this. Do it first.

- [x] **D1. Create Supabase project** ✅ (2026-03-04)
  - Project: `claude-view` at `iebjyftoadahqptmfcio.supabase.co`

- [x] **D2. Enable Email auth (magic link)** ✅ (2026-03-04)
  - Passwordless magic link enabled, confirm email OFF

- [x] **D3. Enable Google OAuth** ✅ (2026-03-04)
  - GCP OAuth 2.0 Client ID created (project `claude-view`)
  - Authorized JS origin: `https://iebjyftoadahqptmfcio.supabase.co`
  - Authorized redirect URI: `https://iebjyftoadahqptmfcio.supabase.co/auth/v1/callback`
  - Client ID + Secret configured in Supabase Google provider
  - **Tested and working** (Google sign-in verified)

- [x] **D4. Configure redirect URLs** ✅ (2026-03-04)
  - Site URL: `https://claudeview.ai`
  - 7 redirect URLs configured:
    ```
    https://claudeview.ai/**
    https://claudeview.com/**
    claudeview://auth
    claude-view://auth
    http://localhost:5173/**
    http://localhost:8081/**
    http://localhost:47892/**
    ```

- [x] **D5. Note your credentials** ✅ (2026-03-04)
  - Publishable key committed in `apps/web/.env.production` (safe — public key)
  - No secret keys used (JWT validation via JWKS fetch)

---

## Phase 2: Cloudflare — Share Worker Infrastructure

Run all commands from `infra/share-worker/`.

```bash
cd infra/share-worker
```

- [ ] **D6. Create R2 bucket**
  ```bash
  bunx wrangler r2 bucket create claude-view-shares
  ```

- [ ] **D7. Create D1 database**
  ```bash
  bunx wrangler d1 create claude-view-share-meta
  ```
  Copy the `database_id` from the output and paste it into `wrangler.toml` line 18:
  ```toml
  database_id = "<paste-here>"
  ```

- [ ] **D8. Run D1 migration**
  ```bash
  bunx wrangler d1 execute claude-view-share-meta --file=./migrations/001_init.sql
  ```
  Verify: `bunx wrangler d1 execute claude-view-share-meta --command="SELECT name FROM sqlite_master WHERE type='table'"`
  Expected: `shares`, `rate_limits`

- [ ] **D9. Set SUPABASE_URL in wrangler.toml**
  Edit `wrangler.toml` line 8 — paste your Supabase project URL:
  ```toml
  SUPABASE_URL = "https://<ref>.supabase.co"
  ```
  (This is a public URL, safe in `[vars]`)

- [ ] **D10. Set Worker secrets**
  ```bash
  bunx wrangler secret put SENTRY_DSN
  # Paste your Sentry DSN when prompted

  bunx wrangler secret put POSTHOG_API_KEY
  # Paste your PostHog project API key when prompted
  ```
  If you don't have Sentry/PostHog yet, you can skip these — the code handles missing values gracefully (observability just won't be active).

- [ ] **D11. Deploy Worker**
  ```bash
  bunx wrangler deploy
  ```
  Note the Worker URL from output (e.g. `https://claude-view-share.<your-subdomain>.workers.dev`)

---

## Phase 3: Cloudflare — Share Viewer SPA

- [ ] **D12. Deploy Viewer SPA to Cloudflare Pages**
  ```bash
  cd apps/share
  bun run build
  bunx wrangler pages deploy dist --project-name claude-view-share
  ```
  First run will prompt you to create the Pages project — say yes.

- [ ] **D13. Configure custom domain: Worker API**
  - Cloudflare dashboard → **Workers & Pages** → `claude-view-share` Worker
  - **Settings** → **Triggers** → **Custom Domains**
  - Add: `api-share.claudeview.ai`
  - Cloudflare will auto-create the DNS record

- [ ] **D14. Configure custom domain: Viewer Pages**
  - Cloudflare dashboard → **Workers & Pages** → `claude-view-share` Pages project
  - **Custom domains** → Add: `share.claudeview.ai`
  - Cloudflare will auto-create the DNS record

---

## Phase 4: Fly.io — Relay Secrets + Deploy

- [x] **D15. Set relay secrets** ✅ (2026-03-04)
  - `SUPABASE_URL` set via `fly secrets set` — relay validates JWTs via JWKS
  - Sentry/PostHog deferred (optional — relay degrades gracefully)

- [x] **D16. Deploy relay** ✅ (previously deployed, 2 machines running)
  - Verified: `claude-view-relay.fly.dev` healthy, rolling deploy on secret update

---

## Phase 5: Local Dev Environment

These files are gitignored. They only exist on your machine for local development.

- [x] **D17. Web frontend** ✅ (2026-03-04)
  - `apps/web/.env.local` created (gitignored, local dev)
  - `apps/web/.env.production` created (committed, CI builds use this)

- [ ] **D18. Share viewer** — create `apps/share/.env.local`
  ```bash
  cat > apps/share/.env.local << 'EOF'
  VITE_WORKER_URL=https://api-share.claudeview.ai
  VITE_SENTRY_DSN=
  VITE_POSTHOG_KEY=
  EOF
  ```

- [ ] **D19. Rust server** — add to your shell profile or run before starting the server
  ```bash
  export SUPABASE_URL=https://<ref>.supabase.co
  export SHARE_WORKER_URL=https://api-share.claudeview.ai
  export SHARE_VIEWER_URL=https://share.claudeview.ai
  ```

---

## Phase 6: E2E Verification

Run through the full flow once to confirm everything works end-to-end.

- [ ] **V1.** Start local server with `SUPABASE_URL` set
  ```bash
  SUPABASE_URL=https://<ref>.supabase.co cargo run -p claude-view-server
  ```

- [ ] **V2.** Open web UI → sign in with magic link or Google

- [ ] **V3.** Open any session → click **Share**

- [ ] **V4.** Verify share URL has `#k=...` fragment (encryption key)

- [ ] **V5.** Open link in **incognito** window (no auth session)

- [ ] **V6.** Verify viewer loads, decrypts, and renders the conversation (not raw JSON)

- [ ] **V7.** Check Cloudflare dashboard → Worker → **Logs** for the share request

- [ ] **V8.** Check Sentry for errors (should be none)

- [ ] **V9.** Settings → **Shared Links** → verify the share appears in the list

- [ ] **V10.** Click **Revoke** → verify the share link stops working (404 in incognito)

- [ ] **V11.** Test relay: open the mobile app → pair with Mac → verify WS connection succeeds with JWT

---

## Quick Reference: Where Each Credential Goes

| Credential | Where it goes | How |
|-----------|--------------|-----|
| `SUPABASE_URL` | wrangler.toml `[vars]` | Edit file (D9) |
| `SUPABASE_URL` | Fly.io relay | `fly secrets set` (D15) |
| `SUPABASE_URL` | Local Rust server | Shell export (D19) |
| `SUPABASE_URL` | Web frontend | `apps/web/.env.local` (D17) |
| `SUPABASE_PUBLISHABLE_KEY` | Web frontend | `apps/web/.env.local` (D17) |
| `SENTRY_DSN` | Cloudflare Worker | `wrangler secret put` (D10) |
| `SENTRY_DSN` | Fly.io relay | `fly secrets set` (D15) |
| `POSTHOG_API_KEY` | Cloudflare Worker | `wrangler secret put` (D10) |
| `POSTHOG_API_KEY` | Fly.io relay | `fly secrets set` (D15) |
| `POSTHOG_KEY` | Share viewer | `apps/share/.env.local` (D18) |

---

*Estimated time: 30-45 minutes if you have all accounts ready. The longest wait is Supabase project provisioning (~2 min) and DNS propagation for custom domains (~5 min).*
