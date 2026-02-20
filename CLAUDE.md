# Claude View - Project Instructions

## Decisions Already Made (Stop Revisiting)

### Distribution vs Development
| Concern | Choice | Why |
|---------|--------|-----|
| **Distribution (users)** | `npx claude-view` | 95% of devs have Node, maximum reach |
| **Development (you)** | Bun | Fast, npm-compatible, use it locally |

Both lockfiles (`bun.lock`, `package-lock.json`) tracked in git. Never use `npm install` for dev. Never delete `package-lock.json`.

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
| Frontend | React SPA |
| Desktop app (Tauri) | Deferred indefinitely |
| Node.js sidecar | Mission Control Phase F |

### Rust Crate Structure
| Crate | Purpose |
|-------|---------|
| `crates/core/` | Shared types, JSONL parser, skill extraction |
| `crates/db/` | SQLite via sqlx |
| `crates/search/` | Tantivy full-text indexer + query |
| `crates/server/` | Axum HTTP routes |

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
- `docs/VISION.md` — Product evolution, business model, identity
- `docs/ROADMAP.md` — Module roadmap, priorities, deferred items
- `docs/plans/mission-control/PROGRESS.md` — Mission Control feature tracker
- `docs/plans/mission-control/design.md` — Mission Control full design spec
- `README.md` — User-facing docs (trilingual: EN, zh-TW, zh-CN)

## Development Priorities

1. Local dev working first — Rust backend serves existing React UI
2. npx deployment second — defer until backend works

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

### Full-Stack Wiring

Trace every new field end-to-end: **DB column -> SELECT query -> Rust struct -> JSON -> API response -> TS type -> hook -> component -> browser**. `Option`/`undefined` silently absorbs gaps — manual browser verification catches what tests won't.

### Frontend / React

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

Vite buffers SSE. In dev mode, connect `EventSource` directly to Rust server at `:47892`. Test SSE with `bun run preview`.

### Release Process

Use `./scripts/release.sh {patch|minor|major}` then push with tags. Also bump `Cargo.toml` workspace version to match. Never manually create tags or GitHub releases.

### Testing

Scoped: `cargo test -p claude-view-{core,db,server,search}`. Full workspace only for cross-crate changes.

## UI/UX Rules

See `docs/uiux-notes.md` for full checklist. Key: active states on clickables, consistent URL param names, filters applied in all views, toggle = click/deselect, no hooks after early returns, popover draft reset on open transition only.
