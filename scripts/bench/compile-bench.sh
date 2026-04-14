#!/usr/bin/env bash
# scripts/bench/compile-bench.sh — reproducible compile benchmark
#
# Runs three compile-speed scenarios through hyperfine and saves results
# to bench/results/{label}.json for later comparison via compare.sh.
#
# Scenarios:
#   B1  touch-then-rebuild server crate  — pure incremental, link-dominated
#   B2  clean-then-check server crate    — frontend work (typeck/borrowck) only
#   B3  workspace-wide check              — total verification cost (warm cache)
#
# Usage:
#   ./scripts/bench/compile-bench.sh baseline
#   ./scripts/bench/compile-bench.sh tier1a
#   ./scripts/bench/compile-bench.sh tier1a-line-tables

set -euo pipefail

LABEL="${1:-unnamed}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

RESULTS_DIR="bench/results"
mkdir -p "$RESULTS_DIR"

STAMP="$(date +%Y%m%d-%H%M%S)"
OUT_B1="$RESULTS_DIR/${LABEL}-b1.json"
OUT_B2="$RESULTS_DIR/${LABEL}-b2.json"
OUT_B3="$RESULTS_DIR/${LABEL}-b3.json"
OUT_B4="$RESULTS_DIR/${LABEL}-b4.json"
OUT_MERGED="$RESULTS_DIR/${LABEL}.json"
LOG="$RESULTS_DIR/${LABEL}.log"

{
  echo "=== compile-bench  label=$LABEL  stamp=$STAMP ==="
  echo "host: $(sw_vers -productName) $(sw_vers -productVersion) — $(sysctl -n machdep.cpu.brand_string) — $(sysctl -n hw.ncpu) cores"
  echo "rust: $(rustc --version)"
  echo "cargo: $(cargo --version)"
  echo ""
} | tee "$LOG"

# Prime: ensure target is in a valid state before measurement
echo "Priming target..." | tee -a "$LOG"
./scripts/cq build -p claude-view-server --quiet 2>&1 | tail -3 | tee -a "$LOG"
echo "" | tee -a "$LOG"

# NOTE on noise: claude-view dev server runs at high CPU on this host,
# creating variance. We report MIN (not mean) in summary to be robust to
# contention outliers. Higher run counts further reduce variance.

# ── B1: Touch-one-file incremental rebuild (link-dominated) ──
echo "--- B1: touch crates/server/src/main.rs → build server ---" | tee -a "$LOG"
hyperfine \
  --warmup 2 \
  --runs 10 \
  --prepare 'touch crates/server/src/main.rs' \
  --export-json "$OUT_B1" \
  --command-name "B1-touch-rebuild-server" \
  './scripts/cq build -p claude-view-server --quiet' 2>&1 | tee -a "$LOG"
echo "" | tee -a "$LOG"

# ── B2: Clean server crate, cargo check (frontend work, deps cached) ──
echo "--- B2: cargo clean -p server → check server ---" | tee -a "$LOG"
hyperfine \
  --warmup 2 \
  --runs 8 \
  --prepare 'cargo clean -p claude-view-server' \
  --export-json "$OUT_B2" \
  --command-name "B2-clean-check-server" \
  './scripts/cq check -p claude-view-server --quiet' 2>&1 | tee -a "$LOG"
echo "" | tee -a "$LOG"

# ── B3: Touch foundation crate, workspace check (fanout propagation) ──
echo "--- B3: touch types/lib.rs → workspace check (fanout) ---" | tee -a "$LOG"
hyperfine \
  --warmup 2 \
  --runs 5 \
  --prepare 'touch crates/types/src/lib.rs' \
  --export-json "$OUT_B3" \
  --command-name "B3-fanout-workspace-check" \
  './scripts/cq check --workspace --quiet' 2>&1 | tee -a "$LOG"
echo "" | tee -a "$LOG"

# ── B4: Touch core/lib.rs → compile tests for core crate ──
# cq routes `test` through nextest; nextest doesn't accept --quiet, so we
# redirect stdout to keep hyperfine output clean.
echo "--- B4: touch core/lib.rs → test compile core (--no-run) ---" | tee -a "$LOG"
hyperfine \
  --warmup 2 \
  --runs 5 \
  --prepare 'touch crates/core/src/lib.rs' \
  --export-json "$OUT_B4" \
  --command-name "B4-test-compile-core" \
  './scripts/cq test -p claude-view-core --no-run > /dev/null 2>&1' 2>&1 | tee -a "$LOG"
echo "" | tee -a "$LOG"

# Merge into a single file
jq -n \
  --arg label "$LABEL" \
  --arg stamp "$STAMP" \
  --argjson b1 "$(cat "$OUT_B1")" \
  --argjson b2 "$(cat "$OUT_B2")" \
  --argjson b3 "$(cat "$OUT_B3")" \
  --argjson b4 "$(cat "$OUT_B4")" \
  '{label: $label, stamp: $stamp, b1: $b1, b2: $b2, b3: $b3, b4: $b4}' \
  > "$OUT_MERGED"

echo "=== SUMMARY: $LABEL (reporting MIN — robust to CPU contention) ===" | tee -a "$LOG"
jq -r '
  [
    ("B1 touch-rebuild-server:   min " + (.b1.results[0].min | tostring | .[0:6]) + "s  mean " + (.b1.results[0].mean | tostring | .[0:6]) + "s"),
    ("B2 clean-check-server:     min " + (.b2.results[0].min | tostring | .[0:6]) + "s  mean " + (.b2.results[0].mean | tostring | .[0:6]) + "s"),
    ("B3 workspace-check:        min " + (.b3.results[0].min | tostring | .[0:6]) + "s  mean " + (.b3.results[0].mean | tostring | .[0:6]) + "s"),
    ("B4 test-compile-core:      min " + (.b4.results[0].min | tostring | .[0:6]) + "s  mean " + (.b4.results[0].mean | tostring | .[0:6]) + "s")
  ] | .[]
' "$OUT_MERGED" | tee -a "$LOG"

echo "" | tee -a "$LOG"
echo "Saved to: $OUT_MERGED" | tee -a "$LOG"
