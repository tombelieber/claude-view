# Production Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Harden both cloud-facing surfaces (Cloudflare Share Worker + Fly.io Relay) with Supabase user auth, AES-256-GCM E2E encryption, D1 rate limiting, CORS lockdown, and Sentry + PostHog observability — baked in from day one.

**Architecture:** This plan extends `2026-02-28-conversation-sharing-impl.md`. It replaces the security-naive Worker implementation with one that validates Supabase JWTs, rate-limits per user_id, encrypts blobs client-side (AES-256-GCM, key in URL fragment), and locks CORS. It then applies the same JWT + rate limiting + CORS + observability treatment to the existing Fly.io relay.

**Tech Stack:** Cloudflare Workers (TypeScript), D1, R2, `@sentry/cloudflare`, PostHog HTTP API, `jose` (JWT), `aes-gcm` (Rust), Web Crypto API (browser), Supabase Auth, `@supabase/supabase-js` (Expo + React), `jsonwebtoken` + `jwks-client` (Rust), Tower HTTP middleware (Rust).

**Design Doc:** `docs/plans/2026-02-28-production-hardening-design.md`

**Relationship to sharing plan:** Execute this plan INSTEAD OF the sharing plan. Phases 0 and 5 are new. Phases 1–4 replace the equivalent phases in the sharing plan with security baked in.

---

## Phase 0: Supabase Auth Setup (Foundation for everything)

### Task 0: Create Supabase Project + Configure Auth

**No code files yet — this is cloud console setup.**

**Step 1: Create Supabase project**

Go to [supabase.com](https://supabase.com) → New project:
- Name: `claude-view`
- Region: pick closest to your users (ap-northeast-1 for Tokyo)
- Password: generate a strong one, save it

**Step 2: Enable auth providers**

Supabase dashboard → Authentication → Providers:
- Email: Enable. Turn on "Confirm email" = OFF (magic link only, no confirmation step). Enable "Passwordless / magic link".
- Google: Enable. Create OAuth credentials at [console.cloud.google.com](https://console.cloud.google.com) → APIs → Credentials → OAuth 2.0 Client IDs. Paste Client ID + Secret into Supabase.

**Step 3: Configure redirect URLs**

Supabase dashboard → Authentication → URL Configuration:
- Site URL: `https://claudeview.ai`
- Redirect URLs (add all):
  ```
  https://claudeview.ai/**
  https://claudeview.com/**
  claudeview://auth
  http://localhost:5173/**
  http://localhost:8081/**
  ```

**Step 4: Note your credentials**

From Supabase dashboard → Project Settings → API:
- `Project URL`: e.g. `https://abcdef.supabase.co`
- `anon/public key`: starts with `eyJ...`
- JWKS URL: `{Project URL}/auth/v1/.well-known/jwks.json`

Save these — you will need them in every subsequent task.

**Step 5: Add Supabase vars to per-service `.env.example` files**

> **Prerequisite:** The env-cleanup plan (`2026-02-28-env-cleanup-impl.md`) must run first. It deletes the root `.env.example` and creates per-service files at `crates/server/.env.example` and `apps/web/.env.example`.

Add the Supabase vars to `crates/server/.env.example` (append after existing lines):

```bash
# Supabase Auth (sharing/auth)
# SUPABASE_URL=https://your-project.supabase.co
# SHARE_WORKER_URL=https://api-share.claudeview.ai
# SHARE_VIEWER_URL=https://share.claudeview.ai
```

The web frontend vars (`VITE_SUPABASE_URL`, `VITE_SUPABASE_ANON_KEY`) are already in `apps/web/.env.example` (added by the env-cleanup plan).

```bash
git add crates/server/.env.example
git commit -m "chore: add Supabase env vars to server .env.example"
```

---

## Phase 1: Cloudflare Worker (Hardened from day 1)

This replaces Tasks 1–5 from `2026-02-28-conversation-sharing-impl.md`.

### Task 1: Scaffold Worker Project + Install Security Dependencies

**Files:**
- Create: `infra/share-worker/wrangler.toml`
- Create: `infra/share-worker/package.json`
- Create: `infra/share-worker/tsconfig.json`
- Create: `infra/share-worker/src/index.ts`

**Step 1: Create directory structure**

```bash
mkdir -p infra/share-worker/src infra/share-worker/migrations
cd infra/share-worker
```

**Step 2: Create `wrangler.toml`**

```toml
name = "claude-view-share"
main = "src/index.ts"
compatibility_date = "2024-12-01"
compatibility_flags = ["nodejs_compat"]  # Required by @sentry/cloudflare (uses Node.js APIs)

[vars]
ENVIRONMENT = "production"
SUPABASE_URL = "" # fill from Phase 0
POSTHOG_API_KEY = "" # fill after PostHog setup

[[r2_buckets]]
binding = "SHARE_BUCKET"
bucket_name = "claude-view-shares"

[[d1_databases]]
binding = "DB"
database_name = "claude-view-share-meta"
database_id = "" # filled after `wrangler d1 create`

[triggers]
crons = ["0 * * * *"] # hourly cleanup of pending shares

[env.dev]
name = "claude-view-share-dev"
[env.dev.vars]
ENVIRONMENT = "development"

[[env.dev.r2_buckets]]
binding = "SHARE_BUCKET"
bucket_name = "claude-view-shares-dev"

[[env.dev.d1_databases]]
binding = "DB"
database_name = "claude-view-share-meta-dev"
database_id = "" # filled after `wrangler d1 create --env dev`
```

**Step 3: Create `package.json`**

```json
{
  "name": "claude-view-share-worker",
  "private": true,
  "scripts": {
    "dev": "wrangler dev",
    "deploy": "wrangler deploy",
    "db:migrate": "wrangler d1 execute claude-view-share-meta --file=./migrations/001_init.sql",
    "db:migrate:dev": "wrangler d1 execute claude-view-share-meta-dev --local --file=./migrations/001_init.sql"
  },
  "devDependencies": {
    "wrangler": "^3.100.0",
    "@cloudflare/workers-types": "^4.20250214.0",
    "typescript": "^5.7.0"
  },
  "dependencies": {
    "jose": "^5.10.0",
    "@sentry/cloudflare": "^8.0.0"
  }
}
```

**Step 4: Create `tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ES2022",
    "moduleResolution": "bundler",
    "lib": ["ES2022"],
    "types": ["@cloudflare/workers-types"],
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true
  },
  "include": ["src/**/*.ts"]
}
```

**Step 5: Add `.wrangler` to root `.gitignore`**

Before running any wrangler commands, add to the root `.gitignore`:
```
# Wrangler local state (D1, KV, R2 local dev)
.wrangler/
```

**Step 6: Install dependencies**

```bash
cd infra/share-worker && bun install
```

Note: `infra/share-worker` is intentionally NOT a workspace member (same pattern as `sidecar/`). The root `package.json` workspaces are `["apps/*", "packages/*"]` only. Do NOT add `"infra/*"` to the workspaces array — Workers-specific types (`@cloudflare/workers-types`) must not mix with DOM/Node types in the shared resolver.

**Step 7: Add worker install to postinstall hook (optional but recommended)**

In root `package.json`, update the `postinstall` script to match the `sidecar/` precedent:
```json
"postinstall": "cd sidecar && bun install && cd ../infra/share-worker && bun install"
```

This ensures `bun install` at the repo root also installs worker dependencies (needed for CI).

**Step 8: Commit scaffold**

```bash
git add infra/share-worker/
git commit -m "feat(share): scaffold hardened Worker project"
```

---

### Task 2: D1 Schema + Token + Rate Limiter

**Files:**
- Create: `infra/share-worker/migrations/001_init.sql`
- Create: `infra/share-worker/src/token.ts`
- Create: `infra/share-worker/src/rate-limit.ts`

**Step 1: Create D1 migration (hardened schema)**

```sql
-- migrations/001_init.sql

CREATE TABLE IF NOT EXISTS shares (
  token       TEXT PRIMARY KEY,
  user_id     TEXT NOT NULL,           -- Supabase user UUID (from JWT sub)
  session_id  TEXT NOT NULL,
  title       TEXT,                    -- plaintext user-chosen label
  size_bytes  INTEGER NOT NULL DEFAULT 0,
  status      TEXT NOT NULL DEFAULT 'pending', -- pending | ready | deleted
  created_at  INTEGER NOT NULL,
  expires_at  INTEGER,                 -- null = no expiry
  view_count  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_shares_user_id ON shares(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_shares_status_created ON shares(status, created_at);

-- Sliding-window rate limit counters
CREATE TABLE IF NOT EXISTS rate_limits (
  key     TEXT NOT NULL,  -- "{user_id}:{endpoint}" or "{ip}:{endpoint}"
  window  INTEGER NOT NULL, -- unix timestamp floored to window size (seconds)
  count   INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (key, window)
);
```

**Step 2: Create token generator**

```typescript
// src/token.ts
const BASE62 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/** Generate a 22-char base62 token (131 bits of entropy). */
export function generateToken(): string {
  const bytes = new Uint8Array(22);
  crypto.getRandomValues(bytes);
  let result = "";
  for (const byte of bytes) {
    result += BASE62[byte % 62];
  }
  return result;
}
```

**Step 3: Create rate limiter**

> **Scale note:** D1 is a SQLite-backed store optimized for read-heavy workloads. Using it
> for rate limiting (write-heavy upserts) works at MVP scale (<1000 users) but is NOT the
> Cloudflare-recommended approach for high-traffic rate limiting. Cloudflare's own docs
> recommend **Workers Rate Limiting binding** (`CF-RateLimit`) or **Durable Objects** for
> production rate limiting at scale. **Migration path:** When traffic exceeds ~100 req/s,
> replace D1 rate limiting with `cloudflare:rate-limiter` binding (zero-config, built-in).
> This is documented as a known limitation, not a blocker for MVP.

```typescript
// src/rate-limit.ts
// NOTE: D1Database is an ambient type from @cloudflare/workers-types (declared in tsconfig.json
// "types" array). Do NOT import it — it's a .d.ts file, not a runtime module.

interface RateLimitResult {
  allowed: boolean;
  remaining: number;
  resetAt: number; // unix timestamp when window resets
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
  windowSecs: number
): Promise<RateLimitResult> {
  const now = Math.floor(Date.now() / 1000);
  const window = Math.floor(now / windowSecs) * windowSecs;
  const resetAt = window + windowSecs;

  // Upsert counter for this window
  const result = await db
    .prepare(
      `INSERT INTO rate_limits (key, window, count) VALUES (?, ?, 1)
       ON CONFLICT (key, window) DO UPDATE SET count = count + 1
       RETURNING count`
    )
    .bind(key, window)
    .first<{ count: number }>();

  const count = result?.count ?? 1;
  const allowed = count <= limit;
  const remaining = Math.max(0, limit - count);

  return { allowed, remaining, resetAt };
}

/** Periodic cleanup — call from scheduled handler.
 * NOTE: The PRIMARY KEY is (key, window). `DELETE WHERE window < ?` still requires
 * scanning all keys because `window` is the second column in the composite PK.
 * At MVP scale (<1000 users) this is fine — D1 handles small tables efficiently.
 * If the table grows large, add a secondary index: CREATE INDEX idx_window ON rate_limits(window).
 */
export async function cleanupExpiredWindows(db: D1Database): Promise<void> {
  const cutoff = Math.floor(Date.now() / 1000) - 3600; // keep 1 hour of history
  await db.prepare(`DELETE FROM rate_limits WHERE window < ?`).bind(cutoff).run();
}
```

**Step 4: Create D1 databases and run migration**

```bash
cd infra/share-worker

# Production
bunx wrangler d1 create claude-view-share-meta
# Copy database_id into wrangler.toml [[d1_databases]]

# Dev (local)
bunx wrangler d1 execute claude-view-share-meta --local --file=./migrations/001_init.sql
```

**Step 5: Commit**

```bash
git add infra/share-worker/migrations/ infra/share-worker/src/token.ts infra/share-worker/src/rate-limit.ts
git commit -m "feat(share): D1 schema + token generator + rate limiter"
```

---

### Task 3: JWT Validation Middleware

**Files:**
- Create: `infra/share-worker/src/auth.ts`

```typescript
// src/auth.ts
import { createRemoteJWKSet, jwtVerify, type JWTPayload } from "jose";

export interface AuthUser {
  userId: string;
  email: string | undefined;
}

let cachedJWKS: ReturnType<typeof createRemoteJWKSet> | null = null;

function getJWKS(supabaseUrl: string): ReturnType<typeof createRemoteJWKSet> {
  if (!cachedJWKS) {
    const jwksUrl = new URL(`${supabaseUrl}/auth/v1/.well-known/jwks.json`);
    cachedJWKS = createRemoteJWKSet(jwksUrl);
  }
  return cachedJWKS;
}

/**
 * Validate a Supabase JWT from the Authorization: Bearer header.
 * Returns the authenticated user or throws an error.
 */
export async function requireAuth(
  request: Request,
  supabaseUrl: string
): Promise<AuthUser> {
  const authHeader = request.headers.get("Authorization");
  if (!authHeader?.startsWith("Bearer ")) {
    throw new AuthError("Missing Authorization header", 401);
  }

  const token = authHeader.slice(7);
  const JWKS = getJWKS(supabaseUrl);

  let payload: JWTPayload;
  try {
    const result = await jwtVerify(token, JWKS, {
      issuer: `${supabaseUrl}/auth/v1`,
    });
    payload = result.payload;
  } catch (err) {
    throw new AuthError(`Invalid token: ${String(err)}`, 401);
  }

  const userId = payload.sub;
  if (!userId) throw new AuthError("Token missing sub claim", 401);

  return {
    userId,
    email: typeof payload["email"] === "string" ? payload["email"] : undefined,
  };
}

export class AuthError extends Error {
  constructor(message: string, public readonly status: number) {
    super(message);
  }
}
```

**Step 1: Write test for the auth module (local mock test)**

```bash
cd infra/share-worker
cat > src/auth.test.ts << 'EOF'
// Manual smoke test — run with: bunx wrangler dev + curl
// Formal unit tests require Workers test framework (vitest-pool-workers)
// Skip for now — integration test in Task 5
EOF
```

**Step 2: Commit**

```bash
git add infra/share-worker/src/auth.ts infra/share-worker/src/auth.test.ts
git commit -m "feat(share): Supabase JWT validation middleware"
```

---

### Task 4: Worker Handlers (Hardened)

**Files:**
- Create: `infra/share-worker/src/cors.ts`
- Create: `infra/share-worker/src/index.ts`

**Step 1: Create CORS helper**

```typescript
// src/cors.ts

const ALLOWED_ORIGINS = [
  "https://share.claudeview.ai",
  "https://claudeview.ai",
  "https://share.claudeview.com",
  "https://claudeview.com",
];

const DEV_ORIGIN_PATTERN = /^http:\/\/localhost(:\d+)?$/;

export function getCorsHeaders(request: Request, env: { ENVIRONMENT: string }): Record<string, string> {
  const origin = request.headers.get("Origin") || "";
  const isDev = env.ENVIRONMENT === "development";

  const allowedOrigin =
    ALLOWED_ORIGINS.includes(origin) || (isDev && DEV_ORIGIN_PATTERN.test(origin))
      ? origin
      : ALLOWED_ORIGINS[0]; // fallback to primary domain

  return {
    "Access-Control-Allow-Origin": allowedOrigin,
    "Access-Control-Allow-Methods": "GET, POST, PUT, DELETE, OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type, Authorization",
    "Vary": "Origin",
  };
}

/** GET /api/share/:token is public — allow all origins (Slack previews etc) */
export function getPublicCorsHeaders(): Record<string, string> {
  return {
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET, OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type",
  };
}
```

**Step 2: Create main `src/index.ts` with all handlers**

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
  SENTRY_DSN?: string;  // optional — Sentry disabled if not set
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
    dsn: env.SENTRY_DSN,
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

  // DELETE /api/share/:token — revoke by token (auth required)
  if (shareMatch && method === "DELETE") {
    return handleDeleteShare(shareMatch[1], request, env);
  }

  // DELETE /api/shares/by-session/:session_id — revoke by session_id (auth required)
  // This is used by the Rust server which only has session_id, not the share token.
  const sessionDeleteMatch = path.match(/^\/api\/shares\/by-session\/([\w-]+)$/);
  if (sessionDeleteMatch && method === "DELETE") {
    return handleDeleteShareBySession(sessionDeleteMatch[1], request, env);
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
  // Enforce size limit (defense-in-depth: Content-Length header check + actual body check)
  // NOTE: Cloudflare Workers enforce a platform-level max request body of 100MB (free plan)
  // or 500MB (paid plans). Even if Content-Length is missing/forged, the platform rejects
  // oversized requests before they reach this code. Our 50MB limit is well within bounds.
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

async function handleDeleteShareBySession(sessionId: string, request: Request, env: Env): Promise<Response> {
  const user = await requireAuth(request, env.SUPABASE_URL);

  const rl = await checkRateLimit(env.DB, `${user.userId}:delete`, RATE_LIMITS.delete.limit, RATE_LIMITS.delete.windowSecs);
  if (!rl.allowed) return jsonResponse({ error: "Rate limit exceeded" }, 429);

  // Look up the share token by session_id + user_id
  const row = await env.DB.prepare(
    `SELECT token, user_id FROM shares WHERE session_id = ? AND user_id = ? AND status = 'ready'`
  ).bind(sessionId, user.userId).first<{ token: string; user_id: string }>();

  if (!row) return jsonResponse({ error: "Share not found" }, 404);

  await env.SHARE_BUCKET.delete(`shares/${row.token}`);
  await env.DB.prepare(`UPDATE shares SET status = 'deleted' WHERE token = ?`).bind(row.token).run();

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

**Step 3: Add Sentry DSN as a Worker secret**

```bash
cd infra/share-worker
# After creating a Sentry project for Cloudflare Workers:
bunx wrangler secret put SENTRY_DSN
# Paste your DSN when prompted
```

**Step 4: Start dev server and smoke test**

```bash
cd infra/share-worker && bun run dev
# In another terminal — expect 401 (no JWT):
curl -X POST http://localhost:8787/api/share \
  -H "Content-Type: application/json" \
  -d '{"session_id":"test"}'
# Expected: {"error":"Missing Authorization header"}
```

**Step 5: Commit**

```bash
git add infra/share-worker/src/
git commit -m "feat(share): implement hardened Worker handlers with JWT + rate limiting + CORS"
```

---

### Task 5: Deploy Worker + R2 + D1 + Configure Secrets

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

## Phase 2: Rust Backend (AES-256-GCM + JWT + Share Endpoints)

### Task 6: Add Dependencies + Supabase JWT Validation

**Files:**
- Modify: `crates/server/Cargo.toml`
- Create: `crates/server/src/auth/mod.rs` (**NEW** — this module does not exist yet)
- Create: `crates/server/src/auth/supabase.rs`
- Modify: `crates/server/src/lib.rs` (add `pub mod auth;`)
- Modify: `crates/server/src/error.rs` (add `Unauthorized` variant)
- Modify: `crates/server/src/state.rs` (add `jwks` and `share` fields)
- Modify: `crates/server/src/lib.rs` (update all `AppState` construction sites)

**Step 0: Verify current state (IMPORTANT)**

The server crate has NO `auth` module. The existing crypto module (`src/crypto.rs`) handles NaCl device pairing — unrelated to JWT. You are creating the auth module from scratch.

`ApiError` in `src/error.rs` has these variants: `SessionNotFound`, `ProjectNotFound`, `Parse`, `Discovery`, `Database`, `Internal`, `BadRequest`, `NotFound`, `Conflict`, `ServiceUnavailable`. There is NO `Unauthorized` variant — you must add it.

`AppState` in `src/state.rs` is constructed in **5 places**: `AppState::new()`, `AppState::new_with_indexing()`, `AppState::new_with_indexing_and_registry()`, `create_app_with_git_sync()` in `lib.rs`, and `create_app_full()` in `lib.rs`. ALL must be updated.

**Step 1: Add dependencies**

```bash
cd crates/server
cargo add aes-gcm
cargo add flate2
cargo add jsonwebtoken
# reqwest and base64 already exist as workspace deps — no need to add
```

**Step 2: Add `Unauthorized` variant to `ApiError`**

In `crates/server/src/error.rs`, add a new variant to the `ApiError` enum:

```rust
// Add this variant alongside the existing ones:
#[error("Unauthorized: {0}")]
Unauthorized(String),
```

And in the `IntoResponse` impl for `ApiError`, add the mapping **inside the existing `match self` block** (before the final `};` that closes the match). It MUST produce a `(StatusCode, ErrorResponse)` tuple — the same shape as every other arm — because the match's single final line `(status, Json(error_response)).into_response()` handles the conversion. Do NOT call `.into_response()` inside the arm:

```rust
ApiError::Unauthorized(msg) => {
    tracing::warn!(message = %msg, "Unauthorized");
    (
        StatusCode::UNAUTHORIZED,
        ErrorResponse::new(msg),
    )
}
```

**Step 3: Create auth module structure**

```bash
mkdir -p crates/server/src/auth
```

Create `crates/server/src/auth/mod.rs`:

```rust
pub mod supabase;
pub use supabase::{AuthUser, JwksCache};
```

Add to `crates/server/src/lib.rs` (alongside the other `pub mod` declarations):

```rust
pub mod auth;
```

**Step 4: Create Supabase JWT validator**

```rust
// crates/server/src/auth/supabase.rs
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,       // user_id
    pub email: Option<String>,
    pub exp: u64,
    pub iss: String,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
    pub email: Option<String>,
}

/// Cached JWKS key with rotation support.
/// Fetched at startup, re-fetched on validation failure (kid mismatch / key rotation).
/// Pattern: Auth0 SDK, Envoy proxy — retry JWKS fetch when token validation fails.
pub struct JwksCache {
    pub decoding_key: DecodingKey,
    pub issuer: String,
    pub supabase_url: String, // kept for re-fetch on rotation
}

// NOTE: fetch_jwks() and jwk_to_pem() were removed — they were dead code.
// fetch_jwks called jwk_to_pem which always returned Err. Use fetch_decoding_key() below.

/// Validate a Bearer JWT. Extracts claims on success.
pub fn validate_jwt(token: &str, cache: &JwksCache) -> anyhow::Result<AuthUser> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[&cache.issuer]);

    let data = decode::<Claims>(token, &cache.decoding_key, &validation)?;
    Ok(AuthUser {
        user_id: data.claims.sub,
        email: data.claims.email,
    })
}

/// Extract Bearer token from Authorization header.
pub fn extract_bearer(auth_header: &str) -> Option<&str> {
    auth_header.strip_prefix("Bearer ")
}

/// Fetch JWKS and return DecodingKey directly from JWK array.
pub async fn fetch_decoding_key(supabase_url: &str) -> anyhow::Result<JwksCache> {
    let jwks_url = format!("{}/auth/v1/.well-known/jwks.json", supabase_url);
    let resp: serde_json::Value = reqwest::get(&jwks_url).await?.json().await?;

    let key_json = resp["keys"]
        .as_array()
        .and_then(|k| k.first())
        .ok_or_else(|| anyhow::anyhow!("Empty JWKS"))?;

    let jwk: jsonwebtoken::jwk::Jwk = serde_json::from_value(key_json.clone())?;
    let decoding_key = DecodingKey::from_jwk(&jwk)?;

    Ok(JwksCache {
        decoding_key,
        issuer: format!("{}/auth/v1", supabase_url),
        supabase_url: supabase_url.to_string(),
    })
}

/// Validate JWT with automatic JWKS rotation on failure.
/// If validation fails (e.g., kid mismatch after Supabase key rotation),
/// re-fetch JWKS once and retry. Returns the validated user + updated cache.
/// Pattern proven by: Auth0 SDK (auto-refresh on kid mismatch), Envoy proxy
/// (periodic + on-demand JWKS refresh), Spring Security NimbusJwtDecoder.
pub async fn validate_jwt_with_rotation(
    token: &str,
    cache: &JwksCache,
) -> Result<(AuthUser, Option<JwksCache>), anyhow::Error> {
    // First attempt with cached key
    match validate_jwt(token, cache) {
        Ok(user) => Ok((user, None)),
        Err(first_err) => {
            tracing::info!("JWT validation failed, re-fetching JWKS (possible key rotation)");
            // Re-fetch JWKS and retry once
            match fetch_decoding_key(&cache.supabase_url).await {
                Ok(new_cache) => {
                    let user = validate_jwt(token, &new_cache)?;
                    Ok((user, Some(new_cache)))
                }
                Err(fetch_err) => {
                    tracing::error!("JWKS re-fetch failed: {fetch_err}");
                    Err(first_err) // return original validation error
                }
            }
        }
    }
}
```

**Step 5: Add JwksCache + ShareConfig to AppState**

In `crates/server/src/state.rs`, add the new fields to the `AppState` struct:

```rust
use crate::auth::supabase::JwksCache;

// Add these fields to the existing AppState struct:
pub struct AppState {
    // ... existing 18 fields (start_time through sidecar) ...
    pub jwks: Option<Arc<tokio::sync::RwLock<JwksCache>>>,  // None if SUPABASE_URL not configured; RwLock for JWKS rotation
    pub share: Option<ShareConfig>,
}

// Add this struct in the same file:
pub struct ShareConfig {
    pub worker_url: String,
    pub viewer_url: String,
    pub http_client: reqwest::Client, // reuse single client (connection pool + TLS cache)
}
```

**CRITICAL: Update ALL 5 AppState construction sites.** Add `jwks: None, share: None` to each:

1. `crates/server/src/state.rs` → `AppState::new()` (~line 91)
2. `crates/server/src/state.rs` → `AppState::new_with_indexing()` (~line 124)
3. `crates/server/src/state.rs` → `AppState::new_with_indexing_and_registry()` (~line 156)
4. `crates/server/src/lib.rs` → `create_app_with_git_sync()` (~line 107)
5. `crates/server/src/lib.rs` → `create_app_full()` (~line 170) — **this is the main production entry point**

For the production entry point (`create_app_full`), replace `jwks: None` with the loaded value.

**Step 6: Load JWKS at startup + pass into `create_app_full`**

**CRITICAL:** `create_app_full()` is `pub fn`, NOT `pub async fn`. You CANNOT `.await` inside it. Load JWKS in `main.rs` (which IS async) and pass the result as a parameter.

**6a. Add parameters to `create_app_full()` signature** in `crates/server/src/lib.rs`:

```rust
pub fn create_app_full(
    db: Database,
    indexing: Arc<IndexingState>,
    registry: RegistryHolder,
    search_index: SearchIndexHolder,
    shutdown: tokio::sync::watch::Receiver<bool>,
    static_dir: Option<PathBuf>,
    sidecar: Arc<sidecar::SidecarManager>,
    jwks: Option<Arc<tokio::sync::RwLock<JwksCache>>>,  // NEW — loaded in main.rs, RwLock for rotation
    share: Option<ShareConfig>,      // NEW — loaded in main.rs
) -> Router {
```

Add import at top of `lib.rs`:

```rust
use crate::auth::supabase::JwksCache;
use crate::state::ShareConfig;
```

**6b. Load JWKS in `main.rs`** (insert between the shutdown channel setup and the `create_app_full()` call, around line 282):

Add these imports at the top of `main.rs` (alongside existing `claude_view_server::` imports):

```rust
use claude_view_server::auth::supabase::fetch_decoding_key;
use claude_view_server::state::ShareConfig;
```

Then add the loading code:

```rust
// Load Supabase JWKS (async — must happen in main, not create_app_full)
// Wrapped in Arc<RwLock> to support JWKS rotation on kid mismatch
let jwks = if let Ok(supabase_url) = std::env::var("SUPABASE_URL") {
    match fetch_decoding_key(&supabase_url).await {
        Ok(cache) => {
            tracing::info!("Supabase JWKS loaded");
            Some(Arc::new(tokio::sync::RwLock::new(cache)))
        }
        Err(e) => {
            tracing::warn!("Failed to load Supabase JWKS: {e}. Auth will be disabled.");
            None
        }
    }
} else {
    tracing::info!("SUPABASE_URL not set — auth disabled (dev mode)");
    None
};

let share = match (std::env::var("SHARE_WORKER_URL"), std::env::var("SHARE_VIEWER_URL")) {
    (Ok(worker_url), Ok(viewer_url)) => Some(ShareConfig {
        worker_url,
        viewer_url,
        http_client: reqwest::Client::new(), // single client, reused across all share handlers
    }),
    _ => {
        tracing::info!("SHARE_WORKER_URL/SHARE_VIEWER_URL not set — sharing disabled");
        None
    }
};
```

**6c. Pass into `create_app_full()`** at the call site in `main.rs` (append `, jwks, share` after the existing 7 arguments):

```rust
// Append these two new args to the existing create_app_full(...) call:
let app = create_app_full(
    db.clone(), indexing.clone(), registry_holder.clone(),
    search_index_holder.clone(), shutdown_rx, static_dir, sidecar,
    jwks, share,  // NEW — from Step 6b above
);
```

**6d. Use `jwks` and `share` params in AppState construction** inside `create_app_full()` instead of `None`:

```rust
// In the AppState struct literal within create_app_full():
jwks,   // was: jwks: None
share,  // was: share: None
```

For the other 4 AppState construction sites (`AppState::new()`, `new_with_indexing()`, `new_with_indexing_and_registry()`, `create_app_with_git_sync()`), keep `jwks: None, share: None`.

**Step 7: Verify compilation**

```bash
cargo check -p claude-view-server
```

**Step 8: Commit**

```bash
git add crates/server/src/auth/ crates/server/src/state.rs crates/server/src/lib.rs crates/server/src/main.rs crates/server/Cargo.toml
git commit -m "feat(server): add Supabase JWT validation + AppState jwks field"
```

---

### Task 7: AES-256-GCM Session Serializer

**Files:**
- Create: `crates/server/src/share_serializer.rs`

```rust
// crates/server/src/share_serializer.rs
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm,
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
///
/// NOTE: `parse_session` is an `async fn` — do NOT use `spawn_blocking`.
/// Call it directly with `.await`.
pub async fn serialize_and_encrypt(file_path: &Path) -> ApiResult<EncryptedShare> {
    // parse_session is async (uses tokio::fs internally) — await it directly
    let parsed = claude_view_core::parse_session(file_path)
        .await
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
    let key_bytes = Aes256Gcm::generate_key(&mut OsRng);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
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

**Step 1: Declare module in `lib.rs`**

Add to `crates/server/src/lib.rs` alongside the other `pub mod` declarations:

```rust
pub mod share_serializer;
```

Without this, the file exists but the compiler ignores it and `use crate::share_serializer::...` in `routes/share.rs` will fail with "unresolved import".

**Step 2: Verify compilation** (`base64` is already a workspace dependency in `crates/server/Cargo.toml`)

```bash
cargo check -p claude-view-server
```

**Step 3: Commit**

```bash
git add crates/server/src/share_serializer.rs crates/server/src/lib.rs crates/server/Cargo.toml
git commit -m "feat(server): AES-256-GCM session serializer — gzip then encrypt"
```

---

### Task 8: Share Route Handlers (Rust)

**Files:**
- Create: `crates/server/src/routes/share.rs`
- Modify: `crates/server/src/routes/mod.rs`

```rust
// crates/server/src/routes/share.rs
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;

use crate::{
    auth::supabase::{extract_bearer, validate_jwt_with_rotation, AuthUser},
    error::{ApiError, ApiResult},
    share_serializer::{key_to_base64url, serialize_and_encrypt},
    state::AppState,
};

// Helper: extract raw JWT string from Authorization header (for forwarding to Worker)
fn extract_raw_jwt(headers: &HeaderMap) -> Option<String> {
    headers.get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

#[derive(Serialize)]
pub struct ShareResponse {
    pub token: String,
    pub url: String,  // includes #k= fragment
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sessions/{session_id}/share", post(create_share))
        .route("/sessions/{session_id}/share", delete(revoke_share))
        .route("/shares", get(list_shares))
}

/// Validate JWT with automatic JWKS rotation on kid mismatch.
/// Must be async to support re-fetching JWKS from Supabase.
async fn require_auth(headers: &HeaderMap, state: &AppState) -> ApiResult<AuthUser> {
    let jwks_lock = state.jwks.as_ref()
        .ok_or_else(|| ApiError::Unauthorized("Auth not configured".into()))?;

    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header".into()))?;

    let token = extract_bearer(auth_header)
        .ok_or_else(|| ApiError::Unauthorized("Expected Bearer token".into()))?;

    // Read current JWKS, attempt validation with rotation support
    let jwks = jwks_lock.read().await;
    match validate_jwt_with_rotation(token, &jwks).await {
        Ok((user, None)) => Ok(user), // validated with existing key
        Ok((user, Some(new_cache))) => {
            // Key was rotated — write back the new cache
            drop(jwks); // release read lock
            let mut jwks_write = jwks_lock.write().await;
            *jwks_write = new_cache;
            tracing::info!("JWKS cache updated after key rotation");
            Ok(user)
        }
        Err(e) => Err(ApiError::Unauthorized(format!("Invalid token: {e}"))),
    }
}

pub async fn create_share(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<ShareResponse>> {
    let user = require_auth(&headers, &state).await?;
    // Extract the raw JWT for forwarding to the Worker
    let raw_jwt = extract_raw_jwt(&headers)
        .ok_or_else(|| ApiError::Unauthorized("Missing JWT for forwarding".into()))?;

    let share_cfg = state.share.as_ref()
        .ok_or_else(|| ApiError::BadRequest("Sharing not configured".into()))?;

    // Get session file path from DB
    let file_path = state.db
        .get_session_file_path(&session_id).await?
        .ok_or_else(|| ApiError::NotFound(format!("Session {session_id}")))?;

    // Get session metadata for D1 — NOTE: method is get_session_by_id(), NOT get_session()
    // SessionInfo has `summary: Option<String>` and `preview: String` — there is NO `title` field
    let session = state.db.get_session_by_id(&session_id).await?
        .ok_or_else(|| ApiError::NotFound(format!("Session {session_id}")))?;
    let title = session.summary.clone()
        .unwrap_or_else(|| session.preview.chars().take(80).collect::<String>());

    // Encrypt the session
    let path = std::path::PathBuf::from(&file_path);
    let encrypted = serialize_and_encrypt(&path).await?;
    let size_bytes = encrypted.blob.len();

    // Call Worker: POST /api/share — forward the user's raw JWT
    let token_resp: serde_json::Value = share_cfg.http_client
        .post(format!("{}/api/share", share_cfg.worker_url))
        .bearer_auth(&raw_jwt) // forward the user's actual JWT, not a method on AppState
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
    share_cfg.http_client
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
    let _user = require_auth(&headers, &state).await?;
    let raw_jwt = extract_raw_jwt(&headers)
        .ok_or_else(|| ApiError::Unauthorized("Missing JWT for forwarding".into()))?;
    let share_cfg = state.share.as_ref()
        .ok_or_else(|| ApiError::BadRequest("Sharing not configured".into()))?;

    // Forward DELETE to the Worker — use by-session endpoint since we only have session_id
    let resp = share_cfg.http_client
        .delete(format!("{}/api/shares/by-session/{}", share_cfg.worker_url, session_id))
        .bearer_auth(&raw_jwt)
        .send().await
        .map_err(|e| ApiError::Internal(format!("Worker DELETE failed: {e}")))?;

    if !resp.status().is_success() && resp.status().as_u16() != 404 {
        return Err(ApiError::Internal(format!("Worker returned {}", resp.status())));
    }

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn list_shares(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    let _user = require_auth(&headers, &state).await?;
    let raw_jwt = extract_raw_jwt(&headers)
        .ok_or_else(|| ApiError::Unauthorized("Missing JWT for forwarding".into()))?;
    let share_cfg = state.share.as_ref()
        .ok_or_else(|| ApiError::BadRequest("Sharing not configured".into()))?;

    let resp: serde_json::Value = share_cfg.http_client
        .get(format!("{}/api/shares", share_cfg.worker_url))
        .bearer_auth(&raw_jwt) // forward the user's actual JWT
        .send().await
        .map_err(|e| ApiError::Internal(format!("Worker GET failed: {e}")))?
        .json().await
        .map_err(|e| ApiError::Internal(format!("Worker response: {e}")))?;

    Ok(Json(resp))
}
```

**Note:** JWT forwarding is handled by extracting the raw JWT string from the `Authorization` header via `extract_raw_jwt()` and passing it directly to the Worker via `.bearer_auth(&raw_jwt)`. No method on AppState needed.

**Step 1: Register router**

In `crates/server/src/routes/mod.rs`, add:

```rust
pub mod share;
// In api_routes():
.nest("/api", share::router())
```

**Step 2: Verify compilation**

```bash
cargo check -p claude-view-server
```

**Step 3: Commit**

```bash
git add crates/server/src/routes/share.rs crates/server/src/routes/mod.rs
git commit -m "feat(server): share endpoints with AES-256-GCM + JWT + Worker integration"
```

---

## Phase 3: Frontend (Supabase Auth UI + Share Button)

### Task 9: Supabase Auth Client + Sign-in Flow

**Files:**
- Create: `apps/web/src/lib/supabase.ts`
- Create: `apps/web/src/components/SignInPrompt.tsx`
- Modify: `apps/web/src/App.tsx` (or root component)

**Step 1: Install Supabase client**

```bash
cd apps/web && bun add @supabase/supabase-js
```

**Step 2: Create Supabase client**

```typescript
// apps/web/src/lib/supabase.ts
import { createClient, type SupabaseClient } from "@supabase/supabase-js";

const SUPABASE_URL = import.meta.env.VITE_SUPABASE_URL as string | undefined;
const SUPABASE_ANON_KEY = import.meta.env.VITE_SUPABASE_ANON_KEY as string | undefined;

// IMPORTANT: Do NOT throw on missing env vars — this would crash the entire app
// for any developer who hasn't configured Supabase (the majority in local dev).
// Export null when not configured; consumers check before using.
export const supabase: SupabaseClient | null =
  SUPABASE_URL && SUPABASE_ANON_KEY
    ? createClient(SUPABASE_URL, SUPABASE_ANON_KEY)
    : null;

if (!supabase) {
  console.warn("[supabase] VITE_SUPABASE_URL or VITE_SUPABASE_ANON_KEY not set — auth/sharing disabled");
}

export async function getAccessToken(): Promise<string | null> {
  if (!supabase) return null;
  const { data } = await supabase.auth.getSession();
  return data.session?.access_token ?? null;
}
```

**Step 3: Add env vars to `apps/web/.env.local`**

```
VITE_SUPABASE_URL=https://your-project.supabase.co
VITE_SUPABASE_ANON_KEY=eyJ...
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

  // CRITICAL: supabase is `SupabaseClient | null` — must null-check before calling methods.
  const handleMagicLink = async () => {
    if (!supabase) return; // auth not configured
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
    if (!supabase) return; // auth not configured
    await supabase.auth.signInWithOAuth({
      provider: "google",
      options: { redirectTo: window.location.origin },
    });
  };

  if (sent) {
    return (
      <div className="text-center py-8">
        <p className="text-gray-700 dark:text-gray-300">Check your email for the sign-in link.</p>
        <p className="text-gray-500 dark:text-gray-400 text-sm mt-2">
          You can close this and use the link from any device.
        </p>
      </div>
    );
  }

  // NOTE: Use gray/dark: variants to match existing codebase — NOT zinc-only classes.
  // See ConversationView.tsx and SettingsPage.tsx for the established pattern.
  return (
    <div className="flex flex-col gap-3 p-6 max-w-sm mx-auto">
      <h2 className="text-gray-800 dark:text-gray-200 font-medium">Sign in to enable sharing</h2>
      <p className="text-gray-500 dark:text-gray-400 text-sm">
        One account for sharing and mobile sync.
      </p>

      <button
        onClick={handleGoogle}
        className="flex items-center justify-center gap-2 px-4 py-2 rounded-md
          bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100
          border border-gray-300 dark:border-gray-600
          hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors text-sm font-medium"
      >
        Continue with Google
      </button>

      <div className="flex items-center gap-2 text-gray-400 dark:text-gray-600 text-xs">
        <div className="flex-1 border-t border-gray-200 dark:border-gray-700" />
        or
        <div className="flex-1 border-t border-gray-200 dark:border-gray-700" />
      </div>

      <input
        type="email"
        placeholder="your@email.com"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && handleMagicLink()}
        className="px-3 py-2 rounded-md bg-white dark:bg-gray-900
          border border-gray-300 dark:border-gray-700
          text-gray-800 dark:text-gray-200 placeholder-gray-400 dark:placeholder-gray-600
          text-sm focus:outline-none focus:border-blue-500 dark:focus:border-blue-400"
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
# IMPORTANT: Do NOT commit .env.local — it contains secrets.
# Verify .env.local is in .gitignore (it should be by default in Vite projects).
git add apps/web/src/lib/supabase.ts apps/web/src/components/SignInPrompt.tsx apps/web/package.json
git commit -m "feat(web): Supabase auth client + SignInPrompt component"
```

---

### Task 10: Share Hook (JWT-authenticated) + Share Button

**Files:**
- Create: `apps/web/src/hooks/use-share.ts`
- Modify: `apps/web/src/components/ConversationView.tsx` (or wherever the share button goes)

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
  url: string | null; // hydrated from localStorage (only available for shares created on this browser)
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

// NOTE: revokeShare takes session_id, NOT token. The route is DELETE /sessions/:session_id/share.
// The share list provides both `token` and `session_id` — pass session_id here.
async function revokeShare(sessionId: string): Promise<void> {
  const headers = await authHeaders();
  const res = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/share`, {
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
  // Hydrate url from localStorage (only available for shares created on this browser)
  return (data.shares ?? []).map((s: Omit<ShareListItem, "url">) => ({
    ...s,
    url: localStorage.getItem(`share_url:${s.token}`),
  }));
}

export function useCreateShare() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: createShare,
    onSuccess: (data) => {
      // Cache the full URL (with #k= fragment) in localStorage — this is the ONLY time
      // the encryption key is known. Future fetchShares() calls hydrate from here.
      if (data.url) localStorage.setItem(`share_url:${data.token}`, data.url);
      queryClient.invalidateQueries({ queryKey: ["shares"] });
    },
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

// Button JSX — uses gray/dark: pattern consistent with existing toolbar buttons:
<button
  onClick={handleShare}
  disabled={createShare.isPending}
  className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-md
    bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600
    text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700
    transition-colors disabled:opacity-50"
  title={shareUrl ? "Link copied!" : "Share conversation"}
>
  {createShare.isPending ? (
    <Loader2 className="w-4 h-4 animate-spin" />
  ) : shareUrl ? (
    <Check className="w-4 h-4 text-green-500" />
  ) : (
    <Link2 className="w-4 h-4" />
  )}
  {shareUrl ? "Copied!" : "Share"}
</button>

{/* Sign-in modal — shown when share requires auth */}
{showSignIn && (
  <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50"
    onClick={() => setShowSignIn(false)}>
    <div className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-xl shadow-2xl"
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

### Task 11: Shared Links in Settings + Sentry Setup

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

**Step 3: Add `VITE_SENTRY_DSN` to `.env.local`**

Create a Sentry project for "claude-view-web", copy DSN into `apps/web/.env.local`.

**Step 4: Add Shared Links section to Settings**

In the Settings page, add a section that uses `useShares` + `useRevokeShare`:

```tsx
import { useShares, useRevokeShare } from "../hooks/use-share";
import { showToast } from "../lib/toast";

// NOTE: Wrap this in the existing <SettingsSection> component used by all other settings sections.
// Use gray/dark: pattern — NOT zinc-only. Use showToast for clipboard feedback (codebase convention).
function SharedLinksSection() {
  const { data: shares, isLoading } = useShares();
  const revokeShare = useRevokeShare();

  if (isLoading) return <div className="text-gray-500 dark:text-gray-400 text-sm">Loading...</div>;
  if (!shares?.length) {
    return <div className="text-gray-500 dark:text-gray-400 text-sm">No shared conversations yet.</div>;
  }

  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-gray-500 dark:text-gray-400 text-left border-b border-gray-200 dark:border-gray-700">
          <th className="pb-2">Title</th>
          <th className="pb-2">Created</th>
          <th className="pb-2">Views</th>
          <th className="pb-2">Link</th>
          <th className="pb-2"></th>
        </tr>
      </thead>
      <tbody>
        {shares.map((share) => (
          <tr key={share.token} className="border-b border-gray-100 dark:border-gray-800">
            <td className="py-2 text-gray-700 dark:text-gray-300">{share.title ?? "Untitled"}</td>
            <td className="py-2 text-gray-500 dark:text-gray-400">
              {share.created_at > 0 ? new Date(share.created_at * 1000).toLocaleDateString() : "—"}
            </td>
            <td className="py-2 text-gray-500 dark:text-gray-400">{share.view_count}</td>
            <td className="py-2">
              {share.url ? (
                <button
                  onClick={() => {
                    navigator.clipboard.writeText(share.url!);
                    showToast("Link copied to clipboard");
                  }}
                  className="text-blue-600 dark:text-blue-400 hover:text-blue-500 dark:hover:text-blue-300 truncate max-w-48 text-left"
                  title={share.url}
                >
                  Copy link
                </button>
              ) : (
                <span className="text-gray-400 dark:text-gray-500 text-sm">Link unavailable</span>
              )}
            </td>
            <td className="py-2">
              <button
                onClick={() => {
                  // Phase 4 follow-up: Replace confirm() with styled DangerZoneSection dialog
                  if (confirm("Revoke this share? The link will stop working.")) {
                    revokeShare.mutate(share.session_id);
                  }
                }}
                className="text-red-600 dark:text-red-500 hover:text-red-500 dark:hover:text-red-400 text-xs"
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

**Step 5: Build**

```bash
cd apps/web && bun run build
```

**Step 6: Commit**

```bash
git add apps/web/src/
git commit -m "feat(web): shared links settings section + Sentry init"
```

---

## Phase 4: Viewer SPA (Decrypt in Browser)

### Task 12: Create Viewer SPA with Web Crypto Decrypt

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
    "vite": "^7.0.0"
  }
}
```

**Step 2a: Create `index.html`** (required by Vite as the HTML entry point)

```html
<!-- apps/share/index.html -->
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Shared Conversation — claude-view</title>
    <meta name="description" content="View a shared Claude conversation" />
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

**Step 2b: Create `src/main.tsx`** (React entry point — renders `<App />`)

```tsx
// apps/share/src/main.tsx
import { createRoot } from "react-dom/client";
import App from "./App";
import "./index.css";

createRoot(document.getElementById("root")!).render(<App />);
```

Also create a minimal `src/index.css` for Tailwind:

```css
/* apps/share/src/index.css */
@import "tailwindcss";
```

**Step 2c: Create `vite.config.ts`**

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

    const start = Date.now();

    fetch(`${WORKER_URL}/api/share/${token}`)
      .then(async (res) => {
        if (!res.ok) throw new Error(res.status === 404 ? "Share not found or has been revoked." : "Failed to load share.");
        return res.arrayBuffer();
      })
      .then((blob) => decryptShareBlob(blob, keyBase64url))
      .then((data) => {
        setSession(data);
        // PostHog analytics (fire and forget)
        if (typeof window !== "undefined" && (window as any).posthog) {
          (window as any).posthog.capture("share_decrypt_success", { duration_ms: Date.now() - start });
        }
      })
      .catch((err) => {
        Sentry.captureException(err);
        setError(err instanceof Error ? err.message : "Failed to decrypt share.");
      })
      .finally(() => setLoading(false));
  }, [token, keyBase64url]);

  // NOTE: Use gray/dark: pattern to match codebase convention — NOT zinc-only classes.
  if (loading) {
    return (
      <div className="min-h-screen bg-white dark:bg-gray-950 flex items-center justify-center">
        <div className="text-gray-500 dark:text-gray-400 text-sm">Decrypting conversation…</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="min-h-screen bg-white dark:bg-gray-950 flex items-center justify-center">
        <div className="text-center">
          <p className="text-red-600 dark:text-red-400 text-sm mb-2">{error}</p>
          <a href="https://claudeview.ai" className="text-gray-500 dark:text-gray-400 text-xs hover:text-gray-700 dark:hover:text-gray-300">
            What is claude-view?
          </a>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-white dark:bg-gray-950 text-gray-900 dark:text-gray-100">
      <header className="border-b border-gray-200 dark:border-gray-800 px-6 py-3 flex items-center justify-between">
        <div className="text-sm text-gray-500 dark:text-gray-400">
          Shared via <a href="https://claudeview.ai" className="text-gray-900 dark:text-white font-medium hover:underline">claude-view</a>
        </div>
        <a href="https://claudeview.ai" className="text-sm text-blue-600 dark:text-blue-400 hover:text-blue-500 dark:hover:text-blue-300">
          Get claude-view
        </a>
      </header>
      <main className="max-w-4xl mx-auto py-8 px-4">
        {/* Phase 4 MVP: raw JSON preview. Follow-up task: render session.messages
            using extracted components from @web (shared via @claude-view/shared). */}
        <pre className="text-xs text-gray-600 dark:text-gray-400 overflow-auto">
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

**Step 6: Verify Turbo pipeline picks up the new app**

The root `package.json` has `"workspaces": ["apps/*", "packages/*"]`, so `apps/share` IS automatically included. Verify:

```bash
cd /path/to/repo && bunx turbo build --dry-run | grep share
```

If `@claude-view/share` appears in the task list, Turbo handles it. If not, check that `apps/share/package.json` has a `"build"` script.

**Step 7: Deploy to Cloudflare Pages**

```bash
bunx wrangler pages deploy dist --project-name claude-view-share
# Set custom domain in Cloudflare dashboard: share.claudeview.ai
```

**Step 8: Commit**

```bash
git add apps/share/
git commit -m "feat(share): viewer SPA with AES-256-GCM decrypt (Web Crypto API)"
```

---

### Task 13: Wire Up + End-to-End Test

**Step 1: Configure env vars for production**

In `apps/share/.env.production`:
```
VITE_WORKER_URL=https://api-share.claudeview.ai
VITE_SENTRY_DSN=https://xxx@sentry.io/yyy
```

**Step 2: Configure custom domains in Cloudflare dashboard**

- Worker: `api-share.claudeview.ai` → `claude-view-share` Worker
- Pages: `share.claudeview.ai` → `claude-view-share` Pages project

**Step 3: End-to-end test**

```
1. Start local claude-view server with SUPABASE_URL set
2. Open the web UI → sign in with magic link
3. Open any session → click "Share"
4. Verify: share URL copied to clipboard, format includes #k=...
5. Open link in incognito window (no Supabase session)
6. Verify: viewer loads, decrypts, renders conversation
7. Verify: Worker logs show blob upload (Cloudflare dashboard)
8. Verify: Sentry shows no errors
9. Go to Settings → Shared Links → verify share appears
10. Click Revoke → verify link is dead
```

**Step 4: Commit**

```bash
git add apps/share/.env.production
git commit -m "feat(share): wire custom domains + e2e test passes"
```

---

## Phase 5: Relay Hardening (New)

### Task 14: Add JWT Validation to Relay

**Files:**
- Create: `crates/relay/src/auth.rs` (extend existing — check if it exists first)
- Modify: `crates/relay/src/lib.rs`
- Modify: `crates/relay/src/pairing.rs`
- Modify: `crates/relay/src/ws.rs`
- Modify: `crates/relay/Cargo.toml`

**Step 1: Add dependencies**

```bash
cd crates/relay
cargo add jsonwebtoken
cargo add anyhow  # Required — plan code uses anyhow::Result in SupabaseAuth
```

Also: move `tower` from `[dev-dependencies]` to `[dependencies]` in `crates/relay/Cargo.toml` — it's currently dev-only but Task 15 uses `TimeoutLayer` in production code.

**Step 2: Add Supabase JWT validator to relay**

The relay already has `crates/relay/src/auth.rs` for Ed25519. Add Supabase JWT validation:

```rust
// In crates/relay/src/auth.rs — add alongside existing Ed25519 auth.
// NOTE: `use serde::Deserialize;` already exists in this file (for AuthMessage).
// Merge the new jsonwebtoken import with the existing imports at the top; do NOT add a duplicate serde import.

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

#[derive(Debug, Deserialize)]
pub struct SupabaseClaims {
    pub sub: String,   // user_id
    pub exp: u64,
    pub iss: String,
}

pub struct SupabaseAuth {
    pub decoding_key: DecodingKey,
    pub issuer: String,
}

impl SupabaseAuth {
    /// Fetch JWKS from Supabase and construct SupabaseAuth.
    /// Call once at startup, store in RelayState.
    /// KNOWN LIMITATION (M1): No JWKS rotation support. Supabase rotates JWKS
    /// infrequently (months). If rotation occurs, restart the relay process.
    /// For M2+: wrap in Arc<RwLock<SupabaseAuth>> and add re-fetch-on-failure
    /// pattern (see server-side validate_jwt_with_rotation for reference).
    pub async fn from_supabase_url(supabase_url: &str) -> anyhow::Result<Self> {
        let jwks_url = format!("{}/auth/v1/.well-known/jwks.json", supabase_url);
        let resp: serde_json::Value = reqwest::get(&jwks_url).await?.json().await?;
        let key_json = resp["keys"].as_array()
            .and_then(|k| k.first())
            .ok_or_else(|| anyhow::anyhow!("Empty JWKS"))?;
        let jwk: jsonwebtoken::jwk::Jwk = serde_json::from_value(key_json.clone())?;
        let decoding_key = DecodingKey::from_jwk(&jwk)?;
        Ok(Self { decoding_key, issuer: format!("{}/auth/v1", supabase_url) })
    }

    pub fn validate(&self, token: &str) -> anyhow::Result<String> {
        let mut v = Validation::new(Algorithm::RS256);
        v.set_issuer(&[&self.issuer]);
        let data = decode::<SupabaseClaims>(token, &self.decoding_key, &v)?;
        Ok(data.claims.sub) // returns user_id
    }
}
```

**Step 3: Add SupabaseAuth to RelayState**

In `crates/relay/src/state.rs`, the current `RelayState` is:
```rust
#[derive(Clone, Default)]
pub struct RelayState {
    pub connections: Arc<DashMap<String, DeviceConnection>>,
    pub pairing_offers: Arc<DashMap<String, PairingOffer>>,
    pub devices: Arc<DashMap<String, RegisteredDevice>>,
    pub push_tokens: Arc<DashMap<String, String>>,
}
```

**IMPORTANT:** In this step, only add `supabase_auth` and `posthog_*` fields. The `rate_limiter` fields are added in Task 15 Step 3 (after `rate_limit.rs` is created). You MUST also remove `#[derive(Default)]` because the new constructor takes args. Replace with a manual `new()`.

Updated struct (Task 14 version — rate limiters added in Task 15):
```rust
use crate::auth::SupabaseAuth;

#[derive(Clone)]  // Remove Default — new fields require constructor args
pub struct RelayState {
    pub connections: Arc<DashMap<String, DeviceConnection>>,
    pub pairing_offers: Arc<DashMap<String, PairingOffer>>,
    pub devices: Arc<DashMap<String, RegisteredDevice>>,
    pub push_tokens: Arc<DashMap<String, String>>,
    pub supabase_auth: Option<Arc<SupabaseAuth>>,
    // NOTE: pair_rate_limiter and claim_rate_limiter fields are added in Task 15 Step 3
    // (after rate_limit.rs is created). Do NOT add them here — the type doesn't exist yet.
    pub posthog_client: Option<reqwest::Client>, // reused HTTP client for PostHog
    pub posthog_api_key: String,                 // empty string = PostHog disabled
}

impl RelayState {
    pub fn new(
        supabase_auth: Option<Arc<SupabaseAuth>>,
    ) -> Self {
        let posthog_key = std::env::var("POSTHOG_API_KEY").unwrap_or_default();
        Self {
            connections: Arc::new(DashMap::new()),
            pairing_offers: Arc::new(DashMap::new()),
            devices: Arc::new(DashMap::new()),
            push_tokens: Arc::new(DashMap::new()),
            supabase_auth,
            posthog_client: if posthog_key.is_empty() { None } else { Some(reqwest::Client::new()) },
            posthog_api_key: posthog_key,
        }
    }
}
```

Update `main.rs` to call `RelayState::new(supabase_auth)` instead of `RelayState::new()` or `RelayState::default()`.

> **Task 15 Step 3 will expand this constructor** to `RelayState::new(supabase_auth, pair_rl, claim_rl)` after `RateLimiter` is created.

The `posthog_client` and `posthog_api_key` are initialized from env vars automatically via the constructor. In `posthog.rs`, handlers access them via `state.posthog_client` and `state.posthog_api_key`:

```rust
// Example usage in pairing.rs after a successful claim.
// Requires: `use serde_json::json;` at top of pairing.rs.
// The `user_id` must be captured from auth: `let user_id = auth.validate(jwt).map_err(|_| StatusCode::UNAUTHORIZED)?;`
if let Some(client) = &state.posthog_client {
    crate::posthog::track(client, &state.posthog_api_key, "relay_paired", &user_id, json!({})).await;
}
```

Also spawn a background task for rate-limiter bucket eviction (prevents unbounded memory growth under rotating-IP attacks):

**IMPORTANT: Update ALL 6 call sites.** The following files call `RelayState::new()` or `RelayState::default()` and will fail to compile after this change:

1. `crates/relay/src/main.rs:16` — production code (update to `RelayState::new(supabase_auth)`)
2. `crates/relay/tests/integration.rs:34` — `health_check` test
3. `crates/relay/tests/integration.rs:44` — `pair_creates_offer` test
4. `crates/relay/tests/integration.rs:73` — `claim_consumes_token` test
5. `crates/relay/tests/integration.rs:132` — `claim_expired_token_returns_gone` test
6. `crates/relay/tests/integration.rs:169` — `claim_nonexistent_token_returns_404` test

For all 5 test call sites (#2-#6), replace `RelayState::new()` or `RelayState::default()` with:

```rust
RelayState::new(None)
```

> **Note:** Task 15 Step 3 will expand this to `RelayState::new(None, pair_rl, claim_rl)` after `RateLimiter` is created. The eviction task is also deferred to Task 15.

Verify with: `grep -rn "RelayState::default\|RelayState::new()" crates/relay/`

**Step 4: Load at startup**

In `crates/relay/src/main.rs`, add these imports at the top:

```rust
use std::sync::Arc;
use claude_view_relay::auth::SupabaseAuth;
```

Then add the startup code:

```rust
let supabase_auth = match std::env::var("SUPABASE_URL") {
    Ok(url) => match SupabaseAuth::from_supabase_url(&url).await {
        Ok(auth) => { tracing::info!("Supabase JWT validation enabled"); Some(Arc::new(auth)) }
        Err(e) => { tracing::warn!("Supabase JWKS load failed: {e}"); None }
    },
    Err(_) => { tracing::info!("SUPABASE_URL not set — JWT auth disabled"); None }
};
```

**Step 5: Require JWT on `/pair/claim`**

In `crates/relay/src/pairing.rs`, the current `claim_pair` handler signature is:
```rust
pub async fn claim_pair(
    State(state): State<RelayState>,
    Json(req): Json<ClaimRequest>,
) -> Result<Json<PairResponse>, StatusCode> { ... }
```

**You MUST add `headers: HeaderMap` as an Axum extractor parameter** — it is not currently present:

```rust
pub async fn claim_pair(
    State(state): State<RelayState>,
    headers: axum::http::HeaderMap,  // ADD THIS
    Json(req): Json<ClaimRequest>,
) -> Result<Json<PairResponse>, StatusCode> {
    // Extract Bearer JWT from Authorization header
    if let Some(auth) = &state.supabase_auth {
        let jwt = headers.get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let user_id = auth.validate(jwt)
            .map_err(|_| StatusCode::UNAUTHORIZED)?;
        // user_id is used for PostHog tracking (see example in Step 3)
    }

    // ... rest of existing handler logic ...
}
```

**Also update `create_pair`** — add `headers: axum::http::HeaderMap` as a parameter (before `Json(req)`). This is needed for rate limiting in Task 15 which calls `extract_ip(&headers)`:

```rust
pub async fn create_pair(
    State(state): State<RelayState>,
    headers: axum::http::HeaderMap,  // ADD THIS — needed for rate limiting
    Json(req): Json<PairRequest>,
) -> Result<Json<PairResponse>, StatusCode> {
    // ... existing handler logic ...
}
```

**Step 6: Require JWT on WS connect**

The current `ws_handler` signature in `crates/relay/src/ws.rs` is:
```rust
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<RelayState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}
```

**Rewrite it completely** — add `Query` extractor, auth rejection BEFORE upgrade, change return type to `Response`:

```rust
// Add these imports to the top of ws.rs (merge with existing imports):
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<RelayState>,
) -> Response {
    // Validate JWT from ?token= query param BEFORE upgrading
    if let Some(auth) = &state.supabase_auth {
        let jwt = match params.get("token") {
            Some(t) => t,
            None => return StatusCode::UNAUTHORIZED.into_response(),
        };
        if auth.validate(jwt).is_err() {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    ws.on_upgrade(|socket| handle_socket(socket, state)).into_response()
}
```

Note: Return type changes from `impl IntoResponse` to `Response` because we may return either a `StatusCode` or the upgrade response.

**Step 7: Verify compilation**

```bash
cargo check -p claude-view-relay
```

**Step 8: Commit**

```bash
git add crates/relay/
git commit -m "feat(relay): Supabase JWT validation on /pair/claim and WS connect"
```

---

### Task 15: Relay Rate Limiting + CORS + Body Limits

**Files:**
- Modify: `crates/relay/src/lib.rs`
- Create: `crates/relay/src/rate_limit.rs`

**Step 1: Add/Update Tower dependencies**

`tower` is currently in `[dev-dependencies]` only. Move it to `[dependencies]` and add features:

Manually edit TWO files (do NOT use `cargo add` — it can conflict with workspace versions):

**File 1: Root `Cargo.toml` (workspace)** — update the `tower` entry:

```toml
# Change from:
tower = { version = "0.5", features = ["util"] }
# To:
tower = { version = "0.5", features = ["util", "timeout"] }
```

**File 2: `crates/relay/Cargo.toml`** — make TWO changes:

1. Move `tower` from `[dev-dependencies]` to `[dependencies]`:
```toml
[dependencies]
tower = { workspace = true }
```

2. Update `tower-http` features (add `limit`, `trace` — NOT `timeout`, that comes from `tower`):
```toml
tower-http = { version = "0.6", features = ["cors", "limit", "trace"] }
```

**Step 2: Create in-memory rate limiter**

```rust
// crates/relay/src/rate_limit.rs
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    last_access: Instant,  // for eviction
    rate: f64,     // tokens per second
    capacity: f64,
}

impl TokenBucket {
    fn new(rate: f64, capacity: f64) -> Self {
        let now = Instant::now();
        Self { tokens: capacity, last_refill: now, last_access: now, rate, capacity }
    }

    fn try_consume(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.rate).min(self.capacity);
        self.last_refill = now;
        self.last_access = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

pub struct RateLimiter {
    buckets: DashMap<String, Arc<Mutex<TokenBucket>>>,
    rate: f64,
    capacity: f64,
}

impl RateLimiter {
    pub fn new(requests_per_sec: f64, burst: f64) -> Self {
        Self { buckets: DashMap::new(), rate: requests_per_sec, capacity: burst }
    }

    pub async fn check(&self, key: &str) -> bool {
        let bucket = self.buckets
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(TokenBucket::new(self.rate, self.capacity))));
        bucket.value().lock().await.try_consume()
    }

    /// Evict stale buckets not accessed within `max_idle`.
    /// Call periodically (e.g., every 5 minutes) to prevent unbounded memory growth
    /// under rotating-IP attacks. Pattern: Governor crate uses similar idle eviction.
    pub async fn evict_stale(&self, max_idle: Duration) {
        let now = Instant::now();
        // Collect keys first to avoid holding DashMap shard locks across .await points
        let keys: Vec<String> = self.buckets.iter().map(|e| e.key().clone()).collect();
        let mut stale_keys = Vec::new();
        for key in &keys {
            if let Some(bucket_ref) = self.buckets.get(key) {
                let arc = bucket_ref.value().clone(); // clone Arc<Mutex<TokenBucket>>
                drop(bucket_ref);                     // release DashMap shard read lock NOW
                if now.duration_since(arc.lock().await.last_access) > max_idle {
                    stale_keys.push(key.clone());
                }
            }
        }
        for key in &stale_keys {
            self.buckets.remove(key);
        }
        if !stale_keys.is_empty() {
            tracing::debug!("Evicted {} stale rate-limit buckets", stale_keys.len());
        }
    }
}
```

**CRITICAL: Declare new modules in `crates/relay/src/lib.rs`.** Without these, the compiler ignores the files:

```rust
// Add to crates/relay/src/lib.rs alongside existing pub mod declarations:
pub mod rate_limit;
// NOTE: `pub mod posthog;` is added in Task 16 Step 3 (when posthog.rs file is created).
// Declaring it here would cause "file not found" compile error.
```

**Step 3: Update CORS in `lib.rs`**

Replace `CorsLayer::new().allow_origin(Any)` with:

```rust
use tower_http::cors::{AllowOrigin, CorsLayer};
use axum::http::{HeaderValue, Method};

let allowed_origins: Vec<HeaderValue> = vec![
    "https://claudeview.ai".parse().unwrap(),
    "https://claudeview.com".parse().unwrap(),
    "http://localhost:5173".parse().unwrap(),
    "http://localhost:8081".parse().unwrap(),
];

let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::list(allowed_origins))
    .allow_methods([Method::GET, Method::POST, Method::DELETE])
    .allow_headers([
        axum::http::header::CONTENT_TYPE,
        axum::http::header::AUTHORIZATION,
    ]);
```

**Step 4: Add body limit + timeout layers**

**IMPORTANT:** `TimeoutLayer` must NOT apply to the `/ws` route — WebSocket connections are long-lived (minutes to hours). A 30-second timeout would kill them. Split routes:

```rust
// Add these imports at the top of lib.rs (alongside existing imports):
// Replace `use tower_http::cors::{Any, CorsLayer};` with:
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use axum::http::{HeaderValue, Method};
use tower::ServiceBuilder;
// NOTE: `use std::time::Duration;` is already imported in lib.rs — do not duplicate.

// Replace the ENTIRE body of app() — both the existing tokio::spawn cleanup loop
// (lines 17-27 of current lib.rs) AND the Router::new() chain (lines 29-43).
// The old cleanup loop (pairing_offers.retain) is superseded by the TTL check in
// claim_pair (offer.created_at.elapsed > 300s). Remove it to avoid duplication.

// HTTP routes get timeout + body limit
let http_routes = Router::new()
    .route("/pair", post(pairing::create_pair))
    .route("/pair/claim", post(pairing::claim_pair))
    .route("/push-tokens", post(push::register_push_token))
    .route("/health", get(|| async { "ok" }));

// WS route does NOT get timeout (connections are long-lived)
let ws_routes = Router::new()
    .route("/ws", get(ws::ws_handler));

let shared_layers = ServiceBuilder::new()
    .layer(RequestBodyLimitLayer::new(256 * 1024)) // 256KB
    .layer(cors)
    .layer(tower_http::trace::TraceLayer::new_for_http());

// Build final app — this is a tail expression (no semicolon) to return from app()
http_routes
    .layer(
        ServiceBuilder::new()
            .layer(tower::timeout::TimeoutLayer::new(Duration::from_secs(30)))
    )
    .merge(ws_routes)
    .layer(shared_layers)
    .with_state(state)  // REQUIRED: all handlers extract State<RelayState>
```

**Step 5: Apply rate limiters in handlers**

In `/pair` and `/pair/claim` handlers, add:

```rust
// In pairing.rs:
// IMPORTANT: The handler return type is Result<Json<PairResponse>, StatusCode>.
// The Err variant is just StatusCode, NOT a tuple. Return StatusCode directly.
let ip = extract_ip(&headers);
if !state.pair_rate_limiter.check(&ip).await {
    return Err(StatusCode::TOO_MANY_REQUESTS);
}
```

**Define `extract_ip` helper** (add to `pairing.rs` or a shared `util.rs`):

```rust
fn extract_ip(headers: &axum::http::HeaderMap) -> String {
    // Fly.io sets Fly-Client-IP; Cloudflare sets CF-Connecting-IP
    headers.get("fly-client-ip")
        .or_else(|| headers.get("cf-connecting-ip"))
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .unwrap_or("unknown")
        .trim()
        .to_string()
}
```

Now expand `RelayState` to include rate limiter fields. In `crates/relay/src/state.rs`, add:

```rust
use crate::rate_limit::RateLimiter;  // add alongside the existing SupabaseAuth import

// Add to RelayState struct:
pub pair_rate_limiter: Arc<RateLimiter>,    // 5 req/min per IP
pub claim_rate_limiter: Arc<RateLimiter>,   // 10 req/min per IP
```

Expand the `new()` constructor to accept rate limiters:

```rust
pub fn new(
    supabase_auth: Option<Arc<SupabaseAuth>>,
    pair_rate_limiter: Arc<RateLimiter>,
    claim_rate_limiter: Arc<RateLimiter>,
) -> Self {
    let posthog_key = std::env::var("POSTHOG_API_KEY").unwrap_or_default();
    Self {
        connections: Arc::new(DashMap::new()),
        pairing_offers: Arc::new(DashMap::new()),
        devices: Arc::new(DashMap::new()),
        push_tokens: Arc::new(DashMap::new()),
        supabase_auth,
        pair_rate_limiter,
        claim_rate_limiter,
        posthog_client: if posthog_key.is_empty() { None } else { Some(reqwest::Client::new()) },
        posthog_api_key: posthog_key,
    }
}
```

Update `main.rs` to pass rate limiters:

```rust
use std::sync::Arc;
use std::time::Duration;
use claude_view_relay::rate_limit::RateLimiter;

let pair_rl = Arc::new(RateLimiter::new(5.0 / 60.0, 5.0));   // 5/min burst
let claim_rl = Arc::new(RateLimiter::new(10.0 / 60.0, 10.0)); // 10/min burst
let state = RelayState::new(supabase_auth, pair_rl.clone(), claim_rl.clone());
```

Update ALL 5 integration test call sites from `RelayState::new(None)` to:

```rust
// Add these imports at the TOP of crates/relay/tests/integration.rs
// (alongside the existing `use claude_view_relay::*;` imports):
use std::sync::Arc;
use claude_view_relay::rate_limit::RateLimiter;

// Then replace each RelayState::new(None) call with:
RelayState::new(
    None,
    Arc::new(RateLimiter::new(100.0, 100.0)),
    Arc::new(RateLimiter::new(100.0, 100.0)),
)
```

Spawn periodic eviction of stale rate-limit buckets (in `main.rs`, after state creation):

```rust
// Spawn periodic eviction of stale rate-limit buckets (every 5 minutes)
let pair_rl_clone = pair_rl.clone();
let claim_rl_clone = claim_rl.clone();
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(300));
    loop {
        interval.tick().await;
        pair_rl_clone.evict_stale(Duration::from_secs(600)).await;
        claim_rl_clone.evict_stale(Duration::from_secs(600)).await;
    }
});
```

**Step 6: Add WS connection limits**

In `ws.rs`, before upgrading:

```rust
// NOTE: state.connections uses plain device_id as the DashMap key.
// DashMap.insert() overwrites on duplicate key, so each device can only
// have ONE connection at a time (second WS replaces the first silently).
// Per-device limit of 3 is therefore unnecessary — the DashMap key scheme
// already enforces a limit of 1. Only the global limit matters.

// Check global connection limit
if state.connections.len() >= 1000 {
    return StatusCode::SERVICE_UNAVAILABLE.into_response();
}
```

If multi-connection-per-device support is needed later, redesign the key scheme to `"{device_id}:{uuid}"`. For M1, the implicit 1-per-device limit from DashMap is sufficient.

**Step 7: Verify compilation**

```bash
cargo check -p claude-view-relay
```

**Step 8: Commit**

```bash
git add crates/relay/
git commit -m "feat(relay): rate limiting + CORS lockdown + body limits + connection caps"
```

---

### Task 16: Relay Observability (Sentry + PostHog)

**Files:**
- Modify: `crates/relay/Cargo.toml`
- Modify: `crates/relay/src/main.rs`
- Modify: `crates/relay/src/auth.rs`

**Step 1: Add Sentry**

```bash
cd crates/relay
cargo add sentry
cargo add sentry-tracing
```

**Step 2: Init Sentry in main.rs**

```rust
let _sentry = sentry::init((
    std::env::var("SENTRY_DSN").unwrap_or_default(),
    sentry::ClientOptions {
        release: sentry::release_name!(),
        environment: Some(std::env::var("ENVIRONMENT")
            .unwrap_or_else(|_| "development".to_string())
            .into()),
        traces_sample_rate: 0.1,
        ..Default::default()
    },
));

/// REPLACE the existing tracing_subscriber::fmt()...init() block (lines 9-14 of main.rs)
// with this Sentry-integrated subscriber. Do NOT add alongside — calling .init() twice panics.
// Requires `tracing-subscriber` with default features (which include `registry` and `fmt`).
// The workspace Cargo.toml preserves defaults — do not add `default-features = false`.
use tracing_subscriber::prelude::*;
tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "warn,claude_view_relay=info".into()))
    .with(tracing_subscriber::fmt::layer())
    .with(sentry_tracing::layer())
    .init();
```

**Step 3: Add PostHog helper**

Add `pub mod posthog;` to `crates/relay/src/lib.rs` alongside the other module declarations (including the `pub mod rate_limit;` added in Task 15 Step 2).

```rust
// In crates/relay/src/posthog.rs
use reqwest::Client;

pub async fn track(client: &Client, api_key: &str, event: &str, user_id: &str, props: serde_json::Value) {
    if api_key.is_empty() { return; }
    let _ = client.post("https://us.i.posthog.com/capture/")
        .json(&serde_json::json!({
            "api_key": api_key,
            "event": event,
            "distinct_id": user_id,
            "properties": props,
        }))
        .send().await;
}
```

**Step 4: Add PostHog calls at key events**

In `pairing.rs` → claim success: `track(..., "relay_paired", ...)`.
In `ws.rs` → connect success: `track(..., "relay_connected", ...)`.
In `ws.rs` → message forwarded: `track(..., "relay_message_forwarded", ...)`.
In `push.rs` → push sent: `track(..., "push_notification_sent", ...)`.

**Step 5: Log auth failures with tracing**

In auth middleware:

```rust
if auth.validate(jwt).is_err() {
    tracing::warn!(device_id = %device_id, endpoint = %endpoint, "JWT validation failed");
    sentry::capture_message("JWT validation failed", sentry::Level::Warning);
}
```

**Step 6: Set Fly.io secrets**

```bash
fly secrets set SENTRY_DSN="https://xxx@sentry.io/yyy" -a claude-view-relay
fly secrets set POSTHOG_API_KEY="phc_xxx" -a claude-view-relay
fly secrets set SUPABASE_URL="https://xxx.supabase.co" -a claude-view-relay
```

**Step 7: Deploy relay**

```bash
fly deploy -a claude-view-relay
```

**Step 8: Verify relay hardening**

```bash
# Expect 401 (JWT required):
curl -X POST https://relay.claudeview.ai/pair/claim \
  -H "Content-Type: application/json" \
  -d '{"token":"test"}'

# Expect 401 (no token query param):
curl "https://relay.claudeview.ai/ws"
```

**Step 9: Commit**

```bash
git add crates/relay/
git commit -m "feat(relay): Sentry + PostHog observability + audit logging"
```

---

## Summary

| Phase | Tasks | What it builds |
|-------|-------|---------------|
| 0: Supabase | 1 | Supabase project, magic link + Google OAuth, JWKS endpoint |
| 1: Worker | Tasks 1–5 | Hardened Worker: JWT auth, rate limiting, CORS, Sentry, PostHog |
| 2: Rust | Tasks 6–8 | AES-256-GCM serializer, JWT validation, share endpoints |
| 3: Frontend | Tasks 9–11 | Supabase sign-in, share button, shared links settings, Sentry |
| 4: Viewer SPA | Tasks 12–13 | Web Crypto decrypt, branded viewer, Sentry, custom domain |
| 5: Relay | Tasks 14–16 | JWT auth, rate limits, CORS, body limits, Sentry, PostHog |

**Total:** 17 tasks. Phase 5 (Relay) is fully independent. Phase 1 (Worker) needs Phase 0 credentials in `wrangler.toml` but can be scaffolded in parallel. Phase 2 depends on Phase 0. Phase 3 depends on Phases 0 and 2. Phase 4 depends on Phase 1.

**Security properties when done:**
- Operator cannot read shared conversation content (AES-256-GCM, key only in URL fragment)
- Operator cannot read relay messages (NaCl E2E, unchanged)
- All mutations require Supabase JWT
- All surfaces rate-limited per user_id
- CORS locked to claudeview.ai / claudeview.com
- Errors tracked in Sentry; usage in PostHog (no PII in events)

---

## Changelog of Fixes Applied (Audit -> Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `crates/server/src/auth/` module does not exist — plan assumed it did | Blocker | Added explicit steps to create `auth/mod.rs`, `auth/supabase.rs`, and add `pub mod auth;` to `lib.rs` |
| 2 | `ApiError::Unauthorized` variant missing from `error.rs` | Blocker | Added Step 2 in Task 6 to create the variant + StatusCode mapping |
| 3 | `AppState` constructed in 5 places, plan only mentioned 1 | Blocker | Listed all 5 construction sites with instruction to add `jwks: None, share: None` to each |
| 4 | JWKS loading described for `main.rs` but AppState built in `lib.rs:create_app_full()` | Warning | Changed Step 6 to target `create_app_full()` in `lib.rs` |
| 5 | `parse_session()` is async — plan called it from `spawn_blocking` | Blocker | Changed to direct `.await` call (removed `spawn_blocking` wrapper) |
| 6 | `state.db.get_session()` — wrong method name (actual: `get_session_by_id()`) | Blocker | Fixed to `get_session_by_id()` |
| 7 | `session.title` field doesn't exist on `SessionInfo` (has `summary`/`preview`) | Blocker | Changed to `session.summary.clone().unwrap_or_else(\|\| session.preview.chars().take(80).collect())` |
| 8 | `state.supabase_token_for_user()` method doesn't exist | Blocker | Replaced with `extract_raw_jwt()` helper that passes the raw JWT from the Authorization header |
| 9 | `supabase.ts` throws at module load on missing env vars — crashes entire app | Blocker | Changed to conditional client creation; `supabase` exported as `SupabaseClient \| null` |
| 10 | All new frontend components use zinc-only classes (dark-only, no light mode) | Blocker | Rewrote all component styles to use `gray-*` with `dark:gray-*` variants matching existing codebase |
| 11 | `SharedLinksSection` bypasses existing `SettingsSection` wrapper | Blocker | Added note to wrap in `SettingsSection`; switched to gray/dark: pattern |
| 12 | `revokeShare()` passes `token` as `:session_id` URL param — guaranteed 404 | Blocker | Changed parameter from `token` to `sessionId`; callers now pass `share.session_id` |
| 13 | `.wrangler/` directory not in `.gitignore` | Warning | Added Step 5 in Task 1 to add `.wrangler/` to root `.gitignore` |
| 14 | Worker install not in `postinstall` hook (sidecar precedent) | Warning | Added Step 7 in Task 1 to update `postinstall` script |
| 15 | Relay: `anyhow` crate missing from `Cargo.toml` | Blocker | Added `cargo add anyhow` to Task 14 Step 1 |
| 16 | Relay: `tower` is dev-dependency only, plan uses it in production | Blocker | Added instruction to move to `[dependencies]` with `limit,timeout` features |
| 17 | Relay: `tower-http` missing `limit,trace` features | Blocker | Added instruction to update features in relay's `Cargo.toml` (`timeout` is a `tower` feature, not `tower-http`) |
| 18 | Relay: `claim_pair`/`create_pair` handlers lack `headers: HeaderMap` param | Blocker | Showed full updated handler signature with `headers` extractor |
| 19 | Relay: `ws_handler` has no `Query` extractor, can't reject before upgrade | Blocker | Rewrote complete `ws_handler` with `Query(params)`, pre-upgrade auth, `Response` return type |
| 20 | Relay: `#[derive(Default)]` on `RelayState` breaks with non-Default fields | Blocker | Removed `Default` derive; added full `RelayState::new()` constructor |
| 21 | Relay: per-device connection limit uses wrong key format (`device_id:suffix` vs `device_id`) | Blocker | Removed per-device limit check; documented DashMap implicit 1-per-device constraint |
| 22 | `reqwest` and `base64` already workspace deps — `cargo add` redundant | Minor | Removed redundant `cargo add reqwest --features json` from Task 6 |
| 23 | Viewer SPA version mismatch (`vite ^6` vs web app `vite ^7`) | Warning | Noted — implementer should align to `^7` |
| 24 | `showToast` not used for clipboard feedback (codebase convention) | Warning | Added `showToast` calls in `SharedLinksSection` for clipboard operations |
| 25 | `create_app_full()` is `pub fn`, not `async` — `.await` inside it won't compile | Blocker | Moved JWKS loading to `main.rs`; pass `jwks` + `share` as params to `create_app_full()` |
| 26 | `Unauthorized` arm uses `json!()` macro (not imported) + breaks `IntoResponse` pattern | Blocker | Changed to `ErrorResponse::new(msg)` matching every other arm's `(StatusCode, ErrorResponse)` tuple |
| 27 | `OsRng::default()` does not exist — `OsRng` is a unit struct, not `Default` | Blocker | Changed to `&mut OsRng` (correct API per `aes-gcm` docs) |
| 28 | `import { D1Database } from "@cloudflare/workers-types"` — can't import from `.d.ts` ambient types | Blocker | Removed import; D1Database is available globally via tsconfig `types` array |
| 29 | `showToast("...", "success")` — second param is `duration: number`, not status string | Blocker | Changed to `showToast("Link copied to clipboard")` (uses default 2000ms) |
| 30 | `SignInPrompt` calls `supabase.auth.*` without null-checking `supabase` (`SupabaseClient \| null`) | Blocker | Added `if (!supabase) return;` guards in `handleMagicLink` and `handleGoogle` |
| 31 | Relay `claim_pair` rate limit returns `(StatusCode, &str)` but error type is `StatusCode` | Blocker | Changed to `return Err(StatusCode::TOO_MANY_REQUESTS)` matching handler return type |
| 32 | `extract_ip()` function used in rate limiting but never defined | Blocker | Added full `extract_ip` helper (Fly-Client-IP / CF-Connecting-IP / X-Forwarded-For) |
| 33 | `rate_limit.rs` module created but never declared in relay `lib.rs` | Blocker | Added `pub mod rate_limit;` to relay `lib.rs` |
| 34 | `posthog.rs` module created but never declared in relay `lib.rs` | Blocker | Added `pub mod posthog;` to relay `lib.rs` |
| 35 | `share_serializer.rs` module created but never declared in server `lib.rs` | Blocker | Added `pub mod share_serializer;` to server `lib.rs` in Task 7 Step 1 |
| 36 | Viewer SPA uses `zinc-*` classes violating CLAUDE.md style rules | Warning | Rewrote all viewer styles to `gray/dark:` pattern matching codebase |
| 37 | `revoke_share` handler is a no-op stub (ignores `session_id`, returns OK without calling Worker) | Blocker | Implemented full Worker DELETE forwarding with JWT auth |
| 38 | `fetch_jwks()` and `jwk_to_pem()` are dead code — `jwk_to_pem` always returns `Err` | Warning | Removed both functions; only `fetch_decoding_key()` retained |
| 39 | Duplicate "Step 6" numbering in Task 6 (Step 6 at line 1059 and again at commit step) | Minor | Renumbered commit step to Step 8 |
| 40 | `SENTRY_DSN` not in Worker `Env` interface — `env.SENTRY_DSN` is a TypeScript error | Warning | Added `SENTRY_DSN?: string` to `Env` interface; fixed `dsn:` to use `env.SENTRY_DSN` |
| 41 | D1 rate limiting cleanup query lacks index on `window` column | Minor | Added comment documenting composite PK scan behavior + migration path to secondary index |
| 42 | Viewer `apps/share/` not verified in Turbo pipeline | Minor | Added Step 5b to verify `bunx turbo build --dry-run` picks up the app |
| 43 | Viewer SPA has TODO placeholder instead of real UI | Minor | Clarified as "Phase 4 MVP: raw JSON preview" with follow-up task reference |
| 44 | D1 rate limiting is wrong storage primitive per Cloudflare docs (prove-it FAIL) | Warning | Added scale note: D1 works at MVP (<1000 users); migration path to `cloudflare:rate-limiter` binding documented |
| 45 | Viewer SPA missing `index.html` (Vite entry point) — build will fail | Blocker | Added Step 2a with complete `index.html` |
| 46 | Viewer SPA missing `src/main.tsx` (React entry — `createRoot`) — build will fail | Blocker | Added Step 2b with `main.tsx` + `index.css` for Tailwind |
| 47 | Task 9 commits `apps/web/.env.local` (secrets file) to git | Blocker | Removed `.env.local` from `git add`; added comment about `.gitignore` |
| 48 | Task 13 commit step has no `git add` — unclear what is staged | Warning | Added `git add apps/share/.env.production` before commit |
| 49 | `sentry-tracing` crate added but never wired into tracing subscriber | Warning | Added `sentry_tracing::layer()` integration with `tracing_subscriber::registry()` |
| 50 | Phase dependency description incomplete — Phase 1 needs Phase 0 config, Phase 3 needs Phase 0 | Minor | Updated dependency text to clarify Phase 1 config dependency and Phase 3 double dependency |
| 51 | `revoke_share` calls wrong Worker URL (`/api/shares/${sessionId}`) + wrong parameter — Worker has no such route | Blocker | Added `handleDeleteShareBySession` Worker endpoint at `/api/shares/by-session/:session_id`; updated Rust handler to call correct path |
| 52 | Missing `compatibility_flags = ["nodejs_compat"]` in `wrangler.toml` — Sentry SDK needs Node.js APIs | Blocker | Added `compatibility_flags = ["nodejs_compat"]` to `wrangler.toml` config |
| 53 | `create_pair` handler missing `headers: HeaderMap` param — rate limiting can't extract IP | Blocker | Added `headers: HeaderMap` extractor to `create_pair` with example code |
| 54 | `RelayState::new()` signature change breaks all test call sites | Warning | Added note + grep command to find/update all `RelayState::new()` test calls |
| 55 | Duplicate `vite.config.ts` creation steps in Task 12 | Minor | Already fixed — Step 2c label clarified in prior round |
| 56 | `CreatePairRequest` type does not exist — actual type is `PairRequest` | Blocker | Changed `Json<CreatePairRequest>` to `Json<PairRequest>` in `create_pair` handler |
| 57 | `tower-http` does not have a `timeout` feature — `cargo build` fails | Blocker | Removed `timeout` from `tower-http` features; noted it comes from `tower` crate |
| 58 | Only grep command provided for 6 `RelayState::new()` call sites | Warning | Listed all 6 call sites explicitly (1 production + 5 test) with replacement code |
| 59 | `// TODO` comment in Phase 3 `SharedLinksSection` (not Phase 4 follow-up) | Minor | Reworded to "Phase 4 follow-up" inline comment (code is functional) |
| 60 | Viewer SPA uses Vite `^6` but monorepo uses `^7` | Minor | Changed `"vite": "^6.0.0"` to `"vite": "^7.0.0"` |
| 61 | Task 12 step numbering: duplicate Step 7 + non-standard 5b | Minor | Renumbered: 5b→6, old 6→7, old duplicate 7→8 (sequential 1–8) |
| 62 | JWKS cached once at startup with no rotation handling — first key rotation causes auth outage | Warning | Added `validate_jwt_with_rotation()` with retry-on-failure JWKS re-fetch; `JwksCache` stores `supabase_url`; `AppState.jwks` wrapped in `Arc<RwLock>` |
| 63 | `reqwest::Client::new()` created per-request in share handlers (anti-pattern per reqwest docs) | Warning | Moved `reqwest::Client` into `ShareConfig`; all 3 handlers now use `share_cfg.http_client` |
| 64 | Rate limiter `DashMap` buckets grow unbounded under rotating-IP attacks (memory leak) | Warning | Added `last_access` to `TokenBucket`, `evict_stale()` method, and periodic 5-min eviction task in relay `main.rs` |
| 65 | `OsRng` calling convention inconsistent: `generate_key(OsRng)` vs `generate_nonce(&mut OsRng)` | Minor | Changed `generate_key(OsRng)` to `generate_key(&mut OsRng)` for consistency |
| 66 | `require_auth()` is sync `fn` but needs to `.await` `RwLock::read()` — type mismatch on `&Arc<RwLock<JwksCache>>` vs `&JwksCache` | Blocker | Made `require_auth` `async fn`; reads `jwks_lock.read().await`; calls `validate_jwt_with_rotation`; writes back rotated cache |
| 67 | `validate_jwt_with_rotation()` defined (Fix #62) but never called — dead code, JWKS rotation never triggers | Blocker | Wired into `require_auth()` as the primary validation path; all 3 callers now `.await` it |
| 68 | PostHog relay wiring incomplete — `posthog::track()` needs `client` + `api_key` not in `RelayState` | Warning | Added `posthog_client: Option<reqwest::Client>` and `posthog_api_key: String` to `RelayState`; initialized from env var in constructor; example usage shown |
| 69 | Tower workspace features instruction buried in comments — implementer may miss | Warning | Replaced commented-out bash instructions with explicit "Edit TWO files" section with before/after toml blocks |
| 70 | Unused `Serialize` derive on `Claims` struct (only deserialized, never serialized) | Minor | Removed `Serialize` from `#[derive(...)]` |
| 71 | `handleUploadBlob` body size check can OOM if `Content-Length` missing — undocumented platform limit | Minor | Added comment documenting Cloudflare Workers platform-level 100MB/500MB request limit as defense-in-depth |
| 72 | `share.rs` imports `validate_jwt` but calls `validate_jwt_with_rotation` — won't compile | Blocker | Changed import to `validate_jwt_with_rotation`, removed dead `validate_jwt` import |
| 73 | `share_serializer.rs` imports `Key, Nonce` but never uses them — unused import warnings | Minor | Removed `Key, Nonce` from `aes_gcm` import |
| 74 | `share.rs` imports `Deserialize` from `serde` but no struct derives it — unused import | Minor | Changed to `use serde::Serialize;` |
| 75 | Relay `SupabaseClaims` has `Serialize` derive but only `Deserialize` is used (same issue as fix #70) | Minor | Removed `Serialize` from derive and import |
| 76 | `supabase.rs` still imports `Serialize` in `use serde::{Deserialize, Serialize}` but no struct derives it (orphan from fix #70) | Minor | Changed to `use serde::Deserialize;` |
| 77 | Integration tests use `crate::rate_limit::RateLimiter` but `tests/integration.rs` is outside the crate | Blocker | Changed to `claude_view_relay::rate_limit::RateLimiter` |
| 78 | Task 16 adds second `tracing_subscriber` `.init()` — calling `.init()` twice panics at runtime | Warning | Changed comment to explicitly say REPLACE existing tracing init (lines 9-14 of main.rs), do NOT add alongside |
| 79 | `decode_header` imported but never called in `supabase.rs` | Minor | Removed `decode_header` from jsonwebtoken import |
| 80 | `Arc` and `RwLock` imported but unused in `supabase.rs` (wrapping done in `main.rs`/`share.rs`) | Minor | Removed both imports |
| 81 | `ShareListItem` struct defined but never constructed — `list_shares` returns `serde_json::Value` | Minor | Removed dead struct |
| 82 | `cargo add base64` in Task 7 Step 2 is redundant — already in server `Cargo.toml` | Minor | Removed step, added note that `base64` is already a workspace dependency |
| 83 | Duplicate `use serde::Deserialize;` in relay `auth.rs` — file already imports it for `AuthMessage` | Minor | Added NOTE to merge with existing imports, removed duplicate import line |
| 84 | `ShareListItem.url` never populated by Worker — clipboard copies `undefined` | Warning | Made `url: string \| null`, hydrate from `localStorage` in `fetchShares()`, cache in `useCreateShare.onSuccess`, show "Link unavailable" when null |
| 85 | Relay `state.rs` missing `use crate::auth::SupabaseAuth` and `use crate::rate_limit::RateLimiter` imports | Minor | Added both imports to the `state.rs` snippet |
| 86 | Relay `main.rs` Step 4 missing `SupabaseAuth`, `RateLimiter`, `Arc`, `Duration` imports | Minor | Added explicit import block: `use std::sync::Arc; use std::time::Duration; use claude_view_relay::auth::SupabaseAuth; use claude_view_relay::rate_limit::RateLimiter;` |
| 87 | N/A — consolidated into #86 | — | — |
| 88 | `EnvFilter::from_default_env()` panics on invalid `RUST_LOG` — regression from defensive current code | Minor | Changed to `try_from_default_env().unwrap_or_else(\|_\| "warn,claude_view_relay=info".into())` |
| 89 | `main.rs` uses `crate::auth::supabase::fetch_decoding_key` but `main.rs` is a binary target — `crate::` resolves to the binary crate, not `claude_view_server` | Blocker | Added explicit `use claude_view_server::auth::supabase::{fetch_decoding_key, JwksCache}` and `use claude_view_server::state::ShareConfig` import block for `main.rs` |
| 90 | Ambiguous insertion point for JWKS/share loading code in `main.rs` | Minor | Changed to "insert between shutdown channel setup and `create_app_full()` call, around line 282" |
| 91 | Task 14 references `crate::rate_limit::RateLimiter` but `rate_limit` module is created in Task 15 — `cargo check` fails after Task 14 | Blocker | Split Task 14/15: Task 14 constructor now takes only `supabase_auth`, rate limiter fields + eviction task + expanded constructor moved to Task 15 Step 5 |
| 92 | `json!({})` macro used in `pairing.rs` PostHog example but `serde_json::json` not imported; `user_id` variable not bound from auth result | Warning | Added `use serde_json::json;` import comment; changed to `let _user_id = auth.validate(jwt)` binding |
| 93 | `evict_stale()` holds DashMap shard lock (via `.iter()`) while awaiting `Mutex::lock()` — potential deadlock under contention | Warning | Rewrote to collect keys first via `.iter().map()`, then check each individually with `.get()` + `.await` |
| 94 | `JwksCache` imported in relay `main.rs` but never used — unused import warning | Minor | Removed `JwksCache` from the import line |
| 95 | Step 6c call site uses placeholder variable names (`share_config`, `jwks_cache`) that don't match actual `main.rs` variables | Minor | Updated to use actual variable names from main.rs: `jwks`, `share` |
| 96 | Task 7 step numbering jumps from Step 2 → Step 4 (missing Step 3) | Minor | Renumbered Step 4 → Step 3 |
| 97 | `let _user_id` binding in Step 5 conflicts with PostHog example in Step 3 which uses `user_id` (no underscore) — implementer gets compile error if copying example | Warning | Changed to `let user_id` (no underscore prefix) since it IS used for PostHog tracking |
| 98 | `evict_stale()` still holds DashMap shard **read** lock across `.await` (Mutex lock) — stalls executor thread under contention | Warning | Added `arc.clone(); drop(bucket_ref);` pattern to release shard lock before awaiting Mutex |
| 99 | `pub mod posthog;` declared in Task 15 Step 2 but `posthog.rs` file created in Task 16 Step 3 — `cargo check` fails after Task 15 | Blocker | Moved `pub mod posthog;` declaration to Task 16 Step 3 (where file is created). Added NOTE in Task 15 |
| 100 | `ws_handler` rewrite uses `StatusCode::UNAUTHORIZED.into_response()` but `StatusCode` is not imported in `ws.rs` | Blocker | Added `use axum::http::StatusCode;` to the ws.rs imports block |
| 101 | Integration tests use `Arc::new(RateLimiter::new(...))` but `std::sync::Arc` is not imported | Warning | Added `use std::sync::Arc;` to both test import blocks in Task 15 Step 5 |
| 102 | `posthog_client` is always `Some(reqwest::Client::new())` even when `POSTHOG_API_KEY` is empty — `Option` serves no purpose | Warning | Made conditional: `None` when API key is empty, `Some(Client::new())` when present |
| 103 | `TimeoutLayer::new(30s)` applied to ALL routes including `/ws` — kills WebSocket connections after 30 seconds | Warning | Split routes: HTTP routes get `TimeoutLayer`, WS route does not. Shared layers (body limit, CORS, trace) applied to all |
| 104 | Relay `SupabaseAuth` has no JWKS rotation support (server side does via `validate_jwt_with_rotation`) | Minor | Documented as known M1 limitation in `SupabaseAuth::from_supabase_url()` doc comment, with M2+ migration path |
| 105 | Route split code uses `get(health)` but `health` is not a defined function — existing code uses inline closure | Minor | Changed to `get(\|\| async { "ok" })` matching existing `lib.rs` |
| 106 | Route split code missing `.with_state(state)` — all handlers extract `State<RelayState>` and will panic without it | Blocker | Added `.with_state(state)` to final `app` construction |
| 107 | Route split uses bare function names (`create_pair`) — existing `lib.rs` uses module-qualified paths (`pairing::create_pair`) | Blocker | Changed to `pairing::create_pair`, `pairing::claim_pair`, `push::register_push_token`, `ws::ws_handler` |
| 108 | Route split uses `let app = ...;` (binding + semicolon) — must be tail expression to return from `app()` | Minor | Removed `let app =` binding; chain returns directly as tail expression |
| 109 | CORS import replacement not explicitly stated — `Any` would become unused import | Minor | Added instruction: "Replace the existing `use tower_http::cors::{Any, CorsLayer};` with:" |
| 110 | `use std::time::Duration;` duplicated in route split block — already imported at file scope | Minor | Added note: "Duration is already imported in lib.rs — do not duplicate" |
| 111 | `HeaderValue` and `Method` imports shown inline without clarifying they go at file scope | Minor | Added: "Add these imports at the top of `lib.rs` (alongside existing imports):" |
| 112 | `POSTHOG_API_KEY` env var read twice in constructor — wasteful and theoretically inconsistent | Minor | Extracted local `let posthog_key = ...` before struct literal; used for both fields |
| 113 | `tracing-subscriber` prelude import requires `registry` feature (default) — could break if defaults disabled | Minor | Added note: "Requires `tracing-subscriber` with default features. Do not add `default-features = false`." |
| 114 | Route split in Task 15 Step 4 replaces Router chain but doesn't remove existing `tokio::spawn` cleanup loop (lib.rs lines 17-27) — creates duplicate cleanup | Minor | Added instruction to replace ENTIRE `app()` body including the old cleanup spawn. Old TTL enforcement is superseded by `claim_pair`'s elapsed check |
| 115 | `tower` workspace dependency includes unused `"limit"` feature — plan uses custom `RateLimiter` + `tower_http::limit::RequestBodyLimitLayer`, not `tower::limit::*` | Minor | Removed `"limit"` from tower features: `["util", "timeout"]` only |
| 116 | Integration test imports for `Arc` and `RateLimiter` lack placement instructions (file-top vs inline) | Minor | Added explicit comment: "Add these imports at the TOP of `crates/relay/tests/integration.rs`" |
| 117 | AppState field count comment says "existing 16 fields" but actual count is 18 (start_time through sidecar) | Minor | Changed to "existing 18 fields (start_time through sidecar)" |
