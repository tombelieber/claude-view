#!/bin/bash
# ─────────────────────────────────────────────────────────────────────
# POC: Tmux Terminal Interaction — Run All Tests
#
# Test 1: Mock Ink prompt round-trip (deterministic)
#   Proves: keystroke index → correct option selection
#
# Test 2: JSONL options vs terminal render order
#   Proves: real CLI JSONL options array = Ink rendering order
#
# Test 3: Multi-client tmux attach
#   Proves: same session viewable/controllable from multiple clients
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo ""
echo "╔═══════════════════════════════════════════════════════╗"
echo "║  POC: Tmux Terminal Interaction — Full Test Suite     ║"
echo "╚═══════════════════════════════════════════════════════╝"
echo ""

# Build mock app first
MOCK_DIR="$SCRIPT_DIR/mock-ink-prompt"
if [[ ! -f "$MOCK_DIR/dist/index.js" ]]; then
  echo "Building mock Ink prompt..."
  (cd "$MOCK_DIR" && bun install --frozen-lockfile 2>/dev/null || bun install && bun run build)
  echo ""
fi

TOTAL_PASS=0
TOTAL_FAIL=0

run_suite() {
  local script="$1"
  local name="$2"
  echo ""
  echo "────────────────────────────────────────────────────"
  echo "  Running: $name"
  echo "────────────────────────────────────────────────────"

  if bash "$script"; then
    echo "  → Suite passed ✓"
  else
    echo "  → Suite FAILED ✗"
    TOTAL_FAIL=$((TOTAL_FAIL + 1))
  fi
  TOTAL_PASS=$((TOTAL_PASS + 1))
}

run_suite "$SCRIPT_DIR/test-1-mock-roundtrip.sh" "Test 1: Mock Ink Round-Trip"
run_suite "$SCRIPT_DIR/test-2-jsonl-vs-terminal.sh" "Test 2: JSONL vs Terminal Order"
run_suite "$SCRIPT_DIR/test-3-multi-client.sh" "Test 3: Multi-Client Attach"

echo ""
echo "╔═══════════════════════════════════════════════════════╗"
if [[ $TOTAL_FAIL -eq 0 ]]; then
  echo "║  ALL SUITES PASSED ✓                                ║"
else
  echo "║  $TOTAL_FAIL SUITE(S) FAILED ✗                             ║"
fi
echo "╚═══════════════════════════════════════════════════════╝"
echo ""

if [[ $TOTAL_FAIL -gt 0 ]]; then
  exit 1
fi
