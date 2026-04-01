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
#   SKIP_STORYBOOK=1  Skip Storybook build check
#   SKIP_INTEGRITY=1  Skip integrity gates (parser/indexer/replay)

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

TOTAL_START=$(date +%s)
GATE=0

gate() {
  GATE=$((GATE + 1))
  echo ""
  echo "--- [$GATE/8] $1 ---"
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

# ── 7. Storybook build ──
gate "Storybook build"
if [ "${SKIP_STORYBOOK:-0}" != "1" ]; then
  S=$(date +%s)
  (cd apps/web && bunx storybook build -o /tmp/storybook-ci-check --quiet 2>&1)
  rm -rf /tmp/storybook-ci-check
  elapsed $S
else echo "  SKIP (SKIP_STORYBOOK=1)"; fi

# ── 8. Integrity gates (parser/indexer/replay) ──
gate "Integrity gates"
if [ "${SKIP_INTEGRITY:-0}" != "1" ]; then
  S=$(date +%s); ./scripts/integrity/ci-gates.sh; elapsed $S
else echo "  SKIP (SKIP_INTEGRITY=1)"; fi

echo ""
echo "=== Local CI: ALL GATES PASSED ($(( $(date +%s) - TOTAL_START ))s) ==="
