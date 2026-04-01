#!/usr/bin/env bash
set -euo pipefail

PLUGIN_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ERRORS=0

check() {
  if [ ! -e "$PLUGIN_ROOT/$1" ]; then
    echo "FAIL: missing $1"
    ERRORS=$((ERRORS + 1))
  else
    echo "  OK: $1"
  fi
}

check_json() {
  # --input-type=commonjs: required because package.json has "type": "module"
  # which makes node -e evaluate as ESM where require() is undefined
  if ! node --input-type=commonjs -e "JSON.parse(require('fs').readFileSync('$PLUGIN_ROOT/$1','utf8'))" 2>/dev/null; then
    echo "FAIL: invalid JSON in $1"
    ERRORS=$((ERRORS + 1))
  else
    echo "  OK: $1 (valid JSON)"
  fi
}

echo "=== claude-view plugin validation ==="
echo ""

echo "--- Required files ---"
check ".claude-plugin/plugin.json"
check ".mcp.json"
check "hooks/hooks.json"
check "hooks/start-server.sh"
check "src/index.ts"
check "skills/session-recap/SKILL.md"
check "skills/daily-cost/SKILL.md"
check "skills/standup/SKILL.md"
check "skills/insights/SKILL.md"
check "skills/team-status/SKILL.md"
check "skills/search/SKILL.md"
check "skills/export-data/SKILL.md"
check "skills/coaching/SKILL.md"
check "skills/project-overview/SKILL.md"
check "README.md"

echo ""
echo "--- JSON validation ---"
check_json ".claude-plugin/plugin.json"
check_json ".mcp.json"
check_json "hooks/hooks.json"

echo ""
echo "--- Executable check ---"
if [ -x "$PLUGIN_ROOT/hooks/start-server.sh" ]; then
  echo "  OK: hooks/start-server.sh is executable"
else
  echo "FAIL: hooks/start-server.sh is not executable"
  ERRORS=$((ERRORS + 1))
fi

echo ""
if [ "$ERRORS" -eq 0 ]; then
  echo "All checks passed."
else
  echo "$ERRORS check(s) failed."
  exit 1
fi
