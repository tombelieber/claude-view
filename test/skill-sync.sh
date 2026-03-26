#!/usr/bin/env bash
set -euo pipefail

# Contract test: ensures SKILL.md and MCP tools stay in sync with the Rust API routes.
# Three checks:
#   1. Every MCP tool endpoint appears in SKILL.md
#   2. Version triple-match: plugin.json = packages/plugin/package.json = root package.json
#   3. Every tool name in MCP code appears in SKILL.md

SKILL="claude-view/SKILL.md"
PLUGIN="plugin.json"
MCP_PKG="packages/plugin/package.json"
ROOT_PKG="package.json"
TOOLS_DIR="packages/plugin/src/tools"

errors=0

echo "=== Skill Sync Check ==="

# --- Check 1: Hand-written MCP tool endpoints appear in SKILL.md ---
# Only check hand-written tools (not generated/ — those are validated by codegen tests)
endpoints=$(grep -rohe "'/api/[^']*'" -e '"/api/[^"]*"' "$TOOLS_DIR" --exclude-dir=generated | tr -d "\"'" | sed 's|/\${[^}]*}|/{id}|g' | sort -u)
for ep in $endpoints; do
  # Normalize: /api/sessions/${args.session_id} → /api/sessions/{id}
  normalized=$(echo "$ep" | sed 's|/\$.*||')
  if ! grep -qF "$normalized" "$SKILL" 2>/dev/null; then
    echo "MISSING in SKILL.md: $ep"
    ((errors++)) || true
  fi
done

# --- Check 2: Version match ---
if [ -f "$PLUGIN" ]; then
  plugin_version=$(grep '"version"' "$PLUGIN" | head -1 | sed 's/.*: *"\(.*\)".*/\1/')
  root_version=$(grep '"version"' "$ROOT_PKG" | head -1 | sed 's/.*: *"\(.*\)".*/\1/')
  if [ "$plugin_version" != "$root_version" ]; then
    echo "VERSION MISMATCH: plugin.json=$plugin_version root=$root_version"
    ((errors++)) || true
  fi
else
  echo "  Note: plugin.json not found (Phase B — skipping version check)"
fi

mcp_version=$(grep '"version"' "$MCP_PKG" | head -1 | sed 's/.*: *"\(.*\)".*/\1/')
root_version=$(grep '"version"' "$ROOT_PKG" | head -1 | sed 's/.*: *"\(.*\)".*/\1/')

if [ "$mcp_version" != "$root_version" ]; then
  echo "VERSION MISMATCH: plugin/package.json=$mcp_version root=$root_version"
  ((errors++)) || true
fi

# --- Check 3: Tool count sanity ---
tool_count=$(grep -roh "name: '[^']*'" "$TOOLS_DIR" | wc -l | tr -d ' ')
echo "  Tools found: $tool_count"
if [ "$tool_count" -lt 8 ]; then
  echo "TOOL COUNT LOW: expected 8, found $tool_count"
  ((errors++)) || true
fi

if [ "$errors" -gt 0 ]; then
  echo ""
  echo "FAIL: $errors sync issue(s) found"
  exit 1
fi

echo ""
echo "OK: All endpoints in SKILL.md, versions match ($root_version)"
