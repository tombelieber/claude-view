#!/bin/bash
set -euo pipefail

# Local CI — all quality gates for this machine.
# GitHub Actions only handles cross-platform build + npm publish.
#
# Usage:
#   ./scripts/ci-local.sh              # run all gates
#   SKIP_RUST=1 ./scripts/ci-local.sh  # skip Rust gates
#
# Skip flags:
#   SKIP_RUST=1       Skip clippy + cargo test
#   SKIP_TS=1         Skip lint + typecheck + test
#   SKIP_EVIDENCE=1   Skip evidence audit (JSONL schema guard)
#   SKIP_PIPELINE=1   Skip block pipeline drift check
#   SKIP_STORYBOOK=1  Skip Storybook build check
#   SKIP_INTEGRITY=1  Skip integrity gates (parser/indexer/replay)
#   SKIP_ACTIONLINT=1 Skip GitHub Actions workflow lint (requires `brew install actionlint`)

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

TOTAL_START=$(date +%s)
GATE=0

gate() {
  GATE=$((GATE + 1))
  echo ""
  echo "--- [$GATE/10] $1 ---"
}

elapsed() {
  echo "  OK ($(( $(date +%s) - $1 ))s)"
}

echo "=== Local CI ==="

# ── 1. TypeScript lint (fastest — biome) ──
gate "TypeScript lint"
if [ "${SKIP_TS:-0}" != "1" ]; then
  S=$(date +%s); bun run lint; elapsed $S
else echo "  SKIP (SKIP_TS=1)"; fi

# ── 2. TypeScript typecheck ──
gate "TypeScript typecheck"
if [ "${SKIP_TS:-0}" != "1" ]; then
  S=$(date +%s); bun run typecheck; elapsed $S
else echo "  SKIP (SKIP_TS=1)"; fi

# ── 3. TypeScript tests ──
gate "TypeScript tests"
if [ "${SKIP_TS:-0}" != "1" ]; then
  S=$(date +%s); bun run test; elapsed $S
else echo "  SKIP (SKIP_TS=1)"; fi

# ── 4. Rust lint (clippy) ──
gate "Rust lint (clippy)"
if [ "${SKIP_RUST:-0}" != "1" ]; then
  S=$(date +%s); ./scripts/cq clippy --workspace -- -D warnings; elapsed $S
else echo "  SKIP (SKIP_RUST=1)"; fi

# ── 5. Rust tests ──
gate "Rust tests"
if [ "${SKIP_RUST:-0}" != "1" ]; then
  S=$(date +%s); ./scripts/cq test --workspace; elapsed $S
else echo "  SKIP (SKIP_RUST=1)"; fi

# ── 6. Evidence audit (JSONL schema guard) ──
gate "Evidence audit"
if [ "${SKIP_EVIDENCE:-0}" != "1" ]; then
  S=$(date +%s); ./scripts/cq run -p claude-view-core --bin evidence-audit --release; elapsed $S
else echo "  SKIP (SKIP_EVIDENCE=1)"; fi

# ── 7. Block pipeline drift check ──
gate "Block pipeline drift check"
if [ "${SKIP_PIPELINE:-0}" != "1" ]; then
  S=$(date +%s)
  AUDIT_OUT=$(mktemp)
  bash scripts/integrity/block-pipeline-audit.sh --output "$AUDIT_OUT" --max-files 300 2>&1 | tail -3
  NEW_TYPES=$(jq -r '.cross_layer_gaps.new_unhandled_types // [] | length' "$AUDIT_OUT" 2>/dev/null || echo "0")
  rm -f "$AUDIT_OUT"
  if [ "$NEW_TYPES" != "0" ]; then
    echo "FAIL: $NEW_TYPES new unhandled JSONL types found — run /skill block-pipeline-audit to close gaps" >&2
    exit 1
  fi
  # Also run the invariant tests that check baseline ↔ parser parity
  ./scripts/cq test -p claude-view-core --test block_accumulator_invariant_test 2>&1 | tail -5
  elapsed $S
else echo "  SKIP (SKIP_PIPELINE=1)"; fi

# ── 8. Storybook build ──
gate "Storybook build"
if [ "${SKIP_STORYBOOK:-0}" != "1" ]; then
  S=$(date +%s)
  (cd apps/web && bunx storybook build -o /tmp/storybook-ci-check --quiet 2>&1)
  rm -rf /tmp/storybook-ci-check
  elapsed $S
else echo "  SKIP (SKIP_STORYBOOK=1)"; fi

# ── 9. Integrity gates (parser/indexer/replay) ──
gate "Integrity gates"
if [ "${SKIP_INTEGRITY:-0}" != "1" ]; then
  S=$(date +%s); ./scripts/integrity/ci-gates.sh; elapsed $S
else echo "  SKIP (SKIP_INTEGRITY=1)"; fi

# ── 10. GitHub Actions workflow lint (actionlint) ──
gate "Actionlint (.github/workflows/*.yml)"
if [ "${SKIP_ACTIONLINT:-0}" != "1" ]; then
  if ! command -v actionlint >/dev/null 2>&1; then
    echo "  FAIL: actionlint not installed" >&2
    echo "  Install: brew install actionlint" >&2
    echo "  Or skip: SKIP_ACTIONLINT=1 $0" >&2
    exit 1
  fi
  S=$(date +%s); actionlint -color; elapsed $S
else echo "  SKIP (SKIP_ACTIONLINT=1)"; fi

echo ""
echo "=== Local CI: ALL 10 GATES PASSED ($(( $(date +%s) - TOTAL_START ))s) ==="
