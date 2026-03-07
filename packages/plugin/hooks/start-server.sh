#!/usr/bin/env bash
# claude-view SessionStart hook — ensure server is running
# Non-blocking: ALWAYS exits 0, even if server fails to start.
# Uses set -uo pipefail (no -e) so the script never aborts early.

set -uo pipefail

PORT="${CLAUDE_VIEW_PORT:-47892}"
HEALTH_URL="http://localhost:${PORT}/api/health"

# Check if already running (curl inside if-guard: safe with any set flags)
if curl -sf --max-time 2 "${HEALTH_URL}" >/dev/null 2>&1; then
  exit 0
fi

# Locate npx — version managers (nvm, volta, fnm, asdf) put it outside system PATH.
# Claude Code hooks run with limited env, so we search common locations.
find_npx() {
  # 1. Already in PATH?
  command -v npx 2>/dev/null && return
  # 2. Common version manager locations
  for candidate in \
    "${HOME}/.volta/bin/npx" \
    "${HOME}/.local/share/fnm/aliases/default/bin/npx" \
    "/usr/local/bin/npx" \
    "/opt/homebrew/bin/npx"; do
    [ -x "$candidate" ] && echo "$candidate" && return
  done
  # 3. nvm — glob for the default version
  for candidate in "${HOME}"/.nvm/versions/node/*/bin/npx; do
    [ -x "$candidate" ] && echo "$candidate" && return
  done
}

NPX="$(find_npx)"
if [ -z "$NPX" ]; then
  # npx not found — cannot auto-start server. Tools will show a clear error.
  exit 0
fi

# Not running — start in background.
# CLAUDE_VIEW_NO_OPEN=1 suppresses browser tab open from the hook context.
# || true ensures nohup failure (e.g. binary download timeout) never propagates.
CLAUDE_VIEW_NO_OPEN=1 nohup "$NPX" claude-view >/dev/null 2>&1 &

# Wait up to 3 seconds for server to become healthy.
# On first-ever run, binary download takes 5-30s — this will time out. That's fine:
# the server starts in the background and will be ready for the next tool call or session.
for _ in 1 2 3; do
  sleep 1
  if curl -sf --max-time 1 "${HEALTH_URL}" >/dev/null 2>&1; then
    exit 0
  fi
done

# Server didn't start in time — don't block the session.
exit 0
