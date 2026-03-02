# Claude View - Project Instructions

## Decisions Already Made (Stop Revisiting)

### Distribution vs Development
| Concern | Choice | Why |
|---------|--------|-----|
| **Distribution (users)** | `npx claude-view` | 95% of devs have Node, maximum reach |
| **Development (you)** | Bun | Fast, npm-compatible, use it locally |

`bun.lock` tracked in git. `package-lock.json` removed — npm can't resolve `workspace:*` protocol. Never use `npm install` for dev.

### Distribution Strategy
| Decision | Choice |
|----------|--------|
| Primary install | `npx claude-view` (downloads pre-built binary) |
| Secondary install | `brew install claude-view` |
| NO cargo install | Users don't need Rust |
| NO Docker for MVP | Complicates local file access |
| NO Bun-only | ~95% have Node, ~15% have Bun |

### Architecture
| Decision | Choice |
|----------|--------|
| Runtime | Localhost web server (browser, not desktop app) |
| Backend | Rust (Axum), ~15MB binary |
| Frontend | React SPA (Vite) + Expo native app |
| Monorepo | Turborepo + Bun workspaces |
| Desktop app (Tauri) | Deferred indefinitely |
| Node.js sidecar | Mission Control Phase F |

### Monorepo Workspace Layout

| Path | Package | Purpose |
|------|---------|---------|
| `apps/web/` | `@claude-view/web` | React SPA (Vite) — the main web frontend |
| `apps/share/` | `@claude-view/share` | Share viewer SPA (Vite) — Cloudflare Pages |
| `apps/mobile/` | `@claude-view/mobile` | Expo SDK 55 native app (Tamagui v2) |
| `apps/landing/` | `@claude-view/landing` | Static HTML landing page (Cloudflare Pages) |
| `packages/shared/` | `@claude-view/shared` | Relay types, theme tokens |
| `packages/design-tokens/` | `@claude-view/design-tokens` | Colors, spacing, typography |
| `crates/` | — | Rust backend (unchanged) |
| `infra/share-worker/` | — | Cloudflare Worker — share API (R2 + D1) |

**Key config files:**

- `turbo.json` — Turborepo pipeline config
- `bunfig.toml` — `linker = "hoisted"` (required for Metro/Expo compatibility)
- `tsconfig.base.json` — shared TypeScript base config, apps extend it
- React pinned to 19.2.0 across all apps for deduplication

### Rust Crate Structure
| Crate | Package name | Purpose |
|-------|-------------|---------|
| `crates/core/` | `claude-view-core` | Shared types, JSONL parser, skill extraction |
| `crates/db/` | `claude-view-db` | SQLite via sqlx |
| `crates/search/` | `claude-view-search` | Tantivy full-text indexer + query |
| `crates/server/` | `claude-view-server` | Axum HTTP routes, **produces the `claude-view` binary** |

**Naming:** The crate is `claude-view-server`, the binary is `claude-view`. Use `cargo test -p claude-view-server` for dev, users run `claude-view`.

### Other Decisions
| Decision | Choice |
|----------|--------|
| Rename | Settled on **claude-view**. No further rename planned. |
| Default port | `47892` (override: `CLAUDE_VIEW_PORT`, fallback: ephemeral) |
| Platform MVP | macOS (ARM64 + Intel). Linux v2.1, Windows v2.2 |

## What NOT to Do

1. Don't suggest Docker for MVP
2. Don't suggest `cargo install`
3. Don't change port to 3000
4. Don't build Tauri desktop app
5. Don't over-engineer — ship Mac-first MVP
6. Don't require Bun for users

## Key Docs

- `docs/plans/PROGRESS.md` — Current status (start here each session)
- `docs/plans/mission-control/PROGRESS.md` — Mission Control feature tracker
- `docs/plans/mission-control/design.md` — Mission Control full design spec
- `README.md` — User-facing docs (trilingual: EN, zh-TW, zh-CN)

## Private Docs (sibling repo)

Business strategy and operational plans live in a **private sibling repo** (one level up, the GTM repo).

```
(private sibling repo)/
  vision/          — VISION.md, ROADMAP.md
  plans/active/    — executable plans (action items, ops tasks, strategy plans)
  plans/backlog/   — backlog strategy plans
  marketing/       — release runbook, blog drafts
```

**Rules:**
- To find it: `ls ../ | grep gtm` from this repo root.
- **Read:** When business context is needed (pricing, product direction), read from that repo.
- **Write:** When creating docs about pricing, monetization, product vision, competitive analysis, GTM, or business strategy, ALWAYS write them to the private sibling repo — never to this repo. Match the directory by topic and status:
  - `vision/` for product direction docs
  - `plans/active/` for plans currently being worked on
  - `plans/backlog/` for future/deferred plans
  - `marketing/` for launch, blog, and content marketing
- **Execute:** When the user says "execute a plan", check that repo's `plans/active/` for the matching plan file and follow it.
- **Search:** When searching for context across both repos, check this repo for engineering docs and the sibling repo for business docs.
- Never commit business/strategy docs to this repo.

## Development Priorities

1. Local dev working first — Rust backend serves existing React UI
2. npx deployment second — defer until backend works

### Dev Commands (Monorepo)

| Command | What it does |
|---------|-------------|
| `bun dev` | Full-stack dev — Rust server (cargo-watch) + Web frontend (Vite HMR) |
| `bun run dev:web` | Web frontend only (assumes Rust server running) |
| `bun run dev:server` | Rust backend only (with cargo-watch) |
| `bun run dev:all` | All JS/TS apps via Turbo (web + mobile + landing, no Rust) |
| `bun run preview` | Production-like local — builds web, runs Rust with prod-like share URLs |
| `bun run build` | `bunx turbo build` — builds all apps |
| `bun run test` | `bunx turbo test` — runs all test suites |
| `cd apps/web && bunx vitest run` | Run web frontend tests only |
| `cargo test -p claude-view-server` | Run Rust server tests only |
| `bun run deploy:share:dev` | Build share SPA (`.env.dev`) + deploy to Pages dev |
| `bun run deploy:share` | Build share SPA (`.env.production`) + deploy to Pages prod |

**Shell-injected share env vars** (Rust server reads these via `std::env::var()`):

| Script | `SHARE_WORKER_URL` (API calls) | `SHARE_VIEWER_URL` (user-facing links) |
| ------------ | --------------------------------------------------------------- | ----------------------------------------- |
| `dev:server` | `claude-view-share-worker-dev.vickyai-tech.workers.dev` | `claude-view-share-viewer-dev.pages.dev` |
| `preview` | `api-share.claudeview.ai` | `share.claudeview.ai` |
| `start` | Not set — sharing disabled unless exported in shell | Not set |

Both `dev:server` and `preview` use `unset SHARE_WORKER_URL SHARE_VIEWER_URL` then hardcode the values — they are NOT overridable via prior shell exports. `SUPABASE_URL` still uses `${VAR:-default}` syntax and can be overridden. `start` is bare `cargo run --release` for production where env vars are set externally (systemd, Docker, CI).

## Git Discipline — Dirty Working Tree

**NEVER `git add` a file that has pre-existing unstaged modifications unless you are ONLY committing your own changes.** When `git status` shows ` M` (unstaged) files at session start, those are the user's in-progress work — not yours to commit.

Before ANY `git add`:
1. Run `git status` and note all pre-existing ` M` files
2. If a file you need to edit is already modified, **STOP and warn the user**: "This file has uncommitted changes. Should I commit your work first, or isolate my changes?"
3. Never commit the user's WIP under your commit message — it destroys git history and makes the user think their work was reverted
4. If you must edit a file with pre-existing changes, either:
   - Ask the user to commit their work first, OR
   - Use `git stash` before starting, make your changes on a clean tree, commit, then `git stash pop`
5. After `git add`, verify the diff size makes sense — if your change was 7 lines but the staged diff is 500+ lines, something is wrong

**The golden rule:** Your commit should contain ONLY your changes. The user's uncommitted work is sacred — don't touch it, don't commit it, don't mix it with yours.

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
| **Share viewer** | `apps/share/.env.dev` / `.env.production` (Vite `VITE_*`) | `apps/share/.env.production` |
| **Sidecar** | `process.env` with defaults | N/A (only `SIDECAR_SOCKET`) |
| **Landing** | None (static HTML) | N/A |
| **Share Worker** | `wrangler secret put` / `wrangler.toml` [vars] | `infra/share-worker/wrangler.toml` |
| **CI/CD** | GitHub Actions secrets | N/A |

### Rules

- **No root `.env`** — each service is self-contained
- **No `dotenvy`** — Rust reads shell env only, no magic file loading
- **Never commit `.env`, `.env.local`, or `.dev.vars`** — they are gitignored
- **Never put secret keys in `.env.example`** — only placeholders
- **Publishable keys are safe to embed** in client code (Supabase publishable key, Supabase URL)
- **Service role / secret keys are NEVER used** in this project — JWT validation uses JWKS

### Cloudflare Dev/Prod Strategy

Primary domain: **claudeview.ai**. `claudeview.com` redirects to `claudeview.ai`.

| Service | Dev | Production |
| ------- | --- | ---------- |
| **Share Worker (API)** | `claude-view-share-worker-dev` (`.workers.dev`) | `claude-view-share-worker-prod` → `api-share.claudeview.ai` |
| **Share Viewer (SPA)** | `claude-view-share-viewer-dev` (Pages) | `claude-view-share-viewer` → `share.claudeview.ai` |
| **D1 Database** | `claude-view-share-d1-dev` | `claude-view-share-d1-prod` |
| **R2 Bucket** | `claude-view-share-r2-dev` | `claude-view-share-r2-prod` |
| **Landing** | Cloudflare Pages preview | `claudeview.ai` |

**Two-domain split:** The share feature uses two separate Cloudflare services on different subdomains:

- `api-share.claudeview.ai` — Worker (API: create/upload/fetch/delete encrypted blobs)
- `share.claudeview.ai` — Pages (SPA: renders shared conversations in browser)

The Rust server uses `SHARE_WORKER_URL` to call the API and `SHARE_VIEWER_URL` to build user-facing links. The share SPA uses `VITE_WORKER_URL` to fetch encrypted blobs from the API. Never conflate these two domains.

Pattern: `claude-view-share-{type}-{env}` — always suffix with `-dev` or `-prod`.

Deploy commands:

- Worker dev: `cd infra/share-worker && npx wrangler deploy --env dev`
- Worker prod: `cd infra/share-worker && npx wrangler deploy`
- Viewer dev: `bun run deploy:share:dev` (builds with `.env.dev`, deploys to Pages)
- Viewer prod: `bun run deploy:share` (builds with `.env.production`, deploys to Pages)

Secrets (set via `wrangler secret put`, NEVER in code/docs):

- `SUPABASE_URL` — set per environment (`--env dev` for dev)
- Any future secrets follow the same pattern

Safe to document (public):

- Supabase project URL, publishable key
- Worker names, D1/R2 resource names
- Domain layout

NEVER document:

- Supabase secret key (`sb_secret_*`)
- Wrangler secret values
- Any `*.workers.dev` URLs with auth tokens

## Hard Rules

> Detailed code examples: `docs/claude-rules-reference.md`

### Rust

- **Env vars:** Strip all `CLAUDE*` env vars when spawning `claude` CLI — hardcode `CLAUDECODE`, `CLAUDE_CODE_SSE_PORT`, `CLAUDE_CODE_ENTRYPOINT` + dynamic prefix scan + `unset` in `dev:server` script + `.stdin(Stdio::null())`
- **sysinfo on macOS:** Never rely solely on `sysinfo` for process cwd/cmd — use `lsof -a -p <pid> -d cwd -Fn` fallback
- **Path decoding:** Use `claude_view_core::discovery::resolve_project_path()` for Claude Code directory names. Never `urlencoding::decode()`
- **Tracing:** Use `EnvFilter`, never `with_max_level()`. Scope RUST_LOG: `warn,claude_view_server=info,claude_view_core=info`
- **Background processes:** `Semaphore(1)` for external calls. No trigger on first discovery (only real state transitions). Backoff on failure. Kill switch config (`enabled: bool`, default `false` for expensive ops)
- **mmap:** Parse directly, never `.to_vec()`
- **memmem::Finder:** Create once at loop top, pass by reference
- **SQLite:** Batch writes in transactions, never individual statements in loops
- **Startup:** Server binds port before any indexing/background work
- **SIMD pre-filter:** `memmem::Finder` check before JSON parse
- **Parallelism:** `Semaphore` bounded to `available_parallelism()`
- **JWT/JWKS:** NEVER hardcode JWT algorithm (RS256, ES256, etc.). Supabase changes signing algorithms without notice (moved from HS256 → ES256). Always read the `alg` field from the JWKS response and use it dynamically. See `crates/server/src/auth/supabase.rs` — `jwk_algorithm()` parses `alg` from the JWK JSON.

### Full-Stack Wiring

Trace every new field end-to-end: **DB column -> SELECT query -> Rust struct -> JSON -> API response -> TS type -> hook -> component -> browser**. `Option`/`undefined` silently absorbs gaps — manual browser verification catches what tests won't.

### Frontend / React

- **Source location:** `apps/web/src/` (not root `src/` -- that was the pre-monorepo layout)
- **useEffect deps:** Never raw parsed objects. `useMemo` on a primitive key
- **URL params:** Copy-then-modify (`new URLSearchParams(existing)`), never blank constructor
- **Timestamps:** Guard `ts <= 0` before `new Date(ts * 1000)` at every layer. Timestamp 0 = data bug
- **WebSocket stale guard:** Every WS handler must check `wsRef.current !== ws` before acting
- **No shadcn/ui CSS vars:** `text-muted-foreground`, `bg-muted`, etc. are undefined here. Use explicit Tailwind + `dark:` variants
- **Radix UI:** Use `@radix-ui/react-*` for overlays. Never hand-roll hover/positioning

### Statistical Analysis

- Shared constants from `crates/core/src/patterns/mod.rs`: `MIN_BUCKET_SIZE=10`, `MIN_BUCKETS=3`, `MIN_MODEL_BUCKET=30`, `MAX_SESSION_DURATION=14400`, `MAX_DISPLAY_PCT=200.0`
- Cap percentages with `format_improvement()`. Directional language for signed metrics
- Filter `duration == 0` and `> 14400s`. `commit_count == 0` is normal, not failure
- No tautological metrics, no degenerate "zero" buckets, guard selection bias
- Every template variable must be emitted by its pattern function

### External Data

Never trust a single external data source. Cross-check indexes against filesystem. Aggregates must UPDATE existing rows, not skip-if-exists. Log discrepancies.

### SSE / Vite Dev Proxy

Vite buffers SSE. In dev mode, connect `EventSource` directly to Rust server at `:47892`. Test SSE with `cd apps/web && bun run preview`.

### Frontend Changes Require `bun run build`

`cargo run` only rebuilds the Rust server binary. The frontend JS bundle in `dist/` is a **separate build artifact**. After editing any `.ts`/`.tsx`/`.css` file, you MUST run `bun run build` before restarting the server — otherwise the browser serves the stale old bundle and your changes are invisible. **Always `bun run build` after frontend changes.**

### Release Process

Use `./scripts/release.sh {patch|minor|major}` then push with tags. Also bump `Cargo.toml` workspace version to match. Never manually create tags or GitHub releases.

### Testing

- **Test only what changed.** Before running tests, check `git diff --name-only` to identify touched crates/apps.
- **Rust:** Only run `cargo test -p claude-view-{crate}` for crates with actual changes. Never blanket-run `cargo test -p claude-view-core` (626 tests, ~60s) unless core was modified.
- **Module-scoped filters** when only a few files changed: `cargo test -p claude-view-core parser::` instead of the full crate.
- **Full Rust workspace** (`cargo test`) only for cross-crate changes (e.g. shared types in core consumed by server).
- **Web frontend:** `cd apps/web && bunx vitest run` (not `bun run test:client` -- that no longer exists).
- **All workspaces:** `bun run test` runs `bunx turbo test` across all apps.

## UI/UX Rules

See `docs/uiux-notes.md` for full checklist. Key: active states on clickables, consistent URL param names, filters applied in all views, toggle = click/deselect, no hooks after early returns, popover draft reset on open transition only.
