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

export CLAUDE_VIEW_SIDECAR_EXTERNAL=1
export RUST_LOG="${RUST_LOG:-warn,claude_view_server=info,claude_view_core=info}"
export VITE_PORT="${VITE_PORT:-5173}"
export SUPABASE_URL="${SUPABASE_URL:-https://iebjyftoadahqptmfcio.supabase.co}"
export SHARE_WORKER_URL="https://claude-view-share-worker-dev.vickyai-tech.workers.dev"
export SHARE_VIEWER_URL="https://claude-view-share-viewer-dev.pages.dev"

exec cargo watch -w crates -s './scripts/cq run -p claude-view-server'
