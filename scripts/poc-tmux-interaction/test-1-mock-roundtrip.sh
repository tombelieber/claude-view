#!/bin/bash
# ─────────────────────────────────────────────────────────────────────
# Test 1: Deterministic mock Ink prompt round-trip
#
# Proves: xterm.js.input() → PTY → Ink SelectInput → correct selection
# Uses a mock Ink app with known options. Fully deterministic.
#
# Prerequisites:
#   cd scripts/poc-tmux-interaction/mock-ink-prompt && bun install && bun run build
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MOCK_DIR="$SCRIPT_DIR/mock-ink-prompt"
SESSION_PREFIX="poc-test"
PASS=0
FAIL=0
TOTAL=0

cleanup() {
  tmux kill-session -t "$SESSION_PREFIX" 2>/dev/null || true
  rm -f /tmp/poc-result-*.txt /tmp/poc-options-*.json
}
trap cleanup EXIT

# ── Helpers ──────────────────────────────────────────────────────────

run_test() {
  local test_name="$1"
  local options_json="$2"
  local keys="$3"           # space-separated tmux send-keys args
  local expected_label="$4"

  TOTAL=$((TOTAL + 1))
  local result_file="/tmp/poc-result-${TOTAL}.txt"
  local options_file="/tmp/poc-options-${TOTAL}.json"
  rm -f "$result_file" "$options_file"

  local session="${SESSION_PREFIX}-${TOTAL}"

  # Start mock prompt in tmux
  tmux new-session -d -s "$session" \
    "RESULT_FILE=$result_file OPTIONS_FILE=$options_file MOCK_OPTIONS='$options_json' node $MOCK_DIR/dist/index.js"

  # Wait for Ink to render
  sleep 1.5

  # Verify the prompt is showing (capture pane)
  local pane_content
  pane_content=$(tmux capture-pane -t "$session" -p 2>/dev/null || echo "")
  if [[ -z "$pane_content" ]]; then
    echo "  ✗ FAIL [$test_name]: tmux pane empty — mock app didn't start"
    FAIL=$((FAIL + 1))
    tmux kill-session -t "$session" 2>/dev/null || true
    return
  fi

  # Send keystrokes one at a time with delay (Ink needs processing time per key)
  for key in $keys; do
    tmux send-keys -t "$session" "$key"
    sleep 0.2
  done

  # Wait for selection + file write
  sleep 2

  # Verify
  if [[ ! -f "$result_file" ]]; then
    echo "  ✗ FAIL [$test_name]: result file not created — keys didn't reach prompt"
    echo "    Pane content was:"
    echo "$pane_content" | sed 's/^/      /'
    FAIL=$((FAIL + 1))
  elif [[ "$(cat "$result_file")" == "$expected_label" ]]; then
    echo "  ✓ PASS [$test_name]: sent '$keys' → got '$(cat "$result_file")'"
    PASS=$((PASS + 1))
  else
    echo "  ✗ FAIL [$test_name]: expected '$expected_label', got '$(cat "$result_file")'"
    echo "    Keys sent: $keys"
    FAIL=$((FAIL + 1))
  fi

  tmux kill-session -t "$session" 2>/dev/null || true
}

# ── Pre-flight ───────────────────────────────────────────────────────

echo "═══════════════════════════════════════════════════════"
echo "  POC Test 1: Mock Ink Prompt Round-Trip"
echo "═══════════════════════════════════════════════════════"
echo ""

if [[ ! -f "$MOCK_DIR/dist/index.js" ]]; then
  echo "Building mock Ink prompt..."
  (cd "$MOCK_DIR" && bun install --frozen-lockfile 2>/dev/null || bun install && bun run build)
  echo ""
fi

if ! command -v tmux &>/dev/null; then
  echo "ERROR: tmux not installed"
  exit 1
fi

# ── Test Cases ───────────────────────────────────────────────────────

OPTIONS_3='[{"label":"JWT with RSA-256","description":"Asymmetric keys"},{"label":"JWT with HMAC-256","description":"Shared secret"},{"label":"OAuth 2.0 + PKCE","description":"Full OAuth flow"}]'

echo "Test matrix: 3 options, verify index-based keystroke mapping"
echo ""

# Test: Enter = select first option (index 0, default selected)
run_test "select index 0 (default)" \
  "$OPTIONS_3" \
  "Enter" \
  "JWT with RSA-256"

# Test: Down + Enter = select second option (index 1)
run_test "select index 1 (Down+Enter)" \
  "$OPTIONS_3" \
  "Down Enter" \
  "JWT with HMAC-256"

# Test: Down Down + Enter = select third option (index 2)
run_test "select index 2 (Down+Down+Enter)" \
  "$OPTIONS_3" \
  "Down Down Enter" \
  "OAuth 2.0 + PKCE"

# Test: 5 options, select last
OPTIONS_5='[{"label":"A","description":"a"},{"label":"B","description":"b"},{"label":"C","description":"c"},{"label":"D","description":"d"},{"label":"E","description":"e"}]'

run_test "select index 4 of 5 (4×Down+Enter)" \
  "$OPTIONS_5" \
  "Down Down Down Down Enter" \
  "E"

# Test: Down then Up (navigate then go back) = still index 0
run_test "Down+Up+Enter = back to index 0" \
  "$OPTIONS_3" \
  "Down Up Enter" \
  "JWT with RSA-256"

# ── Summary ──────────────────────────────────────────────────────────

echo ""
echo "═══════════════════════════════════════════════════════"
echo "  Results: $PASS passed, $FAIL failed, $TOTAL total"
echo "═══════════════════════════════════════════════════════"

if [[ $FAIL -gt 0 ]]; then
  exit 1
fi
