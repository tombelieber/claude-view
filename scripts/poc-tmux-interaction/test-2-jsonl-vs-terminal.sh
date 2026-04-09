#!/bin/bash
# ─────────────────────────────────────────────────────────────────────
# Test 2: Verify JSONL options array matches terminal rendering order
#
# Reads REAL AskUserQuestion entries from CLI JSONL files and verifies
# that the options[].label values appear in the same order in the
# terminal output (via tmux capture-pane on the mock app).
#
# This proves: JSONL options order = Ink rendering order
#
# Usage:
#   ./test-2-jsonl-vs-terminal.sh [path-to-specific-jsonl]
#
# Without args: scans ~/.claude/projects/ for JSONL with AskUserQuestion
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MOCK_DIR="$SCRIPT_DIR/mock-ink-prompt"
SESSION="poc-jsonl-verify"
PASS=0
FAIL=0
TOTAL=0

cleanup() {
  tmux kill-session -t "$SESSION" 2>/dev/null || true
  rm -f /tmp/poc-jsonl-result.txt /tmp/poc-jsonl-options.json /tmp/poc-jsonl-pane.txt
}
trap cleanup EXIT

# ── Pre-flight ───────────────────────────────────────────────────────

echo "═══════════════════════════════════════════════════════"
echo "  POC Test 2: JSONL Options vs Terminal Render Order"
echo "═══════════════════════════════════════════════════════"
echo ""

if [[ ! -f "$MOCK_DIR/dist/index.js" ]]; then
  echo "Building mock Ink prompt..."
  (cd "$MOCK_DIR" && bun install --frozen-lockfile 2>/dev/null || bun install && bun run build)
  echo ""
fi

# ── Find JSONL files with AskUserQuestion ────────────────────────────

if [[ $# -gt 0 ]]; then
  JSONL_FILES=("$@")
else
  echo "Scanning ~/.claude/projects/ for JSONL with AskUserQuestion..."
  mapfile -t JSONL_FILES < <(grep -rl '"AskUserQuestion"' ~/.claude/projects/ --include="*.jsonl" 2>/dev/null | head -10)
fi

if [[ ${#JSONL_FILES[@]} -eq 0 ]]; then
  echo "No JSONL files found with AskUserQuestion. Skipping test 2."
  exit 0
fi

echo "Found ${#JSONL_FILES[@]} JSONL file(s) with AskUserQuestion"
echo ""

# ── Extract and verify each AskUserQuestion entry ────────────────────

for jsonl_file in "${JSONL_FILES[@]}"; do
  # Extract AskUserQuestion tool_use lines with options
  while IFS= read -r line; do
    # Parse the options array from tool_input
    options_json=$(echo "$line" | python3 -c "
import sys, json
data = json.loads(sys.stdin.read())

# Navigate to tool_input — could be nested in content blocks
content = data.get('message', data).get('content', [data])
if isinstance(content, list):
    for block in content:
        if isinstance(block, dict) and block.get('type') == 'tool_use' and block.get('name') == 'AskUserQuestion':
            inp = block.get('input', {})
            questions = inp.get('questions', [])
            for q in questions:
                opts = q.get('options', [])
                if opts:
                    print(json.dumps(opts))
                    sys.exit(0)
" 2>/dev/null || true)

    if [[ -z "$options_json" ]] || [[ "$options_json" == "null" ]]; then
      continue
    fi

    # Check if options have label fields
    has_labels=$(echo "$options_json" | python3 -c "
import sys, json
opts = json.loads(sys.stdin.read())
if opts and all('label' in o for o in opts):
    print('yes')
else:
    print('no')
" 2>/dev/null || echo "no")

    if [[ "$has_labels" != "yes" ]]; then
      continue
    fi

    TOTAL=$((TOTAL + 1))

    # Extract labels in order
    mapfile -t labels < <(echo "$options_json" | python3 -c "
import sys, json
opts = json.loads(sys.stdin.read())
for o in opts:
    print(o['label'])
")

    echo "  Test $TOTAL: ${#labels[@]} options from $(basename "$jsonl_file")"
    for i in "${!labels[@]}"; do
      echo "    [$i] ${labels[$i]}"
    done

    # Start mock with these exact options
    tmux kill-session -t "$SESSION" 2>/dev/null || true
    rm -f /tmp/poc-jsonl-result.txt /tmp/poc-jsonl-pane.txt

    # Write options to temp file to avoid shell quoting issues with JSON
    echo "$options_json" > /tmp/poc-jsonl-input.json
    tmux new-session -d -s "$SESSION" \
      "RESULT_FILE=/tmp/poc-jsonl-result.txt MOCK_OPTIONS=\"\$(cat /tmp/poc-jsonl-input.json)\" node $MOCK_DIR/dist/index.js"

    sleep 1.5

    # Capture terminal output
    tmux capture-pane -t "$SESSION" -p > /tmp/poc-jsonl-pane.txt

    # Verify: do labels appear in the pane in the same order?
    prev_pos=-1
    order_correct=true
    for label in "${labels[@]}"; do
      # Find position of label in pane output (line number)
      pos=$(grep -n "$label" /tmp/poc-jsonl-pane.txt 2>/dev/null | head -1 | cut -d: -f1 || echo "0")
      if [[ "$pos" == "0" ]]; then
        echo "    ⚠ Label '$label' not found in terminal output"
        order_correct=false
        break
      fi
      if [[ "$pos" -le "$prev_pos" ]]; then
        echo "    ⚠ Label '$label' at line $pos is BEFORE previous label at line $prev_pos"
        order_correct=false
        break
      fi
      prev_pos=$pos
    done

    if $order_correct; then
      # Also verify: send Down×(last_index) + Enter → selects last option
      last_idx=$(( ${#labels[@]} - 1 ))
      keys=""
      for ((i=0; i<last_idx; i++)); do
        keys="$keys Down"
      done
      keys="$keys Enter"

      for key in $keys; do
        tmux send-keys -t "$SESSION" "$key"
        sleep 0.2
      done
      sleep 2

      if [[ -f /tmp/poc-jsonl-result.txt ]]; then
        result=$(cat /tmp/poc-jsonl-result.txt)
        expected="${labels[$last_idx]}"
        if [[ "$result" == "$expected" ]]; then
          echo "    ✓ PASS: order matches + Down×$last_idx+Enter → '$result'"
          PASS=$((PASS + 1))
        else
          echo "    ✗ FAIL: order matches but keystroke got '$result' (expected '$expected')"
          FAIL=$((FAIL + 1))
        fi
      else
        echo "    ✗ FAIL: result file not created after sending keys"
        FAIL=$((FAIL + 1))
      fi
    else
      echo "    ✗ FAIL: terminal render order doesn't match JSONL options order"
      echo "    Terminal content:"
      cat /tmp/poc-jsonl-pane.txt | sed 's/^/      /'
      FAIL=$((FAIL + 1))
    fi

    tmux kill-session -t "$SESSION" 2>/dev/null || true
    echo ""

  done < <(grep "AskUserQuestion" "$jsonl_file" 2>/dev/null | head -3)
done

# ── Summary ──────────────────────────────────────────────────────────

echo "═══════════════════════════════════════════════════════"
echo "  Results: $PASS passed, $FAIL failed, $TOTAL total"
echo "═══════════════════════════════════════════════════════"

if [[ $FAIL -gt 0 ]]; then
  exit 1
fi
