#!/usr/bin/env bash
# CQRS PR 6.4 rehearsal gate — run BEFORE committing migration 87 (DROP
# turns + invocations). Greps the source tree for any remaining readers
# or writers of the soon-to-be-dropped tables; fails hard on leakage.
#
# The check excludes migrations/ (legacy DROP / backfill statements are
# allowed to mention the retired tables), tests/ and test support (fixture
# setup still references them until the surrounding tests are updated),
# and the `indexer_parallel` parse phase where the parser still produces
# RawTurn / RawInvocation Rust structs even though the DB write path is
# gone.

set -euo pipefail

echo "== Phase 6.4 rehearsal =="
FAIL=0
for table in turns invocations; do
  LEAKS=$(grep -rn "FROM $table\b\|from $table\b\|INSERT INTO $table\b\|DELETE FROM $table\b" \
      crates/db/src/ crates/server/src/ \
    | grep -v migrations/ \
    | grep -v tests \
    | grep -v test_support \
    | grep -v "batch_insert_${table}" \
    || true)
  if [[ -n "$LEAKS" ]]; then
    echo "FAIL: $table still has readers/writers:"
    echo "$LEAKS"
    FAIL=1
  fi
done

if [[ $FAIL -ne 0 ]]; then
  echo
  echo "REHEARSAL FAILED — migrate the offending readers/writers before"
  echo "committing migration 87."
  exit 1
fi

echo "OK — safe to DROP turns + invocations"
