# Conversation Sharing — Implementation Plan

> **Status:** DONE (2026-03-01) — all 9 tasks implemented, shippable audit passed (SHIP IT)
>
> **Post-ship fix (2026-03-02):** Viewer SPA was built but never deployed. Fixed: deployed to Cloudflare Pages (dev), fixed CORS bug, fixed Tailwind content scanning. See [Post-Ship Gaps](#post-ship-gaps-fixed-2026-03-02) below.

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let users share Claude conversations via an encrypted link (AES-256-GCM, key in URL fragment). Hosted on Cloudflare (Worker + R2 + D1).

### Completion Summary

All 9 tasks delivered across 4 phases. 18 commits. Shippable audit: SHIP IT (0 blockers, 2 minor warnings).

| Task | Commits | Description |
|------|---------|-------------|
| 1 | `6237a256`, `9d6f63a1`, `67e648b4`, `6d45feb7`, `1dff3337` | Scaffold + D1 schema + auth + hardened Worker handlers |
| 2 | `67179648` | Wire custom domains + production env config |
| 3 | `097f4574` | AES-256-GCM session serializer (gzip then encrypt) |
| 4 | `c3fb3623`, `bf1f712e`, `c5561ebe` | Supabase JWT + share routes + test fixes |
| 5 | `6254e70a` | Supabase auth client + SignInPrompt component |
| 6 | `01de1d17`, `1b9741cc` | Share button with JWT auth + error handling fixes |
| 7 | `60ada0af` | Shared links settings section + Sentry init |
| 8 | `1f21b539`, `bd9e10d0`, `fd2757b3`, `add0a5be` | Viewer SPA + shared component extraction |
| 9 | `fe7d447e` | Secure blob upload auth + analytics improvements |

**Architecture:** Local Rust server serializes JSONL → JSON → gzip → AES-256-GCM encrypt (random key), uploads encrypted blob to R2 via Worker. Viewer SPA decrypts in-browser using Web Crypto API. Key travels only in URL fragment (never hits server).

**Tech Stack:** Cloudflare Workers (TypeScript), R2, D1, Pages. Rust (Axum) for local API. React + Vite for viewer SPA. Web Crypto API for browser-side decryption.

**Design Doc:** `docs/plans/2026-02-28-conversation-sharing-design.md`
**Security Design:** `docs/plans/2026-02-28-production-hardening-design.md`

---

## Prerequisites — Execute Production Hardening First

**IMPORTANT:** Complete these phases from `2026-02-28-production-hardening-impl.md` before starting this plan:

- **Phase 0:** Supabase Auth Setup (Task 0)
- **Phase 1:** Worker Security Modules (Tasks 1–3) + CORS helper
- **Phase 5:** Relay Hardening (Tasks 14–16)
- **Rust JWT infra:** Task 6 — `auth/supabase.rs` + `AppState.jwks` (NOT ShareConfig — that's this plan)

### What already exists after hardening

| Component | File | What it provides |
|-----------|------|-----------------|
| Supabase project | Cloud console | Magic link + Google OAuth, JWKS endpoint |
| Worker scaffold | `infra/share-worker/` | wrangler.toml, package.json, tsconfig.json, deps installed |
| D1 schema | `migrations/001_init.sql` | `shares` table (user_id, status, etc.) + `rate_limits` table |
| Token generator | `src/token.ts` | 22-char base62 (131 bits entropy) |
| Rate limiter | `src/rate-limit.ts` | D1 sliding-window counter |
| JWT auth | `src/auth.ts` | `requireAuth()` — validates Supabase RS256 JWT |
| CORS helper | `src/cors.ts` | `getCorsHeaders()` + `getPublicCorsHeaders()` |
| D1 database | Local dev created | `wrangler d1 execute --local` already run |
| Rust JWT | `crates/server/src/auth/supabase.rs` | `JwksCache`, `fetch_decoding_key()`, `validate_jwt()` |
| AppState.jwks | `crates/server/src/state.rs` | JWKS loaded at startup |
| Rust deps | `Cargo.toml` | `jsonwebtoken`, `reqwest` already added |
| Relay | `crates/relay/` | Fully hardened with JWT, rate limiting, CORS, Sentry, PostHog |

---

## Phase 1: Worker Handler Implementations

### Task 1: Implement Worker Handlers (Using Existing Security Modules)

**Files:**
- Modify: `infra/share-worker/src/index.ts` (replace stub with full implementation)

The security modules (`auth.ts`, `rate-limit.ts`, `cors.ts`, `token.ts`) already exist. This task wires them into the actual share CRUD handlers.

**Step 1: Replace `src/index.ts` with full implementation**

```typescript
// src/index.ts
import { withSentry } from "@sentry/cloudflare";
import { requireAuth, AuthError } from "./auth";
import { checkRateLimit, cleanupExpiredWindows } from "./rate-limit";
import { generateToken } from "./token";
import { getCorsHeaders, getPublicCorsHeaders } from "./cors";

export interface Env {
  SHARE_BUCKET: R2Bucket;
  DB: D1Database;
  ENVIRONMENT: string;
  SUPABASE_URL: string;
  POSTHOG_API_KEY: string;
}

const MAX_BLOB_BYTES = 50 * 1024 * 1024; // 50MB
const MAX_JSON_BODY_BYTES = 1024;         // 1KB for JSON endpoints

// Rate limit config per endpoint (limit, windowSecs)
const RATE_LIMITS = {
  create:  { limit: 10, windowSecs: 3600 },  // 10/hour per user_id
  read:    { limit: 60, windowSecs: 60 },    // 60/min per IP
  delete:  { limit: 20, windowSecs: 3600 },  // 20/hour per user_id
  list:    { limit: 30, windowSecs: 60 },    // 30/min per user_id
} as const;

export default withSentry(
  (env: Env) => ({
    dsn: env["SENTRY_DSN"] as string | undefined,
    environment: env.ENVIRONMENT,
    tracesSampleRate: 0.1,
  }),
  {
    async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
      const url = new URL(request.url);
      const corsHeaders = getCorsHeaders(request, env);

      if (request.method === "OPTIONS") {
        return new Response(null, { status: 204, headers: corsHeaders });
      }

      try {
        const response = await route(url, request, env);
        // Apply CORS to all responses
        for (const [k, v] of Object.entries(corsHeaders)) {
          response.headers.set(k, v);
        }
        return response;
      } catch (err) {
        if (err instanceof AuthError) {
          return jsonResponse({ error: err.message }, err.status, corsHeaders);
        }
        console.error("Unhandled error:", err);
        return jsonResponse({ error: "Internal server error" }, 500, corsHeaders);
      }
    },

    async scheduled(_event: ScheduledEvent, env: Env, _ctx: ExecutionContext): Promise<void> {
      // Hourly: clean up abandoned pending shares older than 1 hour
      const cutoff = Math.floor(Date.now() / 1000) - 3600;
      const { results } = await env.DB.prepare(
        `SELECT token FROM shares WHERE status = 'pending' AND created_at < ?`
      ).bind(cutoff).all<{ token: string }>();

      for (const row of results) {
        await env.SHARE_BUCKET.delete(`shares/${row.token}`);
        await env.DB.prepare(`DELETE FROM shares WHERE token = ?`).bind(row.token).run();
      }

      // Clean up stale rate limit entries
      await cleanupExpiredWindows(env.DB);
    },
  }
);

async function route(url: URL, request: Request, env: Env): Promise<Response> {
  const path = url.pathname;
  const method = request.method;

  // POST /api/share — create (auth required)
  if (path === "/api/share" && method === "POST") {
    return handleCreateShare(request, env);
  }

  // PUT /api/share/:token/blob — upload encrypted blob
  const blobMatch = path.match(/^\/api\/share\/([\w]+)\/blob$/);
  if (blobMatch && method === "PUT") {
    return handleUploadBlob(blobMatch[1], request, env);
  }

  // GET /api/share/:token — fetch blob (public)
  const shareMatch = path.match(/^\/api\/share\/([\w]+)$/);
  if (shareMatch && method === "GET") {
    return handleGetShare(shareMatch[1], request, env);
  }

  // DELETE /api/share/:token — revoke (auth required)
  if (shareMatch && method === "DELETE") {
    return handleDeleteShare(shareMatch[1], request, env);
  }

  // GET /api/shares — list caller's shares (auth required)
  if (path === "/api/shares" && method === "GET") {
    return handleListShares(request, env);
  }

  return jsonResponse({ error: "Not found" }, 404);
}

// ---- Handlers ----

async function handleCreateShare(request: Request, env: Env): Promise<Response> {
  const user = await requireAuth(request, env.SUPABASE_URL);

  // Rate limit: 10 creates/hour per user
  const rl = await checkRateLimit(env.DB, `${user.userId}:create`, RATE_LIMITS.create.limit, RATE_LIMITS.create.windowSecs);
  if (!rl.allowed) {
    return jsonResponse(
      { error: "Rate limit exceeded", retry_after: rl.resetAt - Math.floor(Date.now() / 1000) },
      429,
      { "Retry-After": String(rl.resetAt - Math.floor(Date.now() / 1000)) }
    );
  }

  // Enforce body size limit
  const contentLength = parseInt(request.headers.get("Content-Length") || "0");
  if (contentLength > MAX_JSON_BODY_BYTES) {
    return jsonResponse({ error: "Request body too large" }, 413);
  }

  const body = await request.json() as {
    session_id?: string;
    title?: string;
    size_bytes?: number;
  };

  if (!body.session_id) {
    return jsonResponse({ error: "session_id required" }, 400);
  }

  const token = generateToken();
  const now = Math.floor(Date.now() / 1000);

  await env.DB.prepare(
    `INSERT INTO shares (token, user_id, session_id, title, size_bytes, status, created_at)
     VALUES (?, ?, ?, ?, ?, 'pending', ?)`
  ).bind(token, user.userId, body.session_id, body.title ?? null, body.size_bytes ?? 0, now).run();

  // PostHog: share_created event (fire and forget)
  void trackEvent(env, "share_created", user.userId, { size_bytes: body.size_bytes ?? 0 });

  return jsonResponse({ token });
}

async function handleUploadBlob(token: string, request: Request, env: Env): Promise<Response> {
  // Enforce size limit
  const contentLength = parseInt(request.headers.get("Content-Length") || "0");
  if (contentLength > MAX_BLOB_BYTES) {
    return jsonResponse({ error: "Blob too large (max 50MB)" }, 413);
  }

  // Verify token exists and is pending (token-is-auth)
  const row = await env.DB.prepare(
    `SELECT status FROM shares WHERE token = ?`
  ).bind(token).first<{ status: string }>();

  if (!row) return jsonResponse({ error: "Token not found" }, 404);
  if (row.status !== "pending") return jsonResponse({ error: "Share already uploaded" }, 409);

  const body = await request.arrayBuffer();
  if (body.byteLength > MAX_BLOB_BYTES) {
    return jsonResponse({ error: "Blob too large (max 50MB)" }, 413);
  }

  // Store encrypted blob in R2
  await env.SHARE_BUCKET.put(`shares/${token}`, body, {
    httpMetadata: { contentType: "application/octet-stream" },
  });

  await env.DB.prepare(
    `UPDATE shares SET status = 'ready', size_bytes = ? WHERE token = ?`
  ).bind(body.byteLength, token).run();

  return jsonResponse({ status: "ready", size_bytes: body.byteLength });
}

async function handleGetShare(token: string, request: Request, env: Env): Promise<Response> {
  // Rate limit reads by IP (public endpoint)
  const ip = request.headers.get("CF-Connecting-IP") || "unknown";
  const rl = await checkRateLimit(env.DB, `${ip}:read`, RATE_LIMITS.read.limit, RATE_LIMITS.read.windowSecs);
  if (!rl.allowed) {
    return jsonResponse({ error: "Rate limit exceeded" }, 429);
  }

  const row = await env.DB.prepare(
    `SELECT token, session_id, title, created_at, view_count
     FROM shares WHERE token = ? AND status = 'ready'`
  ).bind(token).first<{ token: string; session_id: string; title: string; created_at: number; view_count: number }>();

  if (!row) return new Response("Not found", { status: 404, headers: getPublicCorsHeaders() });

  const obj = await env.SHARE_BUCKET.get(`shares/${token}`);
  if (!obj) return new Response("Blob not found", { status: 404, headers: getPublicCorsHeaders() });

  // Increment view count (fire and forget)
  void env.DB.prepare(`UPDATE shares SET view_count = view_count + 1 WHERE token = ?`).bind(token).run();

  // Track (fire and forget) — hash token for privacy
  const tokenHash = await sha256hex(token);
  void trackEventAnon(env, "share_viewed", { token_hash: tokenHash.slice(0, 16) });

  return new Response(obj.body, {
    headers: {
      ...getPublicCorsHeaders(),
      "Content-Type": "application/octet-stream", // encrypted blob — raw bytes
      "Cache-Control": "public, max-age=300",
    },
  });
}

async function handleDeleteShare(token: string, request: Request, env: Env): Promise<Response> {
  const user = await requireAuth(request, env.SUPABASE_URL);

  const rl = await checkRateLimit(env.DB, `${user.userId}:delete`, RATE_LIMITS.delete.limit, RATE_LIMITS.delete.windowSecs);
  if (!rl.allowed) return jsonResponse({ error: "Rate limit exceeded" }, 429);

  const row = await env.DB.prepare(
    `SELECT user_id FROM shares WHERE token = ? AND status = 'ready'`
  ).bind(token).first<{ user_id: string }>();

  if (!row) return jsonResponse({ error: "Share not found" }, 404);
  if (row.user_id !== user.userId) return jsonResponse({ error: "Forbidden" }, 403);

  await env.SHARE_BUCKET.delete(`shares/${token}`);
  await env.DB.prepare(`UPDATE shares SET status = 'deleted' WHERE token = ?`).bind(token).run();

  void trackEvent(env, "share_revoked", user.userId, {});

  return jsonResponse({ status: "deleted" });
}

async function handleListShares(request: Request, env: Env): Promise<Response> {
  const user = await requireAuth(request, env.SUPABASE_URL);

  const rl = await checkRateLimit(env.DB, `${user.userId}:list`, RATE_LIMITS.list.limit, RATE_LIMITS.list.windowSecs);
  if (!rl.allowed) return jsonResponse({ error: "Rate limit exceeded" }, 429);

  const { results } = await env.DB.prepare(
    `SELECT token, session_id, title, size_bytes, created_at, view_count
     FROM shares WHERE user_id = ? AND status = 'ready'
     ORDER BY created_at DESC LIMIT 100`
  ).bind(user.userId).all();

  return jsonResponse({ shares: results ?? [] });
}

// ---- Helpers ----

function jsonResponse(
  data: unknown,
  status = 200,
  extraHeaders: Record<string, string> = {}
): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: { "Content-Type": "application/json", ...extraHeaders },
  });
}

async function trackEvent(env: Env, event: string, userId: string, props: Record<string, unknown>): Promise<void> {
  if (!env.POSTHOG_API_KEY) return;
  await fetch("https://us.i.posthog.com/capture/", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      api_key: env.POSTHOG_API_KEY,
      event,
      distinct_id: userId,
      properties: { ...props, $lib: "cloudflare-worker" },
    }),
  });
}

async function trackEventAnon(env: Env, event: string, props: Record<string, unknown>): Promise<void> {
  if (!env.POSTHOG_API_KEY) return;
  await fetch("https://us.i.posthog.com/capture/", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      api_key: env.POSTHOG_API_KEY,
      event,
      distinct_id: "anonymous",
      properties: { ...props, $lib: "cloudflare-worker" },
    }),
  });
}

async function sha256hex(input: string): Promise<string> {
  const data = new TextEncoder().encode(input);
  const hash = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(hash)).map(b => b.toString(16).padStart(2, "0")).join("");
}
```

**Step 2: Add Sentry DSN as a Worker secret**

```bash
cd infra/share-worker
# After creating a Sentry project for Cloudflare Workers:
bunx wrangler secret put SENTRY_DSN
# Paste your DSN when prompted
```

**Step 3: Start dev server and smoke test**

```bash
cd infra/share-worker && bun run dev
# In another terminal — expect 401 (no JWT):
curl -X POST http://localhost:8787/api/share \
  -H "Content-Type: application/json" \
  -d '{"session_id":"test"}'
# Expected: {"error":"Missing Authorization header"}
```

**Step 4: Commit**

```bash
git add infra/share-worker/src/index.ts
git commit -m "feat(share): implement hardened Worker handlers with JWT + rate limiting + CORS"
```

---

### Task 2: Deploy Worker + R2 + D1 + Configure Secrets

**Step 1: Create R2 bucket**

```bash
cd infra/share-worker
bunx wrangler r2 bucket create claude-view-shares
```

**Step 2: Create and migrate D1 (production)**

```bash
bunx wrangler d1 create claude-view-share-meta
# Copy database_id into wrangler.toml

bunx wrangler d1 execute claude-view-share-meta --file=./migrations/001_init.sql
```

**Step 3: Set secrets**

```bash
bunx wrangler secret put SENTRY_DSN
bunx wrangler secret put POSTHOG_API_KEY
# For SUPABASE_URL: add to [vars] in wrangler.toml (not a secret — it's public)
```

**Step 4: Deploy**

```bash
bunx wrangler deploy
```

Expected output: `Deployed to https://claude-view-share.<account>.workers.dev`

**Step 5: Smoke test production endpoints**

```bash
# Expect 401 (correct — JWT required):
curl -X POST https://claude-view-share.<account>.workers.dev/api/share \
  -H "Content-Type: application/json" \
  -d '{"session_id":"test"}'

# Expect 404 (no such token):
curl https://claude-view-share.<account>.workers.dev/api/share/doesnotexist
```

**Step 6: Commit wrangler.toml with real IDs**

```bash
git add infra/share-worker/wrangler.toml
git commit -m "feat(share): deploy hardened Worker to Cloudflare"
```

---

## Phase 2: Rust Backend (AES-256-GCM + Share Endpoints)

### Task 3: AES-256-GCM Session Serializer

**Files:**
- Create: `crates/server/src/share_serializer.rs`
- Modify: `crates/server/Cargo.toml` (add aes-gcm, flate2, base64)

**Step 1: Add dependencies**

```bash
cd crates/server
cargo add aes-gcm
cargo add flate2
cargo add base64
```

**Step 2: Implement serializer**

```rust
// crates/server/src/share_serializer.rs
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use flate2::{write::GzEncoder, Compression};
use std::io::Write;
use std::path::Path;

use crate::error::{ApiError, ApiResult};

pub struct EncryptedShare {
    /// AES-256-GCM encrypted, gzip-compressed blob.
    /// Wire format: [12 bytes nonce][ciphertext+tag]
    pub blob: Vec<u8>,
    /// Raw AES-256 key (32 bytes). Caller encodes as base64url for URL fragment.
    pub key: Vec<u8>,
}

/// Serialize a session to a share-ready encrypted blob.
///
/// Pipeline: JSONL file → parse → JSON → gzip → AES-256-GCM encrypt
/// Key is random, unique per share. Never leaves the caller's process except
/// as a URL fragment.
pub async fn serialize_and_encrypt(file_path: &Path) -> ApiResult<EncryptedShare> {
    let path = file_path.to_path_buf();

    // Parse session on blocking thread
    let parsed = tokio::task::spawn_blocking(move || {
        claude_view_core::parse_session(&path)
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Task join: {e}")))?
    .map_err(|e| ApiError::Internal(format!("Parse: {e}")))?;

    // Serialize to JSON
    let json = serde_json::to_vec(&parsed)
        .map_err(|e| ApiError::Internal(format!("Serialize: {e}")))?;

    // Gzip compress the plaintext FIRST (encrypted data is incompressible)
    let compressed = {
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&json)
            .map_err(|e| ApiError::Internal(format!("Gzip write: {e}")))?;
        enc.finish()
            .map_err(|e| ApiError::Internal(format!("Gzip finish: {e}")))?
    };

    // Generate random AES-256 key + nonce
    let key_bytes = Aes256Gcm::generate_key(OsRng);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng::default());
    let cipher = Aes256Gcm::new(&key_bytes);

    // Encrypt: output is ciphertext with 16-byte GCM tag appended
    let ciphertext = cipher
        .encrypt(&nonce, compressed.as_ref())
        .map_err(|e| ApiError::Internal(format!("Encrypt: {e}")))?;

    // Wire format: nonce (12 bytes) || ciphertext+tag
    let mut blob = Vec::with_capacity(12 + ciphertext.len());
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ciphertext);

    Ok(EncryptedShare {
        blob,
        key: key_bytes.to_vec(),
    })
}

/// Encode a raw key as URL-safe base64 (no padding). Safe in URL fragment.
pub fn key_to_base64url(key: &[u8]) -> String {
    use base64::engine::{general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.encode(key)
}
```

**Step 3: Verify compilation**

```bash
cargo check -p claude-view-server
```

**Step 4: Commit**

```bash
git add crates/server/src/share_serializer.rs crates/server/Cargo.toml
git commit -m "feat(server): AES-256-GCM session serializer — gzip then encrypt"
```

---

### Task 4: ShareConfig + Share Route Handlers

**Files:**
- Modify: `crates/server/src/state.rs` (add ShareConfig)
- Create: `crates/server/src/routes/share.rs`
- Modify: `crates/server/src/routes/mod.rs`

**Step 1: Add ShareConfig to AppState**

In `crates/server/src/state.rs`, add:

```rust
pub struct ShareConfig {
    pub worker_url: String,
    pub origin_id: String,
    pub viewer_url: String,
}
```

Add `pub share: Option<ShareConfig>` to `AppState`.

**Step 2: Create share route handlers**

```rust
// crates/server/src/routes/share.rs
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    auth::supabase::{extract_bearer, validate_jwt, AuthUser},
    error::{ApiError, ApiResult},
    share_serializer::{key_to_base64url, serialize_and_encrypt},
    state::AppState,
};

#[derive(Serialize)]
pub struct ShareResponse {
    pub token: String,
    pub url: String,  // includes #k= fragment
}

#[derive(Serialize)]
pub struct ShareListItem {
    pub token: String,
    pub session_id: String,
    pub title: Option<String>,
    pub size_bytes: u64,
    pub created_at: i64,
    pub view_count: u64,
    pub url: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions/{session_id}/share", post(create_share))
        .route("/sessions/{session_id}/share", delete(revoke_share))
        .route("/shares", get(list_shares))
}

fn require_auth(headers: &HeaderMap, state: &AppState) -> ApiResult<AuthUser> {
    let jwks = state.jwks.as_ref()
        .ok_or_else(|| ApiError::Unauthorized("Auth not configured".into()))?;

    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".into()))?;

    let token = extract_bearer(auth_header)
        .ok_or_else(|| ApiError::Unauthorized("Expected Bearer token".into()))?;

    validate_jwt(token, jwks)
        .map_err(|e| ApiError::Unauthorized(format!("Invalid token: {e}")))
}

pub async fn create_share(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<ShareResponse>> {
    let user = require_auth(&headers, &state)?;

    let share_cfg = state.share.as_ref()
        .ok_or_else(|| ApiError::BadRequest("Sharing not configured".into()))?;

    // Get session file path from DB
    let file_path = state.db
        .get_session_file_path(&session_id).await?
        .ok_or_else(|| ApiError::NotFound(format!("Session {session_id}")))?;

    // Get session title for D1 metadata
    let session = state.db.get_session(&session_id).await?
        .ok_or_else(|| ApiError::NotFound(format!("Session {session_id}")))?;
    let title = session.title.or(Some(session.preview.chars().take(80).collect::<String>()));

    // Encrypt the session
    let path = std::path::PathBuf::from(&file_path);
    let encrypted = serialize_and_encrypt(&path).await?;
    let size_bytes = encrypted.blob.len();

    // Call Worker: POST /api/share (forward user's JWT)
    let jwt = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    let client = reqwest::Client::new();
    let token_resp: serde_json::Value = client
        .post(format!("{}/api/share", share_cfg.worker_url))
        .bearer_auth(jwt)
        .json(&serde_json::json!({
            "session_id": session_id,
            "title": title,
            "size_bytes": size_bytes,
        }))
        .send().await
        .map_err(|e| ApiError::Internal(format!("Worker POST failed: {e}")))?
        .json().await
        .map_err(|e| ApiError::Internal(format!("Worker response: {e}")))?;

    let token = token_resp["token"].as_str()
        .ok_or_else(|| ApiError::Internal("Missing token in Worker response".into()))?
        .to_string();

    // Upload encrypted blob: PUT /api/share/:token/blob
    client
        .put(format!("{}/api/share/{}/blob", share_cfg.worker_url, token))
        .body(encrypted.blob)
        .header("Content-Type", "application/octet-stream")
        .send().await
        .map_err(|e| ApiError::Internal(format!("Blob upload failed: {e}")))?;

    // Build URL with AES key in fragment
    let key_b64 = key_to_base64url(&encrypted.key);
    let url = format!("{}/s/{}#k={}", share_cfg.viewer_url, token, key_b64);

    Ok(Json(ShareResponse { token, url }))
}

pub async fn revoke_share(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    let _user = require_auth(&headers, &state)?;
    let _ = session_id; // token comes from query param — handled in frontend
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn list_shares(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    let user = require_auth(&headers, &state)?;
    let share_cfg = state.share.as_ref()
        .ok_or_else(|| ApiError::BadRequest("Sharing not configured".into()))?;

    let jwt = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(format!("{}/api/shares", share_cfg.worker_url))
        .bearer_auth(jwt)
        .send().await
        .map_err(|e| ApiError::Internal(format!("Worker GET failed: {e}")))?
        .json().await
        .map_err(|e| ApiError::Internal(format!("Worker response: {e}")))?;

    Ok(Json(resp))
}
```

**Step 3: Register router**

In `crates/server/src/routes/mod.rs`, add:

```rust
pub mod share;
// In api_routes():
.nest("/api", share::router())
```

**Step 4: Verify compilation**

```bash
cargo check -p claude-view-server
```

**Step 5: Commit**

```bash
git add crates/server/src/routes/share.rs crates/server/src/routes/mod.rs crates/server/src/state.rs
git commit -m "feat(server): share endpoints with AES-256-GCM + JWT + Worker integration"
```

---

## Phase 3: Frontend (Supabase Auth UI + Share Button)

### Task 5: Supabase Auth Client + Sign-in Flow

**Files:**
- Create: `apps/web/src/lib/supabase.ts`
- Create: `apps/web/src/components/SignInPrompt.tsx`

**Step 1: Install Supabase client**

```bash
cd apps/web && bun add @supabase/supabase-js
```

**Step 2: Create Supabase client**

```typescript
// apps/web/src/lib/supabase.ts
import { createClient } from "@supabase/supabase-js";

const SUPABASE_URL = import.meta.env.VITE_SUPABASE_URL as string;
const SUPABASE_PUBLISHABLE_KEY = import.meta.env.VITE_SUPABASE_PUBLISHABLE_KEY as string;

if (!SUPABASE_URL || !SUPABASE_PUBLISHABLE_KEY) {
  throw new Error("Missing VITE_SUPABASE_URL or VITE_SUPABASE_PUBLISHABLE_KEY");
}

export const supabase = createClient(SUPABASE_URL, SUPABASE_PUBLISHABLE_KEY);

export async function getAccessToken(): Promise<string | null> {
  const { data } = await supabase.auth.getSession();
  return data.session?.access_token ?? null;
}
```

**Step 3: Add env vars to `apps/web/.env.local`**

```
VITE_SUPABASE_URL=https://your-project.supabase.co
VITE_SUPABASE_PUBLISHABLE_KEY=eyJ...
```

**Step 4: Create `SignInPrompt` component**

```tsx
// apps/web/src/components/SignInPrompt.tsx
import { useState } from "react";
import { supabase } from "../lib/supabase";

interface Props {
  onSignedIn: () => void;
}

export function SignInPrompt({ onSignedIn }: Props) {
  const [email, setEmail] = useState("");
  const [sent, setSent] = useState(false);
  const [loading, setLoading] = useState(false);

  const handleMagicLink = async () => {
    if (!email.trim()) return;
    setLoading(true);
    const { error } = await supabase.auth.signInWithOtp({
      email: email.trim(),
      options: { emailRedirectTo: window.location.origin },
    });
    setLoading(false);
    if (!error) setSent(true);
  };

  const handleGoogle = async () => {
    await supabase.auth.signInWithOAuth({
      provider: "google",
      options: { redirectTo: window.location.origin },
    });
  };

  if (sent) {
    return (
      <div className="text-center py-8">
        <p className="text-zinc-300">Check your email for the sign-in link.</p>
        <p className="text-zinc-500 text-sm mt-2">
          You can close this and use the link from any device.
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3 p-6 max-w-sm mx-auto">
      <h2 className="text-zinc-200 font-medium">Sign in to enable sharing</h2>
      <p className="text-zinc-500 text-sm">
        One account for sharing and mobile sync.
      </p>

      <button
        onClick={handleGoogle}
        className="flex items-center justify-center gap-2 px-4 py-2 rounded-md
          bg-white text-zinc-900 hover:bg-zinc-100 transition-colors text-sm font-medium"
      >
        Continue with Google
      </button>

      <div className="flex items-center gap-2 text-zinc-600 text-xs">
        <div className="flex-1 border-t border-zinc-800" />
        or
        <div className="flex-1 border-t border-zinc-800" />
      </div>

      <input
        type="email"
        placeholder="your@email.com"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && handleMagicLink()}
        className="px-3 py-2 rounded-md bg-zinc-900 border border-zinc-700
          text-zinc-200 placeholder-zinc-600 text-sm focus:outline-none focus:border-zinc-500"
      />
      <button
        onClick={handleMagicLink}
        disabled={loading || !email.trim()}
        className="px-4 py-2 rounded-md bg-blue-600 hover:bg-blue-500
          text-white text-sm font-medium transition-colors disabled:opacity-50"
      >
        {loading ? "Sending..." : "Send magic link"}
      </button>
    </div>
  );
}
```

**Step 5: Build and verify**

```bash
cd apps/web && bun run build
```

**Step 6: Commit**

```bash
git add apps/web/src/lib/ apps/web/src/components/SignInPrompt.tsx
git commit -m "feat(web): Supabase auth client + SignInPrompt component"
```

---

### Task 6: Share Hook (JWT-authenticated) + Share Button

**Files:**
- Create: `apps/web/src/hooks/use-share.ts`
- Modify: `apps/web/src/components/ConversationView.tsx`

**Step 1: Create share hook (JWT-aware)**

```typescript
// apps/web/src/hooks/use-share.ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { getAccessToken } from "../lib/supabase";

interface ShareResponse {
  token: string;
  url: string; // includes #k= fragment
}

interface ShareListItem {
  token: string;
  session_id: string;
  title: string | null;
  size_bytes: number;
  created_at: number;
  view_count: number;
  url: string;
}

async function authHeaders(): Promise<Record<string, string>> {
  const token = await getAccessToken();
  return token ? { Authorization: `Bearer ${token}` } : {};
}

async function createShare(sessionId: string): Promise<ShareResponse> {
  const headers = await authHeaders();
  const res = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/share`, {
    method: "POST",
    headers,
  });
  if (res.status === 401) throw new Error("AUTH_REQUIRED");
  if (!res.ok) throw new Error(`Share failed: ${res.status}`);
  return res.json();
}

async function revokeShare(token: string): Promise<void> {
  const headers = await authHeaders();
  const res = await fetch(`/api/sessions/${encodeURIComponent(token)}/share`, {
    method: "DELETE",
    headers,
  });
  if (!res.ok) throw new Error("Revoke failed");
}

async function fetchShares(): Promise<ShareListItem[]> {
  const headers = await authHeaders();
  const res = await fetch("/api/shares", { headers });
  if (!res.ok) return [];
  const data = await res.json();
  return data.shares ?? [];
}

export function useCreateShare() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: createShare,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["shares"] }),
  });
}

export function useRevokeShare() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: revokeShare,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["shares"] }),
  });
}

export function useShares() {
  return useQuery({ queryKey: ["shares"], queryFn: fetchShares });
}
```

**Step 2: Add Share button to ConversationView header**

Find the existing header/toolbar in `apps/web/src/components/ConversationView.tsx` and add:

```tsx
import { useState } from "react";
import { Link2, Check, Loader2 } from "lucide-react";
import { useCreateShare } from "../hooks/use-share";
import { SignInPrompt } from "./SignInPrompt";

// Inside component:
const createShare = useCreateShare();
const [shareUrl, setShareUrl] = useState<string | null>(null);
const [showSignIn, setShowSignIn] = useState(false);

const handleShare = async () => {
  if (!sessionId) return;
  try {
    const result = await createShare.mutateAsync(sessionId);
    setShareUrl(result.url);
    await navigator.clipboard.writeText(result.url);
  } catch (err: unknown) {
    if (err instanceof Error && err.message === "AUTH_REQUIRED") {
      setShowSignIn(true);
    }
  }
};

// Button JSX:
<button
  onClick={handleShare}
  disabled={createShare.isPending}
  className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-md
    bg-zinc-800 hover:bg-zinc-700 text-zinc-300 hover:text-white
    transition-colors disabled:opacity-50"
  title={shareUrl ? "Link copied!" : "Share conversation"}
>
  {createShare.isPending ? (
    <Loader2 className="w-4 h-4 animate-spin" />
  ) : shareUrl ? (
    <Check className="w-4 h-4 text-green-400" />
  ) : (
    <Link2 className="w-4 h-4" />
  )}
  {shareUrl ? "Copied!" : "Share"}
</button>

{/* Sign-in modal — shown when share requires auth */}
{showSignIn && (
  <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50"
    onClick={() => setShowSignIn(false)}>
    <div className="bg-zinc-900 border border-zinc-800 rounded-xl shadow-2xl"
      onClick={(e) => e.stopPropagation()}>
      <SignInPrompt onSignedIn={() => { setShowSignIn(false); handleShare(); }} />
    </div>
  </div>
)}
```

**Step 3: Build**

```bash
cd apps/web && bun run build
```

**Step 4: Commit**

```bash
git add apps/web/src/hooks/ apps/web/src/components/ConversationView.tsx
git commit -m "feat(web): share button with JWT auth + sign-in prompt on 401"
```

---

### Task 7: Shared Links in Settings + Sentry Setup

**Files:**
- Modify: Settings page (find with: `grep -r "Settings" apps/web/src --include="*.tsx" -l`)

**Step 1: Install Sentry**

```bash
cd apps/web && bun add @sentry/react
```

**Step 2: Init Sentry in entry point**

In `apps/web/src/main.tsx`, add before `ReactDOM.createRoot`:

```typescript
import * as Sentry from "@sentry/react";

Sentry.init({
  dsn: import.meta.env.VITE_SENTRY_DSN,
  environment: import.meta.env.MODE,
  tracesSampleRate: 0.1,
  enabled: import.meta.env.PROD,
});
```

**Step 3: Add Shared Links section to Settings**

```tsx
import { useShares, useRevokeShare } from "../hooks/use-share";

function SharedLinksSection() {
  const { data: shares, isLoading } = useShares();
  const revokeShare = useRevokeShare();

  if (isLoading) return <div className="text-zinc-500 text-sm">Loading...</div>;
  if (!shares?.length) {
    return <div className="text-zinc-500 text-sm">No shared conversations yet.</div>;
  }

  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-zinc-500 text-left border-b border-zinc-800">
          <th className="pb-2">Title</th>
          <th className="pb-2">Created</th>
          <th className="pb-2">Views</th>
          <th className="pb-2">Link</th>
          <th className="pb-2"></th>
        </tr>
      </thead>
      <tbody>
        {shares.map((share) => (
          <tr key={share.token} className="border-b border-zinc-800/50">
            <td className="py-2 text-zinc-300">{share.title ?? "Untitled"}</td>
            <td className="py-2 text-zinc-500">
              {new Date(share.created_at * 1000).toLocaleDateString()}
            </td>
            <td className="py-2 text-zinc-500">{share.view_count}</td>
            <td className="py-2">
              <button
                onClick={() => navigator.clipboard.writeText(share.url)}
                className="text-blue-400 hover:text-blue-300 truncate max-w-48 text-left"
                title={share.url}
              >
                Copy link
              </button>
            </td>
            <td className="py-2">
              <button
                onClick={() => {
                  if (confirm("Revoke this share? The link will stop working.")) {
                    revokeShare.mutate(share.token);
                  }
                }}
                className="text-red-500 hover:text-red-400 text-xs"
              >
                Revoke
              </button>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
```

**Step 4: Build**

```bash
cd apps/web && bun run build
```

**Step 5: Commit**

```bash
git add apps/web/src/
git commit -m "feat(web): shared links settings section + Sentry init"
```

---

## Phase 4: Viewer SPA (Decrypt in Browser)

### Task 8: Create Viewer SPA with Web Crypto Decrypt

**Files:**
- Create: `apps/share/package.json`
- Create: `apps/share/vite.config.ts`
- Create: `apps/share/index.html`
- Create: `apps/share/src/App.tsx`
- Create: `apps/share/src/crypto.ts`

**Step 1: Create `apps/share/package.json`**

```json
{
  "name": "@claude-view/share",
  "private": true,
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "19.2.0",
    "react-dom": "19.2.0",
    "@sentry/react": "^8.0.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.3.0",
    "tailwindcss": "^4.0.0",
    "typescript": "^5.7.0",
    "vite": "^6.0.0"
  }
}
```

**Step 2: Create `vite.config.ts`**

```typescript
// apps/share/vite.config.ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@web": path.resolve(__dirname, "../web/src"),
    },
  },
});
```

**Step 3: Create `src/crypto.ts` — Web Crypto AES-256-GCM decrypt**

```typescript
// apps/share/src/crypto.ts

/**
 * Decode a base64url string (no padding) to Uint8Array.
 */
function base64urlDecode(str: string): Uint8Array {
  // Convert base64url to base64
  const base64 = str.replace(/-/g, "+").replace(/_/g, "/");
  const padded = base64 + "=".repeat((4 - (base64.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

/**
 * Decompress gzip bytes using DecompressionStream (available in all modern browsers).
 */
async function gunzip(data: Uint8Array): Promise<Uint8Array> {
  const ds = new DecompressionStream("gzip");
  const writer = ds.writable.getWriter();
  const reader = ds.readable.getReader();

  writer.write(data);
  writer.close();

  const chunks: Uint8Array[] = [];
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
  }

  const totalLength = chunks.reduce((sum, c) => sum + c.length, 0);
  const result = new Uint8Array(totalLength);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.length;
  }
  return result;
}

/**
 * Decrypt a share blob using the AES-256-GCM key from the URL fragment.
 *
 * URL format: /s/{token}#k={base64url_key}
 * Blob format: [12 bytes nonce][ciphertext+tag]
 *
 * @returns Parsed session data as a plain object.
 */
export async function decryptShareBlob(
  blob: ArrayBuffer,
  keyBase64url: string
): Promise<unknown> {
  const keyBytes = base64urlDecode(keyBase64url);

  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    keyBytes,
    { name: "AES-GCM" },
    false,
    ["decrypt"]
  );

  const blobBytes = new Uint8Array(blob);
  const iv = blobBytes.slice(0, 12);
  const ciphertext = blobBytes.slice(12);

  const plaintext = await crypto.subtle.decrypt(
    { name: "AES-GCM", iv },
    cryptoKey,
    ciphertext
  );

  // Decompress gzip
  const decompressed = await gunzip(new Uint8Array(plaintext));

  // Parse JSON
  const text = new TextDecoder().decode(decompressed);
  return JSON.parse(text);
}
```

**Step 4: Create `src/App.tsx`**

```tsx
// apps/share/src/App.tsx
import { useEffect, useState } from "react";
import * as Sentry from "@sentry/react";
import { decryptShareBlob } from "./crypto";

const WORKER_URL = import.meta.env.VITE_WORKER_URL || "https://claude-view-share.workers.dev";

Sentry.init({
  dsn: import.meta.env.VITE_SENTRY_DSN,
  enabled: import.meta.env.PROD,
});

export default function App() {
  const token = window.location.pathname.split("/s/")[1]?.split("#")[0];
  const hash = window.location.hash.slice(1); // strip '#'
  const keyBase64url = new URLSearchParams(hash).get("k");

  const [session, setSession] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!token) { setError("No share token in URL"); setLoading(false); return; }
    if (!keyBase64url) { setError("No decryption key in URL fragment. Was the link truncated?"); setLoading(false); return; }

    fetch(`${WORKER_URL}/api/share/${token}`)
      .then(async (res) => {
        if (!res.ok) throw new Error(res.status === 404 ? "Share not found or has been revoked." : "Failed to load share.");
        return res.arrayBuffer();
      })
      .then((blob) => decryptShareBlob(blob, keyBase64url))
      .then((data) => setSession(data))
      .catch((err) => {
        Sentry.captureException(err);
        setError(err instanceof Error ? err.message : "Failed to decrypt share.");
      })
      .finally(() => setLoading(false));
  }, [token, keyBase64url]);

  if (loading) {
    return (
      <div className="min-h-screen bg-zinc-950 flex items-center justify-center">
        <div className="text-zinc-500 text-sm">Decrypting conversation...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="min-h-screen bg-zinc-950 flex items-center justify-center">
        <div className="text-center">
          <p className="text-red-400 text-sm mb-2">{error}</p>
          <a href="https://claudeview.ai" className="text-zinc-500 text-xs hover:text-zinc-400">
            What is claude-view?
          </a>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100">
      <header className="border-b border-zinc-800 px-6 py-3 flex items-center justify-between">
        <div className="text-sm text-zinc-400">
          Shared via <a href="https://claudeview.ai" className="text-white font-medium hover:underline">claude-view</a>
        </div>
        <a href="https://claudeview.ai" className="text-sm text-blue-400 hover:text-blue-300">
          Get claude-view
        </a>
      </header>
      <main className="max-w-4xl mx-auto py-8 px-4">
        {/* TODO: render session.messages using extracted components from @web */}
        <pre className="text-xs text-zinc-600 overflow-auto">
          {JSON.stringify(session, null, 2).slice(0, 2000)}
        </pre>
      </main>
    </div>
  );
}
```

**Step 5: Install and build**

```bash
cd apps/share && bun install && bun run build
```

**Step 6: Deploy to Cloudflare Pages**

```bash
bunx wrangler pages deploy dist --project-name claude-view-share
# Set custom domain in Cloudflare dashboard: share.claudeview.ai
```

**Step 7: Commit**

```bash
git add apps/share/
git commit -m "feat(share): viewer SPA with AES-256-GCM decrypt (Web Crypto API)"
```

---

### Task 9: Wire Up + End-to-End Test

**Step 1: Configure env vars for production**

In `apps/share/.env.production`:
```
VITE_WORKER_URL=https://api-share.claudeview.ai
VITE_SENTRY_DSN=https://xxx@sentry.io/yyy
```

**Step 2: Configure custom domains in Cloudflare dashboard**

- Worker: `api-share.claudeview.ai` -> `claude-view-share` Worker
- Pages: `share.claudeview.ai` -> `claude-view-share` Pages project

**Step 3: End-to-end test**

```
1. Start local claude-view server with SUPABASE_URL set
2. Open the web UI -> sign in with magic link
3. Open any session -> click "Share"
4. Verify: share URL copied to clipboard, format includes #k=...
5. Open link in incognito window (no Supabase session)
6. Verify: viewer loads, decrypts, renders conversation
7. Verify: Worker logs show blob upload (Cloudflare dashboard)
8. Verify: Sentry shows no errors
9. Go to Settings -> Shared Links -> verify share appears
10. Click Revoke -> verify link is dead
```

**Step 4: Commit**

```bash
git commit -m "feat(share): wire custom domains + e2e test passes"
```

---

## Summary

| Phase | Tasks | What it builds | Depends on |
|-------|-------|---------------|------------|
| 1: Worker Handlers | Tasks 1–2 | CRUD handlers + deployment | Hardening Phase 1 (security modules) |
| 2: Rust Backend | Tasks 3–4 | AES-256-GCM serializer + share endpoints | Hardening Task 6 (JWT infra) |
| 3: Frontend | Tasks 5–7 | Supabase auth UI + share button + settings | Phase 2 |
| 4: Viewer SPA | Tasks 8–9 | Web Crypto decrypt + branded viewer | Phase 1 |

**Total:** 9 tasks (down from 13 — security infra removed, handled by hardening plan).

Phases 1 and 2 can run in parallel. Phase 3 depends on Phase 2. Phase 4 depends on Phase 1.

### What was removed (now in production hardening plan)

| Old Task | Now handled by |
|----------|---------------|
| Task 1: Scaffold Worker | Hardening Task 1 |
| Task 2: D1 schema + token gen | Hardening Task 2 |
| Old Task 3-4: Naive handlers (no auth) | Replaced by Task 1 above (hardened handlers) |
| Old Task 5: Deploy (no secrets) | Replaced by Task 2 above (with secrets) |
| Old serializer (gzip only) | Replaced by Task 3 above (AES-256-GCM) |
| Old routes (no JWT) | Replaced by Task 4 above (JWT-authenticated) |
| Old hooks (no auth headers) | Replaced by Task 6 above (JWT-aware) |

### Security properties (inherited from hardening)

- Operator cannot read shared content (AES-256-GCM, key only in URL fragment)
- All mutations require Supabase JWT
- All surfaces rate-limited per user_id
- CORS locked to claudeview.ai / claudeview.com
- Errors tracked in Sentry; usage in PostHog (no PII)

---

### Post-Ship Gaps (Fixed 2026-03-02)

The original impl marked all 9 tasks "DONE" but the viewer SPA (Task 8) was built (`apps/share/dist/`) without being deployed. Share links pointed to the Worker URL which only serves API endpoints, not the viewer HTML.

#### What was broken

1. **`SHARE_VIEWER_URL` pointed to the Worker** — `package.json` `dev:server` set both `SHARE_WORKER_URL` and `SHARE_VIEWER_URL` to the same Worker URL (`claude-view-share-worker-dev.vickyai-tech.workers.dev`). The Worker only has `/api/*` routes, so `/s/{token}` returned `{"error": "Not found"}`.

2. **Viewer SPA never deployed to Cloudflare Pages** — `apps/share/` was built but never `wrangler pages deploy`'d. No Pages project existed.

3. **CORS override bug** — The Worker's outer fetch handler blindly overwrote handler-set CORS headers. `handleGetShare` set `Access-Control-Allow-Origin: *` (public endpoint), but the outer handler replaced it with restrictive origin-locked CORS. This would block the viewer SPA (on `*.pages.dev`) from fetching the blob.

4. **Tailwind content scanning missed shared components** — `apps/share/src/index.css` had `@import "tailwindcss"` but no `@source` directive pointing to `packages/shared/src/`. Tailwind v4 didn't scan the shared components, so their utility classes weren't generated (CSS was 7 KB instead of 59 KB). Result: completely broken styling.

5. **Missing SPA routing** — No `_redirects` file for Cloudflare Pages. Visiting `/s/{token}` returned Pages' default 404 instead of rewriting to `index.html`.

#### Fixes applied

| Fix | File | Detail |
| --- | --- | --- |
| Deploy viewer to Pages | Cloudflare | Created `claude-view-share-viewer-dev` Pages project, deployed `apps/share/dist/` |
| Fix `SHARE_VIEWER_URL` | `package.json` | `dev:server` now uses `https://claude-view-share-viewer-dev.pages.dev` |
| Fix CORS override | `infra/share-worker/src/index.ts` | Outer handler checks `response.headers.has('Access-Control-Allow-Origin')` before applying default CORS |
| Fix Tailwind scanning | `apps/share/src/index.css` | Added `@source "../../../packages/shared/src"`, `@custom-variant dark`, fonts, prose/code styles |
| Add SPA routing | `apps/share/public/_redirects` | `/s/*  /index.html  200` |

#### Remaining: Production deployment

The dev viewer is live at `https://claude-view-share-viewer-dev.pages.dev`. Production still needs:

1. **Create prod Pages project:** `bunx wrangler pages project create claude-view-share-viewer-prod --production-branch main`
2. **Build with prod Worker URL:** `cd apps/share && bun run build` (`.env.production` already has `VITE_WORKER_URL=https://api-share.claudeview.ai`)
3. **Deploy:** `bunx wrangler pages deploy dist --project-name claude-view-share-viewer-prod --branch main --commit-dirty=true`
4. **Add custom domain:** In Cloudflare Dashboard, add `share.claudeview.ai` custom domain to the `claude-view-share-viewer-prod` Pages project
5. **Verify `SHARE_VIEWER_URL` in `preview` script:** Already set to `https://share.claudeview.ai` — correct, no change needed
6. **Deploy CORS fix to prod Worker:** `cd infra/share-worker && bunx wrangler deploy` (no `--env dev` flag = prod)
