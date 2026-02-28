# .env Cleanup & Audit Design

**Date:** 2026-02-28
**Status:** Approved

## Problem

The repo has accumulated env var ambiguity:
- A root `.env.example` that references vars for the Rust server (violates self-containment)
- `dotenvy::dotenv()` in two Rust crates doing "smart" `.env` file discovery
- No clear per-service env var documentation
- CLAUDE.md and README have overlapping/inconsistent env var docs

## Decisions

1. **No root `.env`** — each app/service is self-contained
2. **Shell exports only for Rust** — remove `dotenvy` entirely
3. **No Docker** — current PaaS deploys (Fly, Cloudflare Pages) are simpler
4. **`apps/web/.env.local`** — keep as-is (Vite's standard pattern)

## Changes

### 1. Remove root `.env.example`

Already deleted in working tree. Confirm the git deletion.

### 2. Remove `dotenvy` from Rust crates

- `crates/server/src/main.rs` — delete `dotenvy::dotenv().ok();`
- `crates/relay/src/main.rs` — delete `dotenvy::dotenv().ok();`
- Remove `dotenvy` from both crates' `Cargo.toml` dependencies

Rationale: `npx claude-view` users never have a `.env` file (binary is in `~/.cache/`). All env vars have hardcoded defaults. dotenvy is a no-op in production and unnecessary complexity in dev.

### 3. Keep `apps/web/.env.example`

Already clean and self-contained. Documents `VITE_*` vars for Vite.

### 4. Add `crates/server/.env.example`

Document server env vars for contributors (not loaded automatically):

```
# Rust server env vars — set via shell exports, NOT loaded from this file.
# This file exists only as documentation for contributors.
#
# RELAY_URL=ws://localhost:47893/ws
# CLAUDE_VIEW_PORT=47892
# CLAUDE_VIEW_DATA_DIR=./.data
# RUST_LOG=warn,claude_view_server=info
```

### 5. Add `crates/relay/.env.example`

```
# Relay server env vars — set via shell exports or Fly.io secrets.
# PORT=8080  (Fly sets this automatically)
# RUST_LOG=warn,claude_view_relay=info
```

### 6. Update CLAUDE.md

- Remove references to root `.env`
- Remove dotenvy mentions
- Update the secrets/env management table to reflect per-service approach

### 7. Update README.md

- Minor cleanup in "Environment Variables & Secrets" section
- Remove "no root .env" note (there simply won't be one)
- Clarify Rust reads from shell, not .env files

### 8. No Docker

No changes. Relay keeps its existing Dockerfile for Fly.io deploy. No docker-compose added.

## Env Var Inventory (Source of Truth)

| Service | Variable | Default | Where set | Notes |
|---------|----------|---------|-----------|-------|
| **Rust server** | `CLAUDE_VIEW_PORT` | `47892` | Shell export | |
| | `PORT` | (fallback) | Shell export | |
| | `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | Shell export | Corporate/sandbox only |
| | `RELAY_URL` | None (disabled) | Shell export | Mobile pairing |
| | `STATIC_DIR` | embedded dist | Dev only | |
| | `RUST_LOG` | `warn` | Shell export | |
| **Relay** | `PORT` | `8080` | Fly.io auto | |
| | `RUST_LOG` | `warn,claude_view_relay=info` | Fly secrets | |
| **Web frontend** | `VITE_SUPABASE_URL` | — | `apps/web/.env.local` | Publishable |
| | `VITE_SUPABASE_ANON_KEY` | — | `apps/web/.env.local` | Publishable |
| | `VITE_SENTRY_DSN` | — | `apps/web/.env.local` | Optional |
| **Sidecar** | `SIDECAR_SOCKET` | `/tmp/claude-view-sidecar-{ppid}.sock` | Auto | |
| **Landing** | — | — | — | Static HTML |
| **Mobile** | — | — | — | TBD |

## What Stays the Same

- `apps/web/.env.local` + `.env.example` — Vite's standard pattern
- `sidecar/` — only uses `process.env.SIDECAR_SOCKET` with sensible default
- `apps/landing/` — static HTML, zero env vars
- `apps/mobile/` — no env vars yet
- `crates/relay/Dockerfile` — stays for Fly.io deploy
