# Ground-Truth Data Integrity Hardening — Production Release Contract

**Status:** DRAFT (release-blocking)
**Date:** 2026-03-05
**Priority:** P0
**Branch:** `worktree-monorepo-expo`
**Scope:** `crates/core`, `crates/db`, `crates/search`, `crates/server`, `packages/shared`, `docs`, `scripts`, CI

## Purpose

This contract defines hard release conditions so `claude-view` never presents fabricated, guessed, or schema-invented chat/session data as source truth.

Anything marked **MUST** is a ship blocker.

## Normative Language

- **MUST** = release blocking
- **SHOULD** = strongly required unless explicit written exception approved by owners
- **MAY** = optional

## Hard Invariants (Must Hold)

1. Parser/indexer MUST not depend on imaginary fields.
2. Top-level message type classification MUST come from exact parsed `type` value, never substring inference.
3. Missing source data MUST remain missing (`NULL`/`None`) unless field is declared `derived_with_fallback`.
4. Every surfaced field MUST have a declared provenance class.
5. Unknown/invalid inputs MUST be counted and observable.
6. Synthetic docs MUST never masquerade as source conversation messages.

## Locked Product Decisions (No Open Branches)

1. Source-message search roles for this release are only `user`, `assistant`, `tool`.
2. Synthetic `role:"summary"` docs are removed from default source-message indexing.
3. `live_parser` line type classification uses parsed top-level `type` only.
4. LOC and file extraction use `assistant.message.content[*].input` only.
5. Sub-agent progress extraction uses `type:"progress" && data.type:"agent_progress"` with primary nested path `data.message.message.content[*]`.
6. Forced full rebuild command for this release is `POST /api/sync/deep`; `POST /api/system/reindex` remains a valid endpoint but MUST NOT satisfy this release deep-rebuild control requirement.

## Real-Log Ground Truth

Operator verification scan source for this audit:
1. `/Users/TBGor/.claude/projects/**/*.jsonl` (5303 files scanned on 2026-03-05 UTC).
2. Repro command transcript MUST be saved to `artifacts/integrity/real-corpus-verification.txt`.

Reproducible verification commands:
1. `find /Users/TBGor/.claude/projects -type f -name '*.jsonl' -print0 | xargs -0 jq -c 'select(.type=="result")' | wc -l`
2. `find /Users/TBGor/.claude/projects -type f -name '*.jsonl' -print0 | xargs -0 jq -c 'select(.type=="assistant" and has("input"))' | wc -l`
3. `find /Users/TBGor/.claude/projects -type f -name '*.jsonl' -print0 | xargs -0 jq -c 'select(.type=="assistant" and (.message.content|type=="array") and (any(.message.content[]?; has("input"))))' | wc -l`
4. `find /Users/TBGor/.claude/projects -type f -name '*.jsonl' -print0 | xargs -0 jq -c 'select(.type=="progress" and .data.type=="agent_progress")' | wc -l`
5. `find /Users/TBGor/.claude/projects -type f -name '*.jsonl' -print0 | xargs -0 jq -c 'select(.type=="progress" and .data.type=="agent_progress" and (.data.message.content|type=="array"))' | wc -l`
6. `find /Users/TBGor/.claude/projects -type f -name '*.jsonl' -print0 | xargs -0 jq -c 'select(.type=="progress" and .data.type=="agent_progress" and (.data.message.message.content|type=="array"))' | wc -l`
7. `find /Users/TBGor/.claude/projects -type f -name '*.jsonl' -print0 | xargs -0 jq -r 'select(has("toolUseResult")) | (.toolUseResult|type)' | sort | uniq -c`
8. `find /Users/TBGor/.claude/projects -type f -name '*.jsonl' -print0 | xargs -0 jq -r 'select(.type=="assistant" and (.message.content|type=="array")) | .message.content[]? | select(.type=="tool_use" and (.input|type=="object")) | (.input|keys[])' | rg '^(file_path|path|filename|filePath|entrypoint_path)$' | sort | uniq -c | sort -nr`

Observed outputs on 2026-03-05 UTC (informational telemetry, not CI thresholds):
1. `result_type_count = 0`
2. `assistant_top_level_input_count = 0`
3. `assistant_nested_content_input_count = 130059`
4. `progress_agent_progress_count = 89130`
5. `progress_data_message_content_count = 0`
6. `progress_data_message_message_content_count = 89130`
7. `toolUseResult_type_counts = {object: 53120, string: 5052, array: 738}`
8. `tool_use_input_key_counts = {file_path: 61401, path: 17491, filename: 67, filePath: 22, entrypoint_path: 3}`

### Canonical Replay Corpus (Deterministic)

Release gating MUST read replay input only from `crates/db/tests/integrity_corpus_snapshot/`; reads from `$HOME` or absolute paths outside the repo are release-blocking.

Required assets:
1. `crates/db/tests/integrity_corpus_snapshot/`
2. `crates/db/tests/integrity_corpus_snapshot/manifest.json`
3. `scripts/integrity/replay.sh`
4. `scripts/integrity/assert-replay-thresholds.sh`

`manifest.json` MUST include:
1. `files`
2. `lines`
3. `snapshot_sha256`
4. `captured_at_utc`
5. `schema_version`
6. `baseline_git_commit`

Traversal order MUST be deterministic (`LC_ALL=C`, sorted paths).

## Release Blockers and Required Fixes

## Blocker A — Imaginary LOC extraction path

Required fix:
1. Remove top-level LOC read path.
2. Compute LOC from `tool_use` blocks only.
3. Aggregate across all tool-use blocks in a line.
4. No implicit fabricated source values.

Target file:
- `crates/db/src/indexer_parallel.rs`

## Blocker B — Incomplete file-path key handling

Required extraction key order:
1. `file_path`
2. `path`
3. `filename`
4. `filePath`
5. `entrypoint_path`

If none found, do not synthesize path.
Any newly supported key outside this list MUST be proven in real corpus evidence and added to replay fixtures first.

Target file:
- `crates/db/src/indexer_parallel.rs`

## Blocker C — Substring type misclassification

Required fix:
1. Parse JSON first.
2. Classify by exact `type` string.
3. Unknown values become `LineType::Other` plus counter increment.

Target file:
- `crates/core/src/live_parser.rs`

## Blocker D — Wrong nested progress path

Required fix:
1. Primary path: `data.message.message.content[*]`.
2. Compatibility fallback: `data.message.content[*]`.
3. Fallback usage MUST increment `legacy_fallback_path_count`.

Target file:
- `crates/core/src/live_parser.rs`

## Blocker E — `toolUseResult` variant assumptions

Required fix:
1. Handle `object`, `string`, and `array` without dropping whole line.
2. Variant mismatch MUST not coerce type; count it in diagnostics.

Target files:
- `crates/core/src/live_parser.rs`
- `crates/core/src/parser.rs`

## Blocker F — Synthetic source-role indexing

Required fix:
1. Remove synthetic summary role docs from source-message index output.
2. Source-message index output MUST contain only records with source provenance and `role in {"user","assistant","tool"}`.
3. Any derived document attempting `doc_kind:"source_message"` MUST increment `derived_source_message_doc_count` and block release via Gate 4.

Target files:
- `crates/db/src/indexer_parallel.rs`
- `crates/search/src/indexer.rs`
- `crates/search/src/types.rs`

## Provenance Contract (Normative)

Every externally exposed field MUST be classified exactly as one:
1. `source`
2. `derived`
3. `derived_with_fallback`

`derived_with_fallback` MUST document:
1. fallback input sources
2. default/null behavior
3. downgrade semantics and counters

### Release-Critical Field Taxonomy

| Surface | Field | Class | Canonical source/formula | Null/default policy |
|---|---|---|---|---|
| DB/API | `message_count` | `derived` | `user_prompt_count + api_call_count` | non-null integer, default `0` |
| DB/API | `files_touched` | `derived_with_fallback` | tool-use input keys in locked order | `[]` when no Edit/Write exists; `NULL` when Edit/Write exists but extraction fails |
| DB/API | `lines_added` | `derived_with_fallback` | Edit/Write tool-use input content | `0` when no Edit/Write exists; `NULL` when Edit/Write exists but extraction fails |
| DB/API | `lines_removed` | `derived_with_fallback` | Edit tool-use input old content | `0` when no Edit/Write exists; `NULL` when Edit/Write exists but extraction fails |
| DB/API | `loc_source` | `derived` | enum source tag | non-null integer enum |
| DB internal | `summary_text` | `source` | top-level `type:"summary"` line | nullable |
| API | `summary` | `derived_with_fallback` | `COALESCE(summary_text, summary)` | nullable |
| Search | `role` | `source` | indexed message role for source-message docs | allowed values: `user`, `assistant`, `tool` |
| Search | `doc_kind` | `derived` | indexed document discriminator | non-null string; `source_message` for this release |

Any field in `crates/db/tests/golden_fixtures/integrity_expected.json.required_source_fields` that is missing from this taxonomy table, or appears more than once, blocks release.
Any externally exposed field added or behavior-changed by this release that is not taxonomy-mapped blocks release.

## Schema and Parse Contract

1. Audit-time baseline constant in repo is `CURRENT_PARSE_VERSION = 20`.
2. This release target MUST be `TARGET_PARSE_VERSION = 21` (baseline `CURRENT_PARSE_VERSION + 1`).
3. Reindex eligibility MUST be `parse_version IS NULL OR parse_version < TARGET_PARSE_VERSION`.
4. Successful reindex writes MUST set `parse_version = TARGET_PARSE_VERSION`.
5. This release does not require dropping existing `sessions` columns.
6. If repo `CURRENT_PARSE_VERSION` is no longer `20`, this contract MUST be updated before execution; stale parse-version contracts are release-blocking.

### Integrity Counters (Persistent + API-visible)

Add release counters to persisted `index_runs` metadata.
Alternative persistence backends are out of scope for this release contract.
1. `unknown_top_level_type_count`
2. `unknown_required_path_count`
3. `imaginary_path_access_count`
4. `legacy_fallback_path_count`
5. `dropped_line_invalid_json_count`
6. `schema_mismatch_count`
7. `unknown_source_role_count`
8. `derived_source_message_doc_count`
9. `source_message_non_source_provenance_count`

`GET /api/system` MUST expose these counters at JSON path `.integrity.counters.<counter_name>` with integer values.

## Unknown/Invalid Input Policy (Normative)

| Condition | Action | Counter | Drop line |
|---|---|---|---|
| Unknown top-level `type` string | classify as `Other` | `unknown_top_level_type_count` | No |
| Missing/non-string top-level `type` | classify as `Other` | `unknown_top_level_type_count` | No |
| Missing required nested path on known type | keep row and set only missing required fields to `NULL`; preserve successfully parsed fields | `unknown_required_path_count` | No |
| Compatibility path consumed | parse with fallback | `legacy_fallback_path_count` | No |
| Forbidden imaginary path touched | abort parse+index for current session file; write no rows for that file | `imaginary_path_access_count` | No |
| Invalid JSON line | skip line | `dropped_line_invalid_json_count` | Yes |
| Variant mismatch | keep line with best valid subset | `schema_mismatch_count` | No |
| Unknown/non-allowed source role for source-message indexing | do not index as source-message doc | `unknown_source_role_count` | No |
| Derived record attempted as `doc_kind:"source_message"` | reject index row and count violation | `derived_source_message_doc_count` | No |
| Indexed `source_message` row has non-source provenance | reject row and count violation | `source_message_non_source_provenance_count` | No |

## Observability and Artifacts

Each full scan MUST emit:
1. `artifacts/integrity/replay-before.json`
2. `artifacts/integrity/replay-after.json`
3. `artifacts/integrity/replay-diff.json`
4. `artifacts/integrity/SHA256SUMS`

Required report keys:
1. `report_version`
2. `git_commit`
3. `generated_at_utc`
4. `snapshot_sha256`
5. `baseline_git_commit`
6. `target_parse_version`
7. `counters`
8. `rates`

`counters` MUST include:
1. `unknown_top_level_type_count`
2. `unknown_required_path_count`
3. `imaginary_path_access_count`
4. `legacy_fallback_path_count`
5. `dropped_line_invalid_json_count`
6. `schema_mismatch_count`
7. `unknown_source_role_count`
8. `derived_source_message_doc_count`
9. `synthetic_summary_source_doc_count`
10. `source_message_non_source_provenance_count`
11. `files_touched_extraction_count`

`rates` MUST include:
1. `loc_nonzero_rate_edit_write` = `sessions_with_nonzero_loc_from_edit_or_write / sessions_with_edit_or_write`
2. `source_field_completeness_rate` = `non_null_required_source_field_cells / total_required_source_field_cells`

Deterministic denominator definitions:
1. `sessions_with_edit_or_write`: sessions containing at least one `tool_use` block with `name in {"Edit","Write"}`.
2. `sessions_with_nonzero_loc_from_edit_or_write`: subset of above where `(lines_added + lines_removed) > 0`.
3. `total_required_source_field_cells`: `(count of replay-after records) * (length of integrity_expected.json.required_source_fields)`.
4. `non_null_required_source_field_cells`: count of required-source-field cells in replay-after records where value is not `null`.

## Test and CI Gates (All Release-Blocking)

All gate commands MUST execute through `scripts/integrity/ci-gates.sh` (not ad-hoc copy/paste).

`scripts/integrity/ci-gates.sh` MUST:
1. start with `#!/usr/bin/env bash`
2. enable `set -euo pipefail`
3. fail fast on missing files/commands/artifacts
4. write logs under `artifacts/integrity/`
5. verify required tools before running gates: `cargo`, `rg`, `bash`, `jq`, and at least one of `sha256sum`/`shasum`

## Gate 0 — Required asset presence gate

Commands:
1. `command -v cargo >/dev/null 2>&1`
2. `command -v rg >/dev/null 2>&1`
3. `command -v bash >/dev/null 2>&1`
4. `command -v jq >/dev/null 2>&1`
5. `command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1`
6. `test -f scripts/integrity/static-regression-checks.sh && test -x scripts/integrity/static-regression-checks.sh`
7. `test -f scripts/integrity/replay.sh && test -x scripts/integrity/replay.sh`
8. `test -f scripts/integrity/assert-replay-thresholds.sh && test -x scripts/integrity/assert-replay-thresholds.sh`
9. `test -f scripts/integrity/ci-gates.sh && test -x scripts/integrity/ci-gates.sh`
10. `test -d crates/db/tests/integrity_corpus_snapshot`
11. `test -s crates/db/tests/integrity_corpus_snapshot/manifest.json`
12. `test -f crates/db/tests/golden_fixtures/integrity_nonzero_tool_index.jsonl`
13. `test -f crates/db/tests/golden_fixtures/integrity_progress_nested_content.jsonl`
14. `test -f crates/db/tests/golden_fixtures/integrity_tool_use_result_polymorphic.jsonl`
15. `test -f crates/db/tests/golden_fixtures/integrity_type_substring_noise.jsonl`
16. `test -f crates/db/tests/golden_fixtures/integrity_expected.json`
17. `test -s crates/db/tests/golden_fixtures/integrity_fixture.sha256`
18. `test -f crates/core/tests/live_parser_integrity_test.rs`
19. `test -f crates/db/tests/indexer_integrity_test.rs`
20. `test -f crates/db/tests/integrity_real_shape_fixtures.rs`
21. `test -f crates/search/tests/source_message_index_integrity_test.rs`
22. `OWNERSHIP_BLOCK="$(awk '/^## Ownership and Approval Matrix/{flag=1;next}/^## Go\\/No-Go Checklist/{flag=0}flag' docs/plans/cross-cutting/2026-03-05-ground-truth-data-integrity-hardening.md)"; [ -n "${OWNERSHIP_BLOCK}" ] || { echo "ERROR: ownership section not found" >&2; exit 1; }`
23. `if printf '%s\n' "${OWNERSHIP_BLOCK}" | rg -n '(<name>|TBD|TODO|REPLACE_ME|@@OWNER@@)'; then echo "ERROR: unresolved governance placeholders" >&2; exit 1; fi`

## Gate 1 — Unit and integration command set

1. `mkdir -p artifacts/integrity`
2. `cargo test --locked -p claude-view-core --lib live_parser 2>&1 | tee artifacts/integrity/gate1-core.log`
3. `cargo test --locked -p claude-view-db --lib indexer_parallel 2>&1 | tee artifacts/integrity/gate1-db-lib.log`
4. `cargo test --locked -p claude-view-db --test golden_parse_test 2>&1 | tee artifacts/integrity/gate1-db-golden.log`
5. `cargo test --locked -p claude-view-search --lib 2>&1 | tee artifacts/integrity/gate1-search.log`
6. `for f in artifacts/integrity/gate1-core.log artifacts/integrity/gate1-db-lib.log artifacts/integrity/gate1-db-golden.log artifacts/integrity/gate1-search.log; do test -s "${f}" || { echo "Missing/empty gate1 log: ${f}" >&2; exit 1; }; done`
7. `if rg -n "(running 0 tests|test result: ok\\. 0 passed)" artifacts/integrity/gate1-core.log artifacts/integrity/gate1-db-lib.log artifacts/integrity/gate1-db-golden.log artifacts/integrity/gate1-search.log; then echo "ERROR: zero-test false-green detected (gate1)" >&2; exit 1; fi`

## Gate 2 — Integrity test suite and fixture gate

Commands:
1. `test -s crates/db/tests/golden_fixtures/integrity_fixture.sha256`
2. `if command -v sha256sum >/dev/null 2>&1; then sha256sum -c crates/db/tests/golden_fixtures/integrity_fixture.sha256; else shasum -a 256 -c crates/db/tests/golden_fixtures/integrity_fixture.sha256; fi`
3. `for f in crates/db/tests/golden_fixtures/integrity_nonzero_tool_index.jsonl crates/db/tests/golden_fixtures/integrity_progress_nested_content.jsonl crates/db/tests/golden_fixtures/integrity_tool_use_result_polymorphic.jsonl crates/db/tests/golden_fixtures/integrity_type_substring_noise.jsonl crates/db/tests/golden_fixtures/integrity_expected.json; do rg -F --quiet "${f}" crates/db/tests/golden_fixtures/integrity_fixture.sha256 || { echo "Missing checksum entry: ${f}" >&2; exit 1; }; done`
4. `cargo test --locked -p claude-view-core --test live_parser_integrity_test 2>&1 | tee artifacts/integrity/gate2-core-integrity.log`
5. `cargo test --locked -p claude-view-db --test indexer_integrity_test 2>&1 | tee artifacts/integrity/gate2-db-indexer-integrity.log`
6. `cargo test --locked -p claude-view-db --test integrity_real_shape_fixtures 2>&1 | tee artifacts/integrity/gate2-db-fixture-integrity.log`
7. `cargo test --locked -p claude-view-search --test source_message_index_integrity_test 2>&1 | tee artifacts/integrity/gate2-search-integrity.log`
8. `for f in artifacts/integrity/gate2-core-integrity.log artifacts/integrity/gate2-db-indexer-integrity.log artifacts/integrity/gate2-db-fixture-integrity.log artifacts/integrity/gate2-search-integrity.log; do test -s "${f}" || { echo "Missing/empty gate2 log: ${f}" >&2; exit 1; }; done`
9. `if rg -n "(running 0 tests|test result: ok\\. 0 passed)" artifacts/integrity/gate2-core-integrity.log artifacts/integrity/gate2-db-indexer-integrity.log artifacts/integrity/gate2-db-fixture-integrity.log artifacts/integrity/gate2-search-integrity.log; then echo "ERROR: zero-test false-green detected (gate2)" >&2; exit 1; fi`

## Gate 3 — Static anti-regression gate

Commands:
1. `bash scripts/integrity/static-regression-checks.sh 2>&1 | tee artifacts/integrity/gate3-static.log`
2. `test -s artifacts/integrity/gate3-static.log`

Checks MUST include:
1. forbidden top-level LOC read pattern (`value.get("input")` on full assistant line)
2. forbidden substring-based type classification patterns in `live_parser`
3. forbidden synthetic source role `summary` indexing pattern

## Gate 4 — Deterministic replay gate

`replay.sh` MUST generate:
1. `replay-before.json` from baseline commit in `integrity_corpus_snapshot/manifest.json.baseline_git_commit`.
2. `replay-after.json` from current candidate commit.
3. `replay-diff.json` from deterministic comparison of before/after.

Commands:
1. `LC_ALL=C TZ=UTC ./scripts/integrity/replay.sh --snapshot crates/db/tests/integrity_corpus_snapshot --before artifacts/integrity/replay-before.json --after artifacts/integrity/replay-after.json --diff artifacts/integrity/replay-diff.json`
2. `for f in artifacts/integrity/replay-before.json artifacts/integrity/replay-after.json artifacts/integrity/replay-diff.json; do test -s "${f}" || { echo "Missing replay artifact: ${f}" >&2; exit 1; }; done`
3. `./scripts/integrity/assert-replay-thresholds.sh artifacts/integrity/replay-before.json artifacts/integrity/replay-after.json crates/db/tests/golden_fixtures/integrity_expected.json "$(git rev-parse HEAD)" 2>&1 | tee artifacts/integrity/gate4-thresholds.log`
4. `test -s artifacts/integrity/gate4-thresholds.log`
5. `if command -v sha256sum >/dev/null 2>&1; then sha256sum artifacts/integrity/replay-before.json artifacts/integrity/replay-after.json artifacts/integrity/replay-diff.json > artifacts/integrity/SHA256SUMS; else shasum -a 256 artifacts/integrity/replay-before.json artifacts/integrity/replay-after.json artifacts/integrity/replay-diff.json > artifacts/integrity/SHA256SUMS; fi`
6. `test -s artifacts/integrity/SHA256SUMS`

Ship thresholds:
1. `.after.counters.imaginary_path_access_count == 0`
2. `.after.counters.synthetic_summary_source_doc_count == 0`
3. `.after.counters.derived_source_message_doc_count == 0`
4. `.after.counters.source_message_non_source_provenance_count == 0`
5. `.after.counters.unknown_source_role_count == .expected.counters.unknown_source_role_count`
6. `.after.counters.unknown_top_level_type_count == .expected.counters.unknown_top_level_type_count`
7. `.after.counters.unknown_required_path_count == .expected.counters.unknown_required_path_count`
8. `.after.counters.legacy_fallback_path_count == .expected.counters.legacy_fallback_path_count`
9. `.after.rates.loc_nonzero_rate_edit_write >= .expected.rates.min_loc_nonzero_rate_edit_write`
10. `.after.counters.files_touched_extraction_count >= .expected.counters.min_files_touched_extraction_count`
11. `.after.rates.source_field_completeness_rate >= .expected.rates.min_source_field_completeness_rate`
12. `.before.baseline_git_commit == .after.baseline_git_commit`
13. `.before.git_commit == .before.baseline_git_commit`
14. `.after.git_commit == $CANDIDATE_SHA`

Gate 4 evaluates replay metrics only. Test-failure enforcement is owned by Gate 1 and Gate 2 command exits.

`assert-replay-thresholds.sh` MUST fail if any required key is missing or non-numeric before evaluating thresholds.
`assert-replay-thresholds.sh` MUST also fail if `.expected` or any expected-threshold key is missing.
`assert-replay-thresholds.sh` MUST fail if manifest `baseline_git_commit` does not match `.before.baseline_git_commit`.
`assert-replay-thresholds.sh` invocation contract is `(before_json, after_json, expected_json, candidate_sha)` and MUST fail on missing args.

## Gate 5 — CI enforcement gate

Required workflow file: `.github/workflows/ci.yml`

Required CI job contract:
1. Add a dedicated blocking job named `integrity-gates`.
2. Job command MUST be `bash scripts/integrity/ci-gates.sh`.
3. Upload artifacts with `actions/upload-artifact` using `path: artifacts/integrity/**` and `if-no-files-found: error`.
4. Job MUST not set `continue-on-error: true`.
5. Branch protection required status check MUST include `CI / integrity-gates`.
6. Gate-oracle files (`integrity_expected.json`, `integrity_fixture.sha256`, `replay.sh`, `assert-replay-thresholds.sh`) MUST be CODEOWNERS-protected with required code-owner reviews.

Out-of-band governance verification (release-blocking evidence; not part of `ci-gates.sh` execution) MUST run as one shell block with `set -euo pipefail` and log to `artifacts/integrity/gate5-governance.log`.
1. `export GH_REPO="${GH_REPO:-$(gh repo view --json nameWithOwner -q .nameWithOwner)}"`
2. `mkdir -p artifacts/integrity`
3. `set -euo pipefail`
4. `exec > >(tee artifacts/integrity/gate5-governance.log) 2>&1`
5. `test -f .github/CODEOWNERS`
6. `rg -n '^crates/db/tests/golden_fixtures/integrity_expected\\.json\\b\\s+[^#[:space:]]+' .github/CODEOWNERS`
7. `rg -n '^crates/db/tests/golden_fixtures/integrity_fixture\\.sha256\\b\\s+[^#[:space:]]+' .github/CODEOWNERS`
8. `rg -n '^scripts/integrity/replay\\.sh\\b\\s+[^#[:space:]]+' .github/CODEOWNERS`
9. `rg -n '^scripts/integrity/assert-replay-thresholds\\.sh\\b\\s+[^#[:space:]]+' .github/CODEOWNERS`
10. `gh api "repos/${GH_REPO}/branches/main/protection" > artifacts/integrity/branch-protection.main.json`
11. `jq -e '.required_status_checks.strict == true' artifacts/integrity/branch-protection.main.json >/dev/null`
12. `jq -e '(.required_status_checks.checks // []) as $checks | if ($checks | length) > 0 then ($checks | any(.context=="CI / integrity-gates" and (.app_id|type=="number"))) else ((.required_status_checks.contexts // []) | index("CI / integrity-gates") != null) end' artifacts/integrity/branch-protection.main.json >/dev/null`
13. `jq -e '(.required_pull_request_reviews.required_approving_review_count // 0) >= 1' artifacts/integrity/branch-protection.main.json >/dev/null`
14. `jq -e '(.required_pull_request_reviews.require_code_owner_reviews // false) == true' artifacts/integrity/branch-protection.main.json >/dev/null`
15. `jq -e '(.enforce_admins.enabled // false) == true' artifacts/integrity/branch-protection.main.json >/dev/null`
16. `if jq -e '(.allow_force_pushes.enabled // false) or (.allow_deletions.enabled // false)' artifacts/integrity/branch-protection.main.json >/dev/null; then echo "ERROR: force-push or branch deletion is enabled on main" >&2; exit 1; fi`
17. `test -s artifacts/integrity/branch-protection.main.json`
18. `test -s artifacts/integrity/gate5-governance.log`

## Gate 6 — Release Governance and Approval Gate (Out-of-Band)

Commands:
1. `test -s artifacts/integrity/approvals.json`
2. `CANDIDATE_SHA="$(git rev-parse HEAD)"`
3. `jq -e --arg sha "${CANDIDATE_SHA}" '([.approvals[] | select(.commit_sha==$sha) | .role] | sort | unique) as $r | (["rollout_commander","parser_indexer_lead","rollback_approver"] - $r | length)==0 and ((($r|index("search_lead"))!=null) or (($r|index("observability_lead"))!=null))' artifacts/integrity/approvals.json >/dev/null`
4. `jq -e --arg sha "${CANDIDATE_SHA}" '([.approvals[] | select(.commit_sha==$sha and .role=="rollout_commander") | .approver] | unique | length)==1 and ([.approvals[] | select(.commit_sha==$sha and .role=="rollback_approver") | .approver] | unique | length)==1 and ([.approvals[] | select(.commit_sha==$sha and .role=="rollout_commander") | .approver][0] != [.approvals[] | select(.commit_sha==$sha and .role=="rollback_approver") | .approver][0])' artifacts/integrity/approvals.json >/dev/null`
5. `jq -e '([.vetoes[]? | select((.rescinded_at_utc // null) == null)] | length) == 0' artifacts/integrity/approvals.json >/dev/null`
6. `test -s artifacts/integrity/signoff.json`
7. `jq -e '(.commit_sha|type=="string") and (.ci_run_url|type=="string") and (.gate_sha256s|type=="array") and (.branch_protection_sha256|type=="string") and (.approvals_sha256|type=="string") and (.approvers|type=="array") and (.approved_at_utc|type=="string")' artifacts/integrity/signoff.json >/dev/null`

## Deployment Runbook (Executable)

### Operator Variables

```bash
set -euo pipefail
export CLAUDE_VIEW_PORT="${CLAUDE_VIEW_PORT:-47892}"
export BASE_URL="http://127.0.0.1:${CLAUDE_VIEW_PORT}"
export DATA_DIR="${CLAUDE_VIEW_DATA_DIR:-$HOME/Library/Caches/claude-view}"
export DB_PATH="${DATA_DIR}/claude-view.db"
export TARGET_PARSE_VERSION="${TARGET_PARSE_VERSION:-21}"
export MAX_STARTUP_SETTLE_SECONDS="${MAX_STARTUP_SETTLE_SECONDS:-600}"
export MAX_DEEP_REBUILD_SECONDS="${MAX_DEEP_REBUILD_SECONDS:-1800}"
export TS="$(date -u +%Y%m%dT%H%M%SZ)"
export RELEASE_DIR="${DATA_DIR}/release-backups/${TS}"
export DEPLOY_CMD="${DEPLOY_CMD:?set DEPLOY_CMD to deploy+restart command}"
export STOP_CURRENT_CMD="${STOP_CURRENT_CMD:?set STOP_CURRENT_CMD to stop current build}"
export START_PREVIOUS_CMD="${START_PREVIOUS_CMD:?set START_PREVIOUS_CMD to start previous build}"
mkdir -p "${RELEASE_DIR}"
for cmd in curl jq sqlite3 tar; do command -v "${cmd}" >/dev/null 2>&1 || { echo "Missing required command: ${cmd}" >&2; exit 1; }; done
```

### Rollout Steps

1. Preflight baseline:
```bash
[ -f "${DB_PATH}" ] || { echo "Missing database at ${DB_PATH}" >&2; exit 1; }
curl --fail --silent --show-error --max-time 10 "${BASE_URL}/api/health"
curl --fail --silent --show-error --max-time 10 "${BASE_URL}/api/system" > "${RELEASE_DIR}/system.pre.json"
curl --fail --silent --show-error --max-time 10 "${BASE_URL}/api/indexing/status" > "${RELEASE_DIR}/indexing.pre.json"
sqlite3 "${DB_PATH}" "SELECT COUNT(*) AS sessions_before FROM sessions;"
```

2. Backup DB and search index:
```bash
if [ -f "${DB_PATH}" ]; then sqlite3 "${DB_PATH}" ".backup '${RELEASE_DIR}/claude-view.db.pre'"; fi
if [ -d "${DATA_DIR}/search-index" ]; then tar -C "${DATA_DIR}" -czf "${RELEASE_DIR}/search-index.pre.tgz" search-index; fi
```

3. Deploy/restart new build:
```bash
eval "${DEPLOY_CMD}"
```

4. Wait for startup indexing state to settle:
```bash
STARTUP_DEADLINE_EPOCH="$(( $(date +%s) + MAX_STARTUP_SETTLE_SECONDS ))"
while true; do
  [ "$(date +%s)" -le "${STARTUP_DEADLINE_EPOCH}" ] || { echo "Startup indexing did not settle before timeout" >&2; exit 1; }
  PHASE="$(curl --fail --silent --show-error --max-time 10 "${BASE_URL}/api/indexing/status" | jq -er '.phase')"
  case "${PHASE}" in
    reading-indexes|deep-indexing)
      sleep 2
      ;;
    done|idle)
      break
      ;;
    error)
      echo "Startup indexing phase is error" >&2
      exit 1
      ;;
    *)
      echo "Unexpected indexing phase: ${PHASE}" >&2
      exit 1
      ;;
  esac
done
```

5. Trigger mandatory deep rebuild:
```bash
RESP="$(curl --fail --silent --show-error --max-time 10 -X POST "${BASE_URL}/api/sync/deep")"
echo "${RESP}" | jq .
[ "$(echo "${RESP}" | jq -er '.status')" = "accepted" ] || { echo "Unexpected /api/sync/deep response" >&2; exit 1; }
DEEP_REBUILD_DEADLINE_EPOCH="$(( $(date +%s) + MAX_DEEP_REBUILD_SECONDS ))"
while true; do
  [ "$(date +%s)" -le "${DEEP_REBUILD_DEADLINE_EPOCH}" ] || { echo "Deep rebuild did not finish before timeout" >&2; exit 1; }
  PHASE="$(curl --fail --silent --show-error --max-time 10 "${BASE_URL}/api/indexing/status" | jq -er '.phase')"
  case "${PHASE}" in
    done)
      break
      ;;
    error)
      echo "Deep rebuild entered error phase" >&2
      exit 1
      ;;
    reading-indexes|deep-indexing|idle)
      sleep 2
      ;;
    *)
      echo "Unexpected indexing phase during deep rebuild: ${PHASE}" >&2
      exit 1
      ;;
  esac
done
```

6. Post-rollout verification:
```bash
PENDING_DEEP_INDEX="$(sqlite3 "${DB_PATH}" "SELECT COUNT(*) FROM sessions WHERE file_path IS NOT NULL AND file_path != '' AND deep_indexed_at IS NULL;")"
[ "${PENDING_DEEP_INDEX}" -eq 0 ] || { echo "Rollback trigger: pending_deep_index=${PENDING_DEEP_INDEX}" >&2; exit 1; }
NON_TARGET_PARSE_VERSION="$(sqlite3 "${DB_PATH}" "SELECT COUNT(*) FROM sessions WHERE parse_version IS NULL OR parse_version != ${TARGET_PARSE_VERSION};")"
[ "${NON_TARGET_PARSE_VERSION}" -eq 0 ] || { echo "Rollback trigger: parse_version!=${TARGET_PARSE_VERSION} rows=${NON_TARGET_PARSE_VERSION}" >&2; exit 1; }
PENDING_GIT_ROOT_BACKFILL="$(sqlite3 "${DB_PATH}" "SELECT COUNT(*) FROM sessions WHERE session_cwd IS NOT NULL AND git_root IS NULL;")"
echo "pending_git_root_backfill=${PENDING_GIT_ROOT_BACKFILL}"
curl --fail --silent --show-error --max-time 10 "${BASE_URL}/api/health" > "${RELEASE_DIR}/health.post.txt"
curl --fail --silent --show-error --max-time 10 "${BASE_URL}/api/system" > "${RELEASE_DIR}/system.post.json"
curl --fail --silent --show-error --max-time 10 "${BASE_URL}/api/indexing/status" > "${RELEASE_DIR}/indexing.post.json"
jq -e '.integrity.counters.imaginary_path_access_count == 0 and .integrity.counters.synthetic_summary_source_doc_count == 0 and .integrity.counters.derived_source_message_doc_count == 0 and .integrity.counters.source_message_non_source_provenance_count == 0' "${RELEASE_DIR}/system.post.json" >/dev/null
```

### Rollback Steps

Rollback MUST start immediately if any trigger fires:
1. Any Gate 0-6 command exits non-zero.
2. `/api/health` fails in preflight or post-rollout verification.
3. `/api/indexing/status.phase == "error"` at any point.
4. Startup settle timeout (`MAX_STARTUP_SETTLE_SECONDS`) is exceeded.
5. Deep rebuild timeout (`MAX_DEEP_REBUILD_SECONDS`) is exceeded.
6. `pending_deep_index > 0` after post-rollout verification.
7. Any session has `parse_version IS NULL OR parse_version != TARGET_PARSE_VERSION` after post-rollout verification.
8. Replay-threshold assertion command exits non-zero.
9. Any out-of-band branch-protection verification command exits non-zero.
10. Approval quorum/schema validation fails for `artifacts/integrity/approvals.json`.

Rollback authority:
1. Rollback approver MAY trigger rollback unilaterally.
2. Rollout commander MAY trigger rollback unilaterally.
3. No additional approval is required once a rollback trigger is met.

1. Stop current build:
```bash
eval "${STOP_CURRENT_CMD}"
```

2. Restore snapshot:
```bash
[ -d "${DATA_DIR}" ] || mkdir -p "${DATA_DIR}"
[ -f "${RELEASE_DIR}/claude-view.db.pre" ] && cp "${RELEASE_DIR}/claude-view.db.pre" "${DB_PATH}"
rm -f "${DB_PATH}-wal" "${DB_PATH}-shm"
if [ -f "${RELEASE_DIR}/search-index.pre.tgz" ]; then rm -rf "${DATA_DIR}/search-index"; tar -xzf "${RELEASE_DIR}/search-index.pre.tgz" -C "${DATA_DIR}"; fi
```

3. Start previous build:
```bash
eval "${START_PREVIOUS_CMD}"
```

4. Verify:
```bash
curl --fail --silent --show-error --max-time 10 "${BASE_URL}/api/health"
curl --fail --silent --show-error --max-time 10 "${BASE_URL}/api/system"
```

## Ownership and Approval Matrix (Must Fill Before Release)

| Area | Primary (GitHub handle) | Secondary (GitHub handle) | Authority |
|---|---|---|---|
| Rollout commander | `@TBGor` | `@vicky-ai-release` | Final go/no-go caller |
| Parser/indexer lead | `@TBGor` | `@vicky-ai-data` | Approves Blockers A-E closure |
| Search lead | `@vicky-ai-search` | `@TBGor` | Approves Blocker F closure |
| Observability lead | `@vicky-ai-obs` | `@TBGor` | Approves counters and artifact completeness |
| Rollback approver | `@vicky-ai-ops` | `@vicky-ai-release` | Independent veto and unilateral rollback authority |

Go/No-Go quorum (release-blocking):
1. Rollout commander approval.
2. Parser/indexer lead approval.
3. Rollback approver approval.
4. At least one of Search lead or Observability lead approval.

Approval evidence requirements:
1. Save approvals in `artifacts/integrity/approvals.json` with schema: `{"approvals":[{"role":"rollout_commander|parser_indexer_lead|search_lead|observability_lead|rollback_approver","approver":"@handle","approved_at_utc":"YYYY-MM-DDTHH:MM:SSZ","commit_sha":"<candidate_sha>"}],"vetoes":[{"approver":"@handle","recorded_at_utc":"YYYY-MM-DDTHH:MM:SSZ","reason":"...","rescinded_at_utc":null|"YYYY-MM-DDTHH:MM:SSZ"}]}`.
2. Quorum MUST be computed only from approvals with `commit_sha` equal to the candidate release commit.
3. Missing keys (`role`, `approver`, `approved_at_utc`, `commit_sha`) are release-blocking.
4. Any veto with `rescinded_at_utc == null` forces `NO-GO`.

Approval verification commands:
1. `CANDIDATE_SHA="$(git rev-parse HEAD)"`
2. `jq -e --arg sha "${CANDIDATE_SHA}" '([.approvals[] | select(.commit_sha==$sha) | .role] | sort | unique) as $r | (["rollout_commander","parser_indexer_lead","rollback_approver"] - $r | length)==0 and ((($r|index("search_lead"))!=null) or (($r|index("observability_lead"))!=null))' artifacts/integrity/approvals.json >/dev/null`
3. `jq -e --arg sha "${CANDIDATE_SHA}" '([.approvals[] | select(.commit_sha==$sha and .role=="rollout_commander") | .approver] | unique | length)==1 and ([.approvals[] | select(.commit_sha==$sha and .role=="rollback_approver") | .approver] | unique | length)==1 and ([.approvals[] | select(.commit_sha==$sha and .role=="rollout_commander") | .approver][0] != [.approvals[] | select(.commit_sha==$sha and .role=="rollback_approver") | .approver][0])' artifacts/integrity/approvals.json >/dev/null`
4. `jq -e '([.vetoes[]? | select((.rescinded_at_utc // null) == null)] | length) == 0' artifacts/integrity/approvals.json >/dev/null`

## Go/No-Go Checklist

- [ ] Gates 0-6 all passed.
- [ ] No zero-test false-green logs.
- [ ] Deep rebuild finished with `phase=done`.
- [ ] `parse_version=TARGET_PARSE_VERSION` rollout target verified.
- [ ] Integrity counters satisfy all thresholds.
- [ ] Snapshot restore rehearsal succeeded in staging and produced `artifacts/integrity/rollback-rehearsal.log` and `artifacts/integrity/rollback-rehearsal.system.json`.
- [ ] Branch protection evidence exists at `artifacts/integrity/branch-protection.main.json` and all Gate 5 governance checks pass.
- [ ] Governance log exists at `artifacts/integrity/gate5-governance.log`.
- [ ] Real-corpus verification transcript exists at `artifacts/integrity/real-corpus-verification.txt`.
- [ ] Ownership section contains no unresolved placeholders.
- [ ] Ownership approvals are recorded in `artifacts/integrity/approvals.json`, schema-valid, quorum-valid, and veto-clean.

## Phase Plan

## Phase 1 (P0)
1. Implement Blockers A-F.
2. Add deterministic tests:
   - `crates/core/tests/live_parser_integrity_test.rs`
   - `crates/db/tests/indexer_integrity_test.rs`
   - `crates/db/tests/integrity_real_shape_fixtures.rs`
   - `crates/search/tests/source_message_index_integrity_test.rs`
3. Add deterministic fixture assets:
   - `crates/db/tests/golden_fixtures/integrity_nonzero_tool_index.jsonl`
   - `crates/db/tests/golden_fixtures/integrity_progress_nested_content.jsonl`
   - `crates/db/tests/golden_fixtures/integrity_tool_use_result_polymorphic.jsonl`
   - `crates/db/tests/golden_fixtures/integrity_type_substring_noise.jsonl`
   - `crates/db/tests/golden_fixtures/integrity_expected.json`
   - `crates/db/tests/golden_fixtures/integrity_fixture.sha256`

## Phase 2 (P0)
1. Implement provenance docs/contracts.
2. Implement persistent counters + integrity API block.
3. Add replay corpus assets:
   - `crates/db/tests/integrity_corpus_snapshot/`
   - `crates/db/tests/integrity_corpus_snapshot/manifest.json`
4. Add gate scripts:
   - `scripts/integrity/static-regression-checks.sh`
   - `scripts/integrity/replay.sh`
   - `scripts/integrity/assert-replay-thresholds.sh`
   - `scripts/integrity/ci-gates.sh`

## Phase 3 (P0)
1. Wire CI Gate 5 in `.github/workflows/ci.yml` with required `integrity-gates` job and artifact upload policy.
2. Run staged rollout rehearsal and rollback rehearsal.
3. Capture sign-off evidence in `artifacts/integrity/signoff.json` with keys: `commit_sha`, `ci_run_url`, `gate_sha256s`, `branch_protection_sha256`, `approvals_sha256`, `approvers`, `approved_at_utc`.

## Definition of Done

All MUST be true:
1. No imaginary-field parser/indexer path remains.
2. No substring-based type classification remains.
3. No synthetic source-role docs in default source index.
4. Provenance validation passes: every field in `integrity_expected.json.required_source_fields` maps to exactly one class in `{source, derived, derived_with_fallback}`.
5. Deterministic replay gates pass.
6. Full reindex/backfill completed with parse version bump applied.

## Out of Scope

1. UI redesign unrelated to data integrity.
2. Non-Claude provider harmonization in this release.
3. Feature work not required to satisfy this contract.
