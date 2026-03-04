#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${ROOT_DIR}"

usage() {
  cat >&2 <<'USAGE'
Usage:
  scripts/integrity/assert-replay-thresholds.sh <before_json> <after_json> <expected_json> <candidate_sha>
USAGE
  exit 1
}

if [ "$#" -ne 4 ]; then
  usage
fi

before_json="$1"
after_json="$2"
expected_json="$3"
candidate_sha="$4"

if [ -z "${candidate_sha}" ]; then
  echo "Missing candidate sha" >&2
  exit 1
fi

for required in "${before_json}" "${after_json}" "${expected_json}"; do
  if [ ! -s "${required}" ]; then
    echo "Missing or empty JSON input: ${required}" >&2
    exit 1
  fi
  jq -e 'type == "object"' "${required}" >/dev/null
  done

require_string_key() {
  local file="$1"
  local key="$2"
  if ! jq -e --arg key "${key}" '.[$key] | type == "string" and length > 0' "${file}" >/dev/null; then
    echo "Missing or invalid string key '${key}' in ${file}" >&2
    exit 1
  fi
}

require_number_key() {
  local file="$1"
  local jq_path="$2"
  if ! jq -e "${jq_path} | type == \"number\"" "${file}" >/dev/null; then
    echo "Missing or invalid numeric key '${jq_path}' in ${file}" >&2
    exit 1
  fi
}

# Required top-level keys and types in before/after replay JSON.
for file in "${before_json}" "${after_json}"; do
  require_string_key "${file}" "git_commit"
  require_string_key "${file}" "generated_at_utc"
  require_string_key "${file}" "snapshot_sha256"
  require_string_key "${file}" "baseline_git_commit"
  require_number_key "${file}" '.report_version'
  require_number_key "${file}" '.target_parse_version'
  if ! jq -e '.counters | type == "object"' "${file}" >/dev/null; then
    echo "Missing counters object in ${file}" >&2
    exit 1
  fi
  if ! jq -e '.rates | type == "object"' "${file}" >/dev/null; then
    echo "Missing rates object in ${file}" >&2
    exit 1
  fi
done

required_counters=(
  unknown_top_level_type_count
  unknown_required_path_count
  imaginary_path_access_count
  legacy_fallback_path_count
  dropped_line_invalid_json_count
  schema_mismatch_count
  unknown_source_role_count
  derived_source_message_doc_count
  synthetic_summary_source_doc_count
  source_message_non_source_provenance_count
  files_touched_extraction_count
)

required_rates=(
  loc_nonzero_rate_edit_write
  source_field_completeness_rate
)

for counter_key in "${required_counters[@]}"; do
  require_number_key "${before_json}" ".counters.${counter_key}"
  require_number_key "${after_json}" ".counters.${counter_key}"
done

for rate_key in "${required_rates[@]}"; do
  require_number_key "${before_json}" ".rates.${rate_key}"
  require_number_key "${after_json}" ".rates.${rate_key}"
done

# Required expected keys and types.
if ! jq -e '.required_source_fields | type == "array" and length > 0 and all(.[]; type == "string" and length > 0)' "${expected_json}" >/dev/null; then
  echo "Missing or invalid expected.required_source_fields" >&2
  exit 1
fi
if ! jq -e '.counters | type == "object"' "${expected_json}" >/dev/null; then
  echo "Missing expected.counters object" >&2
  exit 1
fi
if ! jq -e '.rates | type == "object"' "${expected_json}" >/dev/null; then
  echo "Missing expected.rates object" >&2
  exit 1
fi

expected_counter_keys=(
  unknown_source_role_count
  unknown_top_level_type_count
  unknown_required_path_count
  legacy_fallback_path_count
  min_files_touched_extraction_count
)

expected_rate_keys=(
  min_loc_nonzero_rate_edit_write
  min_source_field_completeness_rate
)

for counter_key in "${expected_counter_keys[@]}"; do
  require_number_key "${expected_json}" ".counters.${counter_key}"
done

for rate_key in "${expected_rate_keys[@]}"; do
  require_number_key "${expected_json}" ".rates.${rate_key}"
done

# Manifest baseline must match replay-before baseline.
manifest_path="crates/db/tests/integrity_corpus_snapshot/manifest.json"
if [ ! -s "${manifest_path}" ]; then
  echo "Missing or empty manifest: ${manifest_path}" >&2
  exit 1
fi
manifest_baseline_git_commit="$(jq -er '.baseline_git_commit | strings | select(length > 0)' "${manifest_path}")"
before_baseline_git_commit="$(jq -er '.baseline_git_commit | strings | select(length > 0)' "${before_json}")"
if [ "${manifest_baseline_git_commit}" != "${before_baseline_git_commit}" ]; then
  echo "Manifest baseline_git_commit mismatch: manifest=${manifest_baseline_git_commit} before=${before_baseline_git_commit}" >&2
  exit 1
fi

jq -e -n \
  --arg candidate_sha "${candidate_sha}" \
  --slurpfile before_data "${before_json}" \
  --slurpfile after_data "${after_json}" \
  --slurpfile expected_data "${expected_json}" \
  '
  ($before_data[0]) as $before
  | ($after_data[0]) as $after
  | ($expected_data[0]) as $expected
  |
  ($after.counters.imaginary_path_access_count == 0)
  and ($after.counters.synthetic_summary_source_doc_count == 0)
  and ($after.counters.derived_source_message_doc_count == 0)
  and ($after.counters.source_message_non_source_provenance_count == 0)
  and ($after.counters.unknown_source_role_count == $expected.counters.unknown_source_role_count)
  and ($after.counters.unknown_top_level_type_count == $expected.counters.unknown_top_level_type_count)
  and ($after.counters.unknown_required_path_count == $expected.counters.unknown_required_path_count)
  and ($after.counters.legacy_fallback_path_count == $expected.counters.legacy_fallback_path_count)
  and ($after.rates.loc_nonzero_rate_edit_write >= $expected.rates.min_loc_nonzero_rate_edit_write)
  and ($after.counters.files_touched_extraction_count >= $expected.counters.min_files_touched_extraction_count)
  and ($after.rates.source_field_completeness_rate >= $expected.rates.min_source_field_completeness_rate)
  and ($before.baseline_git_commit == $after.baseline_git_commit)
  and ($before.git_commit == $before.baseline_git_commit)
  and ($after.git_commit == $candidate_sha)
  ' >/dev/null

echo "assert-replay-thresholds: OK"
