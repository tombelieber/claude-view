# .env Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove dotenvy, delete root .env.example, update all docs to reflect per-service env var layout with shell exports only.

**Architecture:** Each service owns its own env vars. No root .env. No automatic .env file loading. Rust reads from shell, Vite reads from apps/web/.env.local. All documented env vars stay (features coming soon) but docs reflect the new layout.

**Tech Stack:** Rust (Cargo.toml), CLAUDE.md, README.md (all languages)

---

### Task 1: Remove dotenvy from Rust server

**Files:**
- Modify: `crates/server/src/main.rs:159-160`
- Modify: `crates/server/Cargo.toml:79`

**Step 1: Remove dotenvy call from server main.rs**

In `crates/server/src/main.rs`, delete lines 159-160:
```rust
    // Load .env file if present (no-op if missing)
    dotenvy::dotenv().ok();
```

**Step 2: Remove dotenvy dependency from server Cargo.toml**

In `crates/server/Cargo.toml`, delete line 79:
```toml
dotenvy = { workspace = true }
```

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-server`
Expected: success (no code references dotenvy)

---

### Task 2: Remove dotenvy from Rust relay

**Files:**
- Modify: `crates/relay/src/main.rs:7`
- Modify: `crates/relay/Cargo.toml:23`

**Step 1: Remove dotenvy call from relay main.rs**

In `crates/relay/src/main.rs`, delete line 7:
```rust
    dotenvy::dotenv().ok();
```

**Step 2: Remove dotenvy dependency from relay Cargo.toml**

In `crates/relay/Cargo.toml`, delete line 23:
```toml
dotenvy = { workspace = true }
```

**Step 3: Verify it compiles**

Run: `cargo check -p claude-view-relay`
Expected: success

---

### Task 3: Remove dotenvy from workspace Cargo.toml

**Files:**
- Modify: `Cargo.toml:58-59`

**Step 1: Remove dotenvy from workspace dependencies**

In root `Cargo.toml`, delete lines 58-59:
```toml
# Environment
dotenvy = "0.15"
```

**Step 2: Full workspace check**

Run: `cargo check --workspace`
Expected: success — no crate references dotenvy anymore

**Step 3: Commit**

```bash
git add crates/server/src/main.rs crates/server/Cargo.toml crates/relay/src/main.rs crates/relay/Cargo.toml Cargo.toml
git commit -m "refactor: remove dotenvy — all env vars via shell exports only"
```

---

### Task 4: Delete root .env.example, add per-crate .env.example

**Files:**
- Delete: `.env.example` (confirm git rm)
- Create: `crates/server/.env.example`
- Create: `crates/relay/.env.example`

**Step 1: Git-remove root .env.example**

```bash
git rm .env.example
```

**Step 2: Create crates/server/.env.example**

```
# Rust server env vars — set via shell exports, NOT loaded from a file.
# This file exists only as documentation for contributors.
#
# Core (all have sensible defaults — zero-config for npx users):
# CLAUDE_VIEW_PORT=47892
# CLAUDE_VIEW_DATA_DIR=~/.cache/claude-view
# RUST_LOG=warn,claude_view_server=info
#
# Optional features:
# RELAY_URL=ws://localhost:47893/ws       # Mobile pairing (wss:// in production)
# SUPABASE_URL=https://your-project.supabase.co  # Sharing/auth (future)
# SHARE_WORKER_URL=https://share.claude.view     # Share worker endpoint (future)
# SHARE_VIEWER_URL=https://share.claude.view/s   # Share viewer SPA URL (future)
```

**Step 3: Create crates/relay/.env.example**

```
# Relay server env vars — set via Fly.io secrets or shell exports.
# This file exists only as documentation for contributors.
#
# PORT=8080                              # Fly.io sets this automatically
# RUST_LOG=warn,claude_view_relay=info
```

**Step 4: Commit**

```bash
git add .env.example crates/server/.env.example crates/relay/.env.example
git commit -m "chore: move .env.example to per-service locations"
```

---

### Task 5: Update apps/web/.env.example

**Files:**
- Modify: `apps/web/.env.example`

**Step 1: Update with clearer documentation**

Replace contents of `apps/web/.env.example` with:

```
# Web frontend env vars — Vite reads VITE_* at build time.
# Copy to .env.local and fill in values for local development.
# These are publishable keys (safe to embed in browser).
#
# For npx users: these are baked into the binary at release build time via CI.
# For contributors: cp .env.example .env.local && fill in values.

# Supabase Auth (publishable keys)
VITE_SUPABASE_URL=https://your-project.supabase.co
VITE_SUPABASE_ANON_KEY=sb_publishable_...

# Observability (optional)
VITE_SENTRY_DSN=
```

**Step 2: Commit**

```bash
git add apps/web/.env.example
git commit -m "docs: clarify apps/web/.env.example for contributors"
```

---

### Task 6: Update CLAUDE.md — Secrets & Environment Variables section

**Files:**
- Modify: `CLAUDE.md:140-173`

**Step 1: Replace the "Secrets & Environment Variables" section**

Replace lines 140-173 with:

```markdown
## Secrets & Environment Variables

### Architecture

End users running `npx claude-view` need **ZERO configuration**. All public keys/URLs are baked into the JS bundle at CI build time. No `.env` files ship with the binary. No `.env` files are loaded at runtime.

### Per-service env var layout

Each service manages its own env vars. No root `.env`. No automatic `.env` file loading in Rust (dotenvy removed).

| Service | How env vars are set | .env.example location |
|---------|---------------------|----------------------|
| **Rust server** | Shell exports (`std::env::var()`) | `crates/server/.env.example` |
| **Relay** | Fly.io secrets / shell exports | `crates/relay/.env.example` |
| **Web frontend** | `apps/web/.env.local` (Vite `VITE_*`) | `apps/web/.env.example` |
| **Sidecar** | `process.env` with defaults | N/A (only `SIDECAR_SOCKET`) |
| **Landing** | None (static HTML) | N/A |
| **Cloudflare Worker** | `wrangler secret put` / `wrangler.toml` [vars] | N/A |
| **CI/CD** | GitHub Actions secrets | N/A |

### Rules

- **No root `.env`** — each service is self-contained
- **No `dotenvy`** — Rust reads shell env only, no magic file loading
- **Never commit `.env`, `.env.local`, or `.dev.vars`** — they are gitignored
- **Never put secret keys in `.env.example`** — only placeholders
- **Publishable keys are safe to embed** in client code (Supabase anon key, Supabase URL)
- **Service role / secret keys are NEVER used** in this project — JWT validation uses JWKS
```

---

### Task 7: Update README.md — Environment Variables & Secrets section

**Files:**
- Modify: `README.md:327-371`

**Step 1: Replace the "Environment Variables & Secrets" section**

Replace lines 327-371 with:

```markdown
### Environment Variables & Secrets

**End users need ZERO configuration.** `npx claude-view` works out of the box.

This section is for **developers contributing to claude-view**.

Each service manages its own env vars — no root `.env`, no automatic file loading.

**Rust server** (`crates/server/.env.example`) — set via shell exports:

| Variable | Default | Purpose |
|----------|---------|---------|
| `CLAUDE_VIEW_PORT` | `47892` | Override the default port |
| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | Override data directory |
| `RELAY_URL` | None (disabled) | Mobile relay WebSocket endpoint |
| `SUPABASE_URL` | — | Supabase project URL (sharing/auth) |
| `SHARE_WORKER_URL` | — | Cloudflare Share Worker endpoint |
| `SHARE_VIEWER_URL` | — | Share viewer SPA URL |
| `RUST_LOG` | `warn` | Tracing verbosity |

**Relay server** (`crates/relay/.env.example`) — set via Fly.io secrets:

| Variable | Default | Purpose |
|----------|---------|---------|
| `PORT` | `8080` | Fly.io sets this automatically |
| `RUST_LOG` | `warn,claude_view_relay=info` | Tracing verbosity |

**Web frontend** (`apps/web/.env.local`) — Vite reads `VITE_*` at build time:

| Variable | Purpose |
|----------|---------|
| `VITE_SUPABASE_URL` | Supabase URL (browser SDK) |
| `VITE_SUPABASE_ANON_KEY` | Supabase publishable key |
| `VITE_SENTRY_DSN` | Sentry error tracking (optional) |

**Secret management:**

| Service | How secrets are managed |
|---------|------------------------|
| Cloudflare Worker | `wrangler secret put` (encrypted at rest) |
| Fly.io Relay | `fly secrets set` |
| Web frontend | `VITE_*` baked at CI build time (publishable keys only) |
| Rust server | Shell exports |
| CI/CD | GitHub Actions secrets |

```bash
# Developer setup
cp apps/web/.env.example apps/web/.env.local   # Fill in Supabase credentials
export RELAY_URL=ws://localhost:47893/ws        # Optional: enable mobile pairing
bun dev                                        # Start full-stack dev
```

> **Note:** `.env.local` is gitignored. Never commit real credentials. The shipped binary contains only publishable keys.
```

**Step 2: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "docs: update env var docs to per-service layout, remove root .env references"
```

---

### Task 8: Update translated READMEs — add missing `CLAUDE_VIEW_DATA_DIR`

**Files:**
- Modify: `README.de.md`, `README.es.md`, `README.fr.md`, `README.it.md`, `README.ja.md`, `README.ko.md`, `README.nl.md`, `README.pt.md`, `README.zh-TW.md`, `README.zh-CN.md`

**IMPORTANT:** The translated READMEs do NOT have the full developer "Environment Variables & Secrets" section. They only have a minimal consumer-facing `### Configuration` table at ~line 169-173 with a single `CLAUDE_VIEW_PORT` row. There is no developer env var content to update.

**Step 1: Add `CLAUDE_VIEW_DATA_DIR` row to each translated README's config table**

The English README has two rows in its Configuration table, but translated READMEs only have one. Add the missing `CLAUDE_VIEW_DATA_DIR` row to match.

For each file, find the config table (at ~line 173) and add the missing row after `CLAUDE_VIEW_PORT`. Examples:

**README.zh-TW.md** (~line 173): Add after the `CLAUDE_VIEW_PORT` row:
```
| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | 覆蓋資料目錄 |
```

**README.ja.md** (~line 173): Add after the `CLAUDE_VIEW_PORT` row:
```
| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | データディレクトリを上書き |
```

**README.de.md** (~line 173): Add after the `CLAUDE_VIEW_PORT` row:
```
| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | Datenverzeichnis überschreiben |
```

**README.es.md**: `| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | Ruta del directorio de datos |`
**README.fr.md**: `| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | Répertoire de données |`
**README.it.md**: `| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | Directory dei dati |`
**README.ko.md**: `| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | 데이터 디렉토리 경로 |`
**README.nl.md**: `| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | Data-directory overschrijven |`
**README.pt.md**: `| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | Diretório de dados |`
**README.zh-CN.md**: `| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | 覆盖数据目录 |`

No other changes to translated READMEs — the developer env var section only exists in the English README.

**Step 2: Commit**

```bash
git add README.*.md
git commit -m "docs: add CLAUDE_VIEW_DATA_DIR to translated README config tables"
```

---

### Task 9: Verify everything compiles and tests pass

**Step 1: Full Rust workspace check**

Run: `cargo check --workspace`
Expected: success

**Step 2: Run affected Rust tests**

Run: `cargo test -p claude-view-server && cargo test -p claude-view-relay`
Expected: all pass

**Step 3: Verify .env.example files exist in right places**

```bash
ls crates/server/.env.example crates/relay/.env.example apps/web/.env.example
```
Expected: all three exist

**Step 4: Verify root .env.example is gone**

```bash
test ! -f .env.example && echo "PASS: root .env.example removed"
```

---

## Changelog of Fixes Applied (Audit -> Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Task 8 assumed translated READMEs have same dev env var section as English README — they don't, only a minimal 1-row config table | Blocker | Rewrote Task 8 to only add missing `CLAUDE_VIEW_DATA_DIR` row to each translated config table |
| 2 | CLAUDE.md references phantom path `infra/share-worker/.dev.vars` (infra/ dir doesn't exist) | Minor | Plan correctly removes it — confirmed intentional |
| 3 | Tasks 1-3 line numbers for dotenvy verified exactly against codebase | PASS | No change needed |
| 4 | Task 6 CLAUDE.md section boundary (lines 140-173) verified | PASS | No change needed |
| 5 | Task 7 README.md section boundary (lines 327-371) verified | PASS | No change needed |
