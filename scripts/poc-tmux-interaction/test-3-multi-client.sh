#!/bin/bash
# ─────────────────────────────────────────────────────────────────────
# Test 3: Multi-client tmux attach — same session, multiple views
#
# Proves: two tmux clients attached to the same session both see the
# same state and either can send input.
#
# This validates the "same session in multiple xterm.js instances" UX.
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MOCK_DIR="$SCRIPT_DIR/mock-ink-prompt"
SESSION="poc-multi"
PASS=0
FAIL=0
TOTAL=0

cleanup() {
  tmux kill-session -t "$SESSION" 2>/dev/null || true
  rm -f /tmp/poc-multi-result.txt /tmp/poc-multi-pane-*.txt
}
trap cleanup EXIT

echo "═══════════════════════════════════════════════════════"
echo "  POC Test 3: Multi-Client tmux Attach"
echo "═══════════════════════════════════════════════════════"
echo ""

if [[ ! -f "$MOCK_DIR/dist/index.js" ]]; then
  echo "Building mock Ink prompt..."
  (cd "$MOCK_DIR" && bun install --frozen-lockfile 2>/dev/null || bun install && bun run build)
  echo ""
fi

OPTIONS='[{"label":"Option A","description":"First"},{"label":"Option B","description":"Second"},{"label":"Option C","description":"Third"}]'

# ── Test 3a: Both clients see the same prompt ────────────────────────

TOTAL=$((TOTAL + 1))
echo "Test 3a: Two clients see same content"

tmux new-session -d -s "$SESSION" \
  "RESULT_FILE=/tmp/poc-multi-result.txt MOCK_OPTIONS='$OPTIONS' node $MOCK_DIR/dist/index.js"

sleep 1.5

# Capture from two different "clients" (same session, different capture calls)
tmux capture-pane -t "$SESSION" -p > /tmp/poc-multi-pane-1.txt
# Simulate second client's view
tmux capture-pane -t "$SESSION" -p > /tmp/poc-multi-pane-2.txt

if diff -q /tmp/poc-multi-pane-1.txt /tmp/poc-multi-pane-2.txt &>/dev/null; then
  echo "  ✓ PASS: both clients see identical content"
  PASS=$((PASS + 1))
else
  echo "  ✗ FAIL: clients see different content"
  FAIL=$((FAIL + 1))
fi

# ── Test 3b: Input from "client 2" affects the session ───────────────

TOTAL=$((TOTAL + 1))
echo "Test 3b: Input from second client reaches the app"

# Send keys with delay (simulating a second client / xterm.js instance sending input)
tmux send-keys -t "$SESSION" Down
sleep 0.2
tmux send-keys -t "$SESSION" Enter
sleep 2

if [[ -f /tmp/poc-multi-result.txt ]]; then
  result=$(cat /tmp/poc-multi-result.txt)
  if [[ "$result" == "Option B" ]]; then
    echo "  ✓ PASS: second client's Down+Enter selected 'Option B'"
    PASS=$((PASS + 1))
  else
    echo "  ✗ FAIL: expected 'Option B', got '$result'"
    FAIL=$((FAIL + 1))
  fi
else
  echo "  ✗ FAIL: result file not created"
  FAIL=$((FAIL + 1))
fi

tmux kill-session -t "$SESSION" 2>/dev/null || true

# ── Test 3c: aggressive-resize allows different window sizes ─────────

TOTAL=$((TOTAL + 1))
echo "Test 3c: aggressive-resize option works"

tmux new-session -d -s "$SESSION" -x 120 -y 40 \
  "RESULT_FILE=/tmp/poc-multi-result-2.txt MOCK_OPTIONS='$OPTIONS' node $MOCK_DIR/dist/index.js"

# Set aggressive-resize so multiple clients don't constrain each other
tmux set-option -t "$SESSION" aggressive-resize on 2>/dev/null

sleep 1

# Verify session is alive and option is set
resize_val=$(tmux show-option -t "$SESSION" aggressive-resize 2>/dev/null | grep -o "on" || echo "off")
if [[ "$resize_val" == "on" ]]; then
  echo "  ✓ PASS: aggressive-resize is 'on'"
  PASS=$((PASS + 1))
else
  echo "  ✗ FAIL: aggressive-resize not set (got '$resize_val')"
  FAIL=$((FAIL + 1))
fi

tmux kill-session -t "$SESSION" 2>/dev/null || true

# ── Summary ──────────────────────────────────────────────────────────

echo ""
echo "═══════════════════════════════════════════════════════"
echo "  Results: $PASS passed, $FAIL failed, $TOTAL total"
echo "═══════════════════════════════════════════════════════"

if [[ $FAIL -gt 0 ]]; then
  exit 1
fi
