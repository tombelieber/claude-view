#!/bin/bash
# ============================================================================
# claude-view DACS Setup — Run OUTSIDE the sandbox
# ============================================================================
#
# This script prepares a portable claude-view installation that can be
# copied into an enterprise DACS sandbox (where ~/.claude/ and ~/.cache/
# are read-only).
#
# What it does:
#   1. Downloads the claude-view release binary + frontend + sidecar
#   2. Wires hooks into ~/.claude/settings.json (your normal machine)
#   3. Creates the statusline wrapper script
#   4. Packages everything into install-claude-view/ ready to copy
#
# Usage:
#   chmod +x scripts/dacs-setup-outside.sh
#   ./scripts/dacs-setup-outside.sh [--port 8080] [--sidecar-port 4001]
#
# After running:
#   cp -r install-claude-view/ <your-dacs-mount>/
# ============================================================================

set -euo pipefail

# ── Defaults ──
PORT="${1:-47892}"
SIDECAR_PORT="${2:-3001}"
VERSION=$(node -e "console.log(require('./package.json').version)" 2>/dev/null || echo "0.26.0")
INSTALL_DIR="install-claude-view"
SENTINEL="# claude-view-hook"
STATUSLINE_SENTINEL="# claude-view-statusline"

# Parse flags
while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="$2"; shift 2 ;;
    --sidecar-port) SIDECAR_PORT="$2"; shift 2 ;;
    *) shift ;;
  esac
done

echo "=== claude-view DACS Setup (Outside Sandbox) ==="
echo "  Version:       $VERSION"
echo "  Server port:   $PORT"
echo "  Sidecar port:  $SIDECAR_PORT"
echo "  Install dir:   $INSTALL_DIR/"
echo ""

# ── Step 1: Build or locate the binary + assets ──
echo "[1/4] Building claude-view..."

# Build sidecar
echo "  Building sidecar..."
(cd sidecar && (bun install --frozen-lockfile 2>/dev/null || bun install) && bun run build) || {
  echo "ERROR: Failed to build sidecar. Run from the repo root."
  exit 1
}

# Build frontend
echo "  Building frontend..."
turbo build --filter=@claude-view/web || {
  echo "ERROR: Failed to build frontend."
  exit 1
}

# Build Rust binary (release mode)
echo "  Building server binary..."
cargo build -p claude-view-server --release || {
  echo "ERROR: Failed to build server binary."
  exit 1
}

# ── Step 2: Package into install dir ──
echo "[2/4] Packaging into $INSTALL_DIR/..."
rm -rf "$INSTALL_DIR"
mkdir -p "$INSTALL_DIR"/{data,sidecar/dist}

# Binary
cp target/release/claude-view "$INSTALL_DIR/claude-view"

# Frontend
cp -r apps/web/dist "$INSTALL_DIR/dist"

# Sidecar
cp -r sidecar/dist/* "$INSTALL_DIR/sidecar/dist/"
cp sidecar/package.json "$INSTALL_DIR/sidecar/"
# Sidecar node_modules needed at runtime
if [ -d sidecar/node_modules ]; then
  cp -r sidecar/node_modules "$INSTALL_DIR/sidecar/node_modules"
fi

# ── Step 3: Wire hooks into ~/.claude/settings.json ──
echo "[3/4] Registering hooks in ~/.claude/settings.json..."

SETTINGS_FILE="$HOME/.claude/settings.json"
mkdir -p "$(dirname "$SETTINGS_FILE")"

# Read existing settings or create empty
if [ -f "$SETTINGS_FILE" ]; then
  SETTINGS=$(cat "$SETTINGS_FILE")
else
  SETTINGS='{}'
fi

# Remove any existing claude-view hooks (by sentinel)
SETTINGS=$(echo "$SETTINGS" | node -e "
const fs = require('fs');
const input = fs.readFileSync('/dev/stdin', 'utf8');
const settings = JSON.parse(input || '{}');
const sentinel = '$SENTINEL';

if (settings.hooks) {
  for (const [event, groups] of Object.entries(settings.hooks)) {
    if (Array.isArray(groups)) {
      settings.hooks[event] = groups.filter(g => {
        const hooks = g.hooks || [];
        return !hooks.some(h => (h.command || '').includes(sentinel));
      });
    }
  }
}
console.log(JSON.stringify(settings, null, 2));
")

# Add our hooks
HOOK_EVENTS='SessionStart UserPromptSubmit PreToolUse PostToolUse PostToolUseFailure PermissionRequest Stop Notification SubagentStart SubagentStop TeammateIdle TaskCompleted PreCompact SessionEnd'

SETTINGS=$(echo "$SETTINGS" | node -e "
const fs = require('fs');
const input = fs.readFileSync('/dev/stdin', 'utf8');
const settings = JSON.parse(input || '{}');
if (!settings.hooks) settings.hooks = {};

const events = '$HOOK_EVENTS'.split(' ');
const port = $PORT;
const sentinel = '$SENTINEL';

for (const event of events) {
  const command = \`curl -s -X POST http://localhost:\${port}/api/live/hook -H 'Content-Type: application/json' -H 'X-Claude-PID: '\\\$PPID --data-binary @- 2>/dev/null || true \${sentinel}\`;
  const handler = { type: 'command', command, statusMessage: 'Live Monitor' };
  if (event !== 'SessionStart') handler.async = true;

  if (!settings.hooks[event]) settings.hooks[event] = [];
  settings.hooks[event].push({ hooks: [handler] });
}

console.log(JSON.stringify(settings, null, 2));
")

echo "$SETTINGS" > "$SETTINGS_FILE"
echo "  Wrote $(echo "$HOOK_EVENTS" | wc -w | tr -d ' ') hooks to $SETTINGS_FILE"

# ── Step 4: Create statusline wrapper ──
echo "[4/4] Creating statusline wrapper..."

WRAPPER_DIR="$HOME/.cache/claude-view"
mkdir -p "$WRAPPER_DIR"
WRAPPER_PATH="$WRAPPER_DIR/statusline-wrapper.sh"

# Check if user has an existing statusline command
EXISTING_STATUSLINE=$(echo "$SETTINGS" | node -e "
const fs = require('fs');
const s = JSON.parse(fs.readFileSync('/dev/stdin', 'utf8'));
const cmd = s?.statusLine?.command || '';
const sentinel = '$STATUSLINE_SENTINEL';
// Don't return our own wrapper as 'existing'
if (cmd.includes(sentinel)) console.log('');
else console.log(cmd);
" 2>/dev/null || echo "")

cat > "$WRAPPER_PATH" << 'WRAPPER_EOF'
#!/bin/sh
input=$(cat)
session_id=$(printf '%s' "$input" | jq -r '.session_id // empty' 2>/dev/null)
WRAPPER_EOF

# Add the curl POST
cat >> "$WRAPPER_PATH" << WRAPPER_CURL
[ -n "\$session_id" ] && printf '%s' "\$input" | \\
  curl -s -X POST "http://localhost:${PORT}/api/live/statusline" \\
    -H 'Content-Type: application/json' \\
    -H "X-Claude-Pid: \$PPID" \\
    --data-binary @- \\
    > /dev/null 2>&1 & ${STATUSLINE_SENTINEL}
WRAPPER_CURL

# If user had an existing statusline command, pipe to it too
if [ -n "$EXISTING_STATUSLINE" ]; then
  ESCAPED=$(echo "$EXISTING_STATUSLINE" | sed "s/'/'\\\\''/g")
  echo "printf '%s' \"\$input\" | sh -c '$ESCAPED'" >> "$WRAPPER_PATH"
fi

chmod +x "$WRAPPER_PATH"

# Update settings.json with statusline wrapper path
SETTINGS=$(cat "$SETTINGS_FILE" | node -e "
const fs = require('fs');
const s = JSON.parse(fs.readFileSync('/dev/stdin', 'utf8'));
if (!s.statusLine) s.statusLine = {};
s.statusLine.command = '$WRAPPER_PATH';
console.log(JSON.stringify(s, null, 2));
")
echo "$SETTINGS" > "$SETTINGS_FILE"
echo "  Statusline wrapper: $WRAPPER_PATH"

# ── Step 5: Create the DACS start script ──
cat > "$INSTALL_DIR/start.sh" << START_EOF
#!/bin/bash
# claude-view DACS start script — run INSIDE the sandbox
# Generated by dacs-setup-outside.sh

set -euo pipefail
DIR="\$(cd "\$(dirname "\$0")" && pwd)"

export CLAUDE_VIEW_PORT=${PORT}
export SIDECAR_PORT=${SIDECAR_PORT}
export CLAUDE_VIEW_DATA_DIR="\$DIR/data"
export CLAUDE_VIEW_SKIP_HOOKS=1
export CLAUDE_VIEW_TELEMETRY=0
export STATIC_DIR="\$DIR/dist"
export SIDECAR_DIR="\$DIR/sidecar"
export RUST_LOG=warn,claude_view_server=info

echo "Starting claude-view (DACS mode)..."
echo "  Port:     ${PORT}"
echo "  Data dir: \$DIR/data"
echo "  Hooks:    pre-configured (outside sandbox)"
echo ""

exec "\$DIR/claude-view"
START_EOF

chmod +x "$INSTALL_DIR/start.sh"

echo ""
echo "=== Done ==="
echo ""
echo "Installation packaged in: $INSTALL_DIR/"
echo ""
echo "Next steps:"
echo "  1. Copy $INSTALL_DIR/ into your DACS sandbox"
echo "  2. Inside DACS, run: ./start.sh"
echo "  3. Open http://localhost:$PORT"
echo ""
echo "Contents:"
ls -la "$INSTALL_DIR/"
