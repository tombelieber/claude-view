#!/usr/bin/env bash
# Rehearses CQRS Phase 5.6 — verifies readers are off legacy columns
# BEFORE migration 85 drops them. Exit 0 = safe to proceed.
set -euo pipefail

echo "== Phase 5.6 rehearsal =="

echo "[1/3] grep for sessions.archived_at / category_* / classified_at readers..."
# Match only lines that USE the legacy columns (SQL refs or dot-access),
# ignoring lines where the identifier appears only inside a Rust `//` or
# `///` comment. Phase 5.5 landed the shadow-only reader; the comments
# are documentation and must not block the rehearsal.
LEAKS=$(grep -rn "sessions\.archived_at\|sessions\.category_l1\|sessions\.category_l2\|sessions\.category_l3\|sessions\.category_confidence\|sessions\.category_source\|sessions\.classified_at" crates/server/ crates/db/src/ \
  | grep -vE '^\s*[^:]+:[0-9]+:\s*//' \
  | grep -vE '^\s*[^:]+:[0-9]+:.*//.*(archived_at|category_l1|category_l2|category_l3|category_confidence|category_source|classified_at)' \
  || true)
if [[ -n "$LEAKS" ]]; then
  echo "FAIL: legacy column readers still present:"
  echo "$LEAKS"
  exit 1
fi
echo "OK — no legacy readers"

echo "[2/3] running deterministic parity..."
./scripts/cq test -p claude-view-db --test deterministic_parity_test -- --nocapture

echo "[3/3] running full workspace tests..."
./scripts/cq test --workspace

echo "== REHEARSAL GREEN — safe to run migration 85 =="
