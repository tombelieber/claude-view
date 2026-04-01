#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${ROOT_DIR}"

ARTIFACT_DIR="artifacts/integrity"
mkdir -p "${ARTIFACT_DIR}"

require_cmd() {
  local cmd="$1"
  command -v "${cmd}" >/dev/null 2>&1 || {
    echo "Missing required command: ${cmd}" >&2
    exit 1
  }
}

run_gate() {
  local gate_name="$1"
  local log_file="$2"
  shift 2

  {
    echo "[${gate_name}] START"
    "$@"
    echo "[${gate_name}] OK"
  } 2>&1 | tee "${log_file}"
}

require_cmd cargo
require_cmd rg
require_cmd bash
require_cmd jq
if ! command -v sha256sum >/dev/null 2>&1 && ! command -v shasum >/dev/null 2>&1; then
  echo "Missing required checksum tool: sha256sum or shasum" >&2
  exit 1
fi

run_gate "Gate 0" "${ARTIFACT_DIR}/gate0-presence.log" bash -euo pipefail -c '
command -v cargo >/dev/null 2>&1
command -v rg >/dev/null 2>&1
command -v bash >/dev/null 2>&1
command -v jq >/dev/null 2>&1
command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1

test -f scripts/integrity/static-regression-checks.sh && test -x scripts/integrity/static-regression-checks.sh
test -f scripts/integrity/replay.sh && test -x scripts/integrity/replay.sh
test -f scripts/integrity/assert-replay-thresholds.sh && test -x scripts/integrity/assert-replay-thresholds.sh
test -f scripts/integrity/ci-gates.sh && test -x scripts/integrity/ci-gates.sh

test -d crates/db/tests/integrity_corpus_snapshot
test -s crates/db/tests/integrity_corpus_snapshot/manifest.json
test -f crates/db/tests/golden_fixtures/integrity_nonzero_tool_index.jsonl
test -f crates/db/tests/golden_fixtures/integrity_progress_nested_content.jsonl
test -f crates/db/tests/golden_fixtures/integrity_tool_use_result_polymorphic.jsonl
test -f crates/db/tests/golden_fixtures/integrity_type_substring_noise.jsonl
test -f crates/db/tests/golden_fixtures/integrity_expected.json
test -s crates/db/tests/golden_fixtures/integrity_fixture.sha256

test -f crates/core/tests/live_parser_integrity_test.rs
test -f crates/db/tests/indexer_integrity_test.rs
test -f crates/db/tests/integrity_real_shape_fixtures.rs
test -f crates/search/tests/source_message_index_integrity_test.rs

GOVERNANCE_DOC="docs/plans/cross-cutting/2026-03-05-ground-truth-data-integrity-hardening.md"
if [ -f "${GOVERNANCE_DOC}" ]; then
  OWNERSHIP_BLOCK="$(awk "BEGIN{flag=0} /^## Ownership and Approval Matrix/{flag=1;next} /^## Go\\/No-Go Checklist/{flag=0} flag" "${GOVERNANCE_DOC}")"
  [ -n "${OWNERSHIP_BLOCK}" ] || { echo "ERROR: ownership section not found" >&2; exit 1; }
  if printf "%s\n" "${OWNERSHIP_BLOCK}" | rg -n "(<name>|TBD|TODO|REPLACE_ME|@@OWNER@@)"; then
    echo "ERROR: unresolved governance placeholders" >&2
    exit 1
  fi
else
  echo "SKIP: governance doc not present (private)"
fi
'

run_gate "Gate 1" "${ARTIFACT_DIR}/gate1.log" bash -euo pipefail -c '
cargo test --locked -p claude-view-core --lib live_parser 2>&1 | tee artifacts/integrity/gate1-core.log
cargo test --locked -p claude-view-db --lib indexer_parallel 2>&1 | tee artifacts/integrity/gate1-db-lib.log
cargo test --locked -p claude-view-db --test golden_parse_test 2>&1 | tee artifacts/integrity/gate1-db-golden.log
cargo test --locked -p claude-view-search --lib 2>&1 | tee artifacts/integrity/gate1-search.log
for f in \
  artifacts/integrity/gate1-core.log \
  artifacts/integrity/gate1-db-lib.log \
  artifacts/integrity/gate1-db-golden.log \
  artifacts/integrity/gate1-search.log; do
  test -s "${f}" || { echo "Missing/empty gate1 log: ${f}" >&2; exit 1; }
done
if rg -n "(running 0 tests|test result: ok\\. 0 passed)" \
  artifacts/integrity/gate1-core.log \
  artifacts/integrity/gate1-db-lib.log \
  artifacts/integrity/gate1-db-golden.log \
  artifacts/integrity/gate1-search.log; then
  echo "ERROR: zero-test false-green detected (gate1)" >&2
  exit 1
fi
'

run_gate "Gate 2" "${ARTIFACT_DIR}/gate2.log" bash -euo pipefail -c '
test -s crates/db/tests/golden_fixtures/integrity_fixture.sha256
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum -c crates/db/tests/golden_fixtures/integrity_fixture.sha256
else
  shasum -a 256 -c crates/db/tests/golden_fixtures/integrity_fixture.sha256
fi
for f in \
  crates/db/tests/golden_fixtures/integrity_nonzero_tool_index.jsonl \
  crates/db/tests/golden_fixtures/integrity_progress_nested_content.jsonl \
  crates/db/tests/golden_fixtures/integrity_tool_use_result_polymorphic.jsonl \
  crates/db/tests/golden_fixtures/integrity_type_substring_noise.jsonl \
  crates/db/tests/golden_fixtures/integrity_expected.json; do
  rg -F --quiet "${f}" crates/db/tests/golden_fixtures/integrity_fixture.sha256 || {
    echo "Missing checksum entry: ${f}" >&2
    exit 1
  }
done
cargo test --locked -p claude-view-core --test live_parser_integrity_test 2>&1 | tee artifacts/integrity/gate2-core-integrity.log
cargo test --locked -p claude-view-db --test indexer_integrity_test 2>&1 | tee artifacts/integrity/gate2-db-indexer-integrity.log
cargo test --locked -p claude-view-db --test integrity_real_shape_fixtures 2>&1 | tee artifacts/integrity/gate2-db-fixture-integrity.log
cargo test --locked -p claude-view-search --test source_message_index_integrity_test 2>&1 | tee artifacts/integrity/gate2-search-integrity.log
for f in \
  artifacts/integrity/gate2-core-integrity.log \
  artifacts/integrity/gate2-db-indexer-integrity.log \
  artifacts/integrity/gate2-db-fixture-integrity.log \
  artifacts/integrity/gate2-search-integrity.log; do
  test -s "${f}" || { echo "Missing/empty gate2 log: ${f}" >&2; exit 1; }
done
if rg -n "(running 0 tests|test result: ok\\. 0 passed)" \
  artifacts/integrity/gate2-core-integrity.log \
  artifacts/integrity/gate2-db-indexer-integrity.log \
  artifacts/integrity/gate2-db-fixture-integrity.log \
  artifacts/integrity/gate2-search-integrity.log; then
  echo "ERROR: zero-test false-green detected (gate2)" >&2
  exit 1
fi
'

run_gate "Gate 3" "${ARTIFACT_DIR}/gate3.log" bash -euo pipefail -c '
bash scripts/integrity/static-regression-checks.sh 2>&1 | tee artifacts/integrity/gate3-static.log
test -s artifacts/integrity/gate3-static.log
'

run_gate "Gate 4" "${ARTIFACT_DIR}/gate4.log" bash -euo pipefail -c '
LC_ALL=C TZ=UTC ./scripts/integrity/replay.sh \
  --snapshot crates/db/tests/integrity_corpus_snapshot \
  --before artifacts/integrity/replay-before.json \
  --after artifacts/integrity/replay-after.json \
  --diff artifacts/integrity/replay-diff.json
for f in \
  artifacts/integrity/replay-before.json \
  artifacts/integrity/replay-after.json \
  artifacts/integrity/replay-diff.json; do
  test -s "${f}" || { echo "Missing replay artifact: ${f}" >&2; exit 1; }
done
./scripts/integrity/assert-replay-thresholds.sh \
  artifacts/integrity/replay-before.json \
  artifacts/integrity/replay-after.json \
  crates/db/tests/golden_fixtures/integrity_expected.json \
  "$(git rev-parse HEAD)" 2>&1 | tee artifacts/integrity/gate4-thresholds.log
test -s artifacts/integrity/gate4-thresholds.log
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum \
    artifacts/integrity/replay-before.json \
    artifacts/integrity/replay-after.json \
    artifacts/integrity/replay-diff.json > artifacts/integrity/SHA256SUMS
else
  shasum -a 256 \
    artifacts/integrity/replay-before.json \
    artifacts/integrity/replay-after.json \
    artifacts/integrity/replay-diff.json > artifacts/integrity/SHA256SUMS
fi
test -s artifacts/integrity/SHA256SUMS
'

echo "ci-gates: OK"
