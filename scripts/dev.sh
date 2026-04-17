#!/usr/bin/env bash
# scripts/dev.sh — dev-stack orchestrator.
#
# Replaces the ~400-char inline `concurrently` in package.json with a
# readable script that:
#
#   1. Pre-cleans ports AND orphan processes (fixes "Cannot find module
#      './cjs/index.cjs' from ''" ghost errors from stale tsx/node processes).
#   2. Sets CLAUDE_VIEW_SIDECAR_EXTERNAL=1 so Rust never kills the
#      externally-owned sidecar (CLAUDE.md "Dev/Prod Process Ownership" rule).
#   3. Runs rust, web, and sidecar in parallel with named coloured output
#      via concurrently, with --kill-others-on-fail so any boot-phase crash
#      tears down the whole stack immediately (no half-alive dev server).
#
# Exit codes propagate. Ctrl-C gracefully kills all children.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# 1. Clean pre-flight: ports + orphan processes.
echo "⟳ cleaning up old dev processes..."
bash scripts/dev-cleanup.sh --quiet

# 2. Shared env for all three processes. Kept here (not duplicated per-script)
#    so there's ONE source of truth for dev-mode behaviour.

# Load .env.local (gitignored) for optional dev-only config such as
# SHARE_WORKER_URL. See .env.example at repo root for the template.
if [ -f "$ROOT/.env.local" ]; then
  set -a; source "$ROOT/.env.local"; set +a
fi

export RUST_LOG="${RUST_LOG:-warn,claude_view_server=info,claude_view_core=info}"
export VITE_PORT="${VITE_PORT:-5173}"
export SUPABASE_URL="${SUPABASE_URL:-https://iebjyftoadahqptmfcio.supabase.co}"
# Optional: share feature. Empty value disables sharing. Populate in .env.local.
export SHARE_WORKER_URL="${SHARE_WORKER_URL:-}"
export SHARE_VIEWER_URL="${SHARE_VIEWER_URL:-}"

# CLAUDE.md hard rule: strip these before anything spawns SDK child processes.
unset CLAUDECODE CLAUDE_CODE_SSE_PORT CLAUDE_CODE_ENTRYPOINT

# CLAUDE.md "Dev/Prod Process Ownership" rule: dev opts out of Rust's
# sidecar lifecycle management. Only the `dev.sh`-orchestrated sidecar
# owns port 3001 in dev. This gate must NEVER be removed.
export CLAUDE_VIEW_SIDECAR_EXTERNAL=1

# 3. Run all three with concurrently.
#
# --kill-others-on-fail: if ANY of the three crashes at boot, tear down
#    the whole stack. Previously --kill-others only fired on clean exit,
#    leaving a half-alive stack when e.g. the sidecar had a tsx loader error.
#
# --restart-tries=1 --restart-after=2000: one retry for the Rust server
#    specifically, because cargo watch occasionally emits transient
#    "file changed" spam during a rebuild storm. NOT applied to sidecar
#    (crashes there are structural — retry would mask real bugs).
#
# Per-process notes:
#   rust:    cargo watch recompiles on any crates/ change. Rebuild is slow
#            (~8s first boot, ~3s incremental via cq). Output is verbose.
#            The watched command goes through ./scripts/cq so all worktrees
#            share one target dir and one queued cargo lane.
#   web:     vite is fast (~200ms ready). Proxies to :47892 and :3001.
#            First 8s will show ECONNREFUSED for /api/* until rust binds.
#            This is expected — not an error.
#   sidecar: node --import tsx --watch reloads on any sidecar/src/ change
#            in ~150ms. Zero-downtime because the OS re-binds :3001 on each
#            restart, and Rust's external-mode reconciler health-checks
#            every 10s to re-attach.
exec bunx concurrently \
  --kill-others-on-fail \
  --names "rust,web,sidecar" \
  --prefix-colors "red,cyan,yellow" \
  --prefix "[{name}]" \
  --timestamp-format "HH:mm:ss" \
  "cargo watch -w crates -s './scripts/cq run -p claude-view-server'" \
  "bun --filter='@claude-view/web' run dev" \
  "cd sidecar && bun run dev"
