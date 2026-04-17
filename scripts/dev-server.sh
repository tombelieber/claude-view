#!/usr/bin/env bash
# scripts/dev-server.sh — Rust server only (no web, no sidecar).
#
# Use this when:
#   - You want to debug the Rust server in isolation
#   - You're running sidecar + web in separate terminals
#
# For the full stack (rust + web + sidecar), use `bun dev` instead.
#
# Notes:
# - Still opts into external sidecar mode to avoid killing any
#   externally-running sidecar on port 3001.
# - Uses cq (build queue) to prevent CPU oversubscription when multiple
#   worktrees rebuild concurrently, and to share one target dir across them.
#   See CLAUDE.md "Use cq" section.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# CLAUDE.md hard rule
unset CLAUDECODE CLAUDE_CODE_SSE_PORT CLAUDE_CODE_ENTRYPOINT SHARE_WORKER_URL SHARE_VIEWER_URL

# Load .env.local (gitignored) for optional dev-only config such as
# SHARE_WORKER_URL. See .env.example at repo root for the template.
if [ -f "$ROOT/.env.local" ]; then
  set -a; source "$ROOT/.env.local"; set +a
fi

export CLAUDE_VIEW_SIDECAR_EXTERNAL=1
export RUST_LOG="${RUST_LOG:-warn,claude_view_server=info,claude_view_core=info}"
export VITE_PORT="${VITE_PORT:-5173}"
export SUPABASE_URL="${SUPABASE_URL:-https://iebjyftoadahqptmfcio.supabase.co}"
# Optional: share feature. Empty value disables sharing. Populate in .env.local.
export SHARE_WORKER_URL="${SHARE_WORKER_URL:-}"
export SHARE_VIEWER_URL="${SHARE_VIEWER_URL:-}"

exec cargo watch -w crates -s './scripts/cq run -p claude-view-server'
