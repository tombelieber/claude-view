# Dev Scripts

Shell scripts that orchestrate the local dev stack. Called from `package.json`.

## Scripts

### `dev.sh` — full stack orchestrator (run by `bun dev`)

Runs Rust server + Vite web + sidecar in parallel with crash-aware teardown.

- **Pre-flight**: kills port holders AND orphan processes via `dev-cleanup.sh`
- **Env setup**: sets `CLAUDE_VIEW_SIDECAR_EXTERNAL=1` so Rust doesn't kill the
  dev-owned sidecar (see `CLAUDE.md` "Dev/Prod Process Ownership" rule)
- **Process supervision**: uses `concurrently --kill-others-on-fail` so any
  boot-phase crash tears down the whole stack instead of leaving a half-alive
  dev server

### `dev-cleanup.sh` — port + orphan process killer

Standalone cleanup utility. Kills:

1. Any process holding ports `47892` (Rust), `5173` (Vite), `3001` (sidecar)
2. Orphan processes matching `claude-view-server`, `cargo-watch.*claude-view-server`,
   `sidecar/src/index.ts`, `sidecar/dist/index.js`

Pattern-matching prevents nuking unrelated `node` / `cargo` processes on the
machine — we only kill things we own.

Usage:

```bash
bun run dev:cleanup            # kill ports + orphan processes
bash scripts/dev-cleanup.sh --quiet       # only print on action
bash scripts/dev-cleanup.sh --ports-only  # legacy port-only mode
```

Runs automatically before every `bun dev` via `dev.sh`.

### `dev-server.sh` — Rust server only

Standalone mode for debugging the Rust server without web/sidecar. Still
opts into `CLAUDE_VIEW_SIDECAR_EXTERNAL=1` so it never kills an externally-
running sidecar.

### `sidecar/scripts/dev-watch.sh` — crash-aware sidecar supervisor

Wraps `node --import tsx --watch src/index.ts` with an HTTP health-check
loop. **Without this wrapper, node's `--watch` flag SWALLOWS CRASHES** — it
keeps the parent process alive after any boot-phase or runtime error and
silently waits for a file change. That's fatal for a dev orchestrator:
sidecar dies, frontend hangs at "connecting", no error visible.

The supervisor polls `GET /health` every 2 seconds:

- **Boot-phase crash**: if the port never opens within 15 seconds, exit with
  code 1. `concurrently --kill-others-on-fail` sees the non-zero exit and
  tears down the whole stack. User sees actionable error immediately.
- **Runtime crash**: if 3 consecutive health checks fail after boot, exit 1.
  Same teardown path.
- **File-change reload**: node's own `--watch` handles file reloads in
  ~300ms. Our health-check threshold (3 failures × 2s = 6s) is well above
  that, so normal reloads never falsely trigger teardown.
- **Clean shutdown**: SIGTERM/SIGINT propagates to the node child.

## Why not use nodemon / tsx-watch / pm2?

- **nodemon**: has the same crash-swallowing bug. Its `--exitcrash` flag
  exits on crash, but it also disables file-watch. No middle ground.
- **tsx watch**: same crash-swallowing behaviour as `node --watch`.
- **pm2**: production process manager, too heavy for dev.

50 lines of bash solves the problem and has no external deps.

## Port assignments (single source of truth)

| Port  | Service           |
|-------|-------------------|
| 47892 | Rust server (HTTP API) |
| 5173  | Vite dev server (frontend) |
| 3001  | Sidecar (Node.js / tsx, SDK proxy) |

If you add a new port, update:

- `scripts/dev-cleanup.sh` (PORTS array)
- `apps/web/vite.config.ts` (proxy config)
- This README
