#!/usr/bin/env bash
# scripts/bench/compare.sh — diff two benchmark runs
#
# Usage:
#   ./scripts/bench/compare.sh baseline tier1a
#   ./scripts/bench/compare.sh baseline tier2b
#
# Reports wall-clock delta and percentage for each scenario.

set -euo pipefail

BEFORE_LABEL="${1:?usage: compare.sh BEFORE AFTER}"
AFTER_LABEL="${2:?usage: compare.sh BEFORE AFTER}"

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BEFORE="$ROOT/bench/results/${BEFORE_LABEL}.json"
AFTER="$ROOT/bench/results/${AFTER_LABEL}.json"

[ -f "$BEFORE" ] || { echo "missing: $BEFORE" >&2; exit 1; }
[ -f "$AFTER" ] || { echo "missing: $AFTER" >&2; exit 1; }

printf "\n=== %s → %s ===\n\n" "$BEFORE_LABEL" "$AFTER_LABEL"

jq -n \
  --argjson before "$(cat "$BEFORE")" \
  --argjson after "$(cat "$AFTER")" \
  '
  def fmt($v): ($v | tostring | .[0:6]);
  def pct($a; $b):
    (($b - $a) / $a * 100) as $p |
    if $p < 0 then "\($p | tostring | .[0:5])%" else "+\($p | tostring | .[0:5])%" end;
  def row($name; $a; $b):
    "\($name):  min \(fmt($a))s → \(fmt($b))s   \(pct($a; $b))";
  # Compare MIN (not mean) — robust to CPU contention outliers
  [
    row("B1 touch-rebuild-server  "; $before.b1.results[0].min; $after.b1.results[0].min),
    row("B2 clean-check-server    "; $before.b2.results[0].min; $after.b2.results[0].min),
    row("B3 workspace-check       "; $before.b3.results[0].min; $after.b3.results[0].min),
    row("B4 test-compile-core     "; ($before.b4.results[0].min // 0); ($after.b4.results[0].min // 0))
  ] | .[]
  ' -r

printf "\n"
