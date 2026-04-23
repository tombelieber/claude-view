#!/usr/bin/env bash
# Rehearsal gate for CQRS Phase 7.h (E.5) — DROP `sessions` table.
#
# Runs as a TDD gate: fails BEFORE the migration, passes AFTER all
# production writers + readers have moved to `session_stats` +
# `session_flags`. Test helpers and migration files are allowed to
# keep touching `sessions` until E.5.h4 rewrites them.
#
# Usage: ./scripts/rehearse-phase-e5.sh
# Exits non-zero and prints offending sites when the gate trips.

set -euo pipefail

fails=0

# Gate 1 — zero production INSERT/UPDATE/DELETE on `sessions`.
writes=$(grep -rn '\b\(INSERT INTO\|UPDATE\|DELETE FROM\) sessions\b' crates/ \
    --include='*.rs' 2>/dev/null |
    grep -v -E '/tests/|_test\.rs|tests\.rs|src/migrations/|/examples/' || true)
if [ -n "$writes" ]; then
    echo "FAIL: production writes to sessions still present:" >&2
    echo "$writes" >&2
    fails=$((fails + 1))
fi

# Gate 2 — zero `FROM sessions` in production code (migrations + tests allowed).
reads=$(grep -rn '\bFROM sessions\b' crates/ \
    --include='*.rs' 2>/dev/null |
    grep -v -E '/tests/|_test\.rs|tests\.rs|src/migrations/|/examples/' || true)
if [ -n "$reads" ]; then
    echo "FAIL: reads from sessions still present:" >&2
    echo "$reads" >&2
    fails=$((fails + 1))
fi

if [ "$fails" -eq 0 ]; then
    echo "OK: rehearse-phase-e5 clean"
    exit 0
fi

echo "" >&2
echo "Summary: $fails gate(s) failed — see above." >&2
exit 1
