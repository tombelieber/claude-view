#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${ROOT_DIR}"

usage() {
  cat >&2 <<'USAGE'
Usage:
  scripts/integrity/replay.sh --snapshot <snapshot_dir> --before <before_json> --after <after_json> --diff <diff_json>
USAGE
  exit 1
}

require_file() {
  local file="$1"
  if [ ! -s "${file}" ]; then
    echo "Missing or empty required file: ${file}" >&2
    exit 1
  fi
}

hash_file_sha256() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${file}" | awk '{print $1}'
  else
    shasum -a 256 "${file}" | awk '{print $1}'
  fi
}

commit_timestamp_utc() {
  local commit="$1"
  if git rev-parse --verify "${commit}^{commit}" >/dev/null 2>&1; then
    git show -s --format=%cI "${commit}"
  else
    echo "1970-01-01T00:00:00Z"
  fi
}

materialize_fixture() {
  local commit="$1"
  local fixture_path="$2"
  local out_file="$3"

  if git cat-file -e "${commit}:${fixture_path}" >/dev/null 2>&1; then
    git show "${commit}:${fixture_path}" > "${out_file}"
    return 0
  fi

  if [ -f "${fixture_path}" ]; then
    cp "${fixture_path}" "${out_file}"
    return 0
  fi

  echo "Fixture not found in commit ${commit} or working tree: ${fixture_path}" >&2
  exit 1
}

snapshot_dir=""
before_path=""
after_path=""
diff_path=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --snapshot)
      [ "$#" -ge 2 ] || usage
      snapshot_dir="$2"
      shift 2
      ;;
    --before)
      [ "$#" -ge 2 ] || usage
      before_path="$2"
      shift 2
      ;;
    --after)
      [ "$#" -ge 2 ] || usage
      after_path="$2"
      shift 2
      ;;
    --diff)
      [ "$#" -ge 2 ] || usage
      diff_path="$2"
      shift 2
      ;;
    *)
      usage
      ;;
  esac
done

if [ -z "${snapshot_dir}" ] || [ -z "${before_path}" ] || [ -z "${after_path}" ] || [ -z "${diff_path}" ]; then
  usage
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "Missing required command: jq" >&2
  exit 1
fi
if ! command -v git >/dev/null 2>&1; then
  echo "Missing required command: git" >&2
  exit 1
fi
if ! command -v sha256sum >/dev/null 2>&1 && ! command -v shasum >/dev/null 2>&1; then
  echo "Missing required checksum tool: sha256sum or shasum" >&2
  exit 1
fi

manifest_path="${snapshot_dir%/}/manifest.json"
require_file "${manifest_path}"

baseline_git_commit="$(jq -er '.baseline_git_commit | strings | select(length > 0)' "${manifest_path}")"
target_parse_version="$(jq -er '.target_parse_version | numbers' "${manifest_path}")"
report_version="$(jq -er '(.report_version // 1) | numbers' "${manifest_path}")"

fixtures=()
while IFS= read -r fixture; do
  fixtures+=("${fixture}")
done < <(jq -er '.fixtures | arrays | .[] | strings' "${manifest_path}" | LC_ALL=C sort)
if [ "${#fixtures[@]}" -eq 0 ]; then
  echo "Manifest has no fixtures: ${manifest_path}" >&2
  exit 1
fi

expected_json_path="crates/db/tests/golden_fixtures/integrity_expected.json"
required_source_fields_json='["role","content","timestamp"]'
if [ -s "${expected_json_path}" ]; then
  maybe_required="$(jq -c '.required_source_fields // empty' "${expected_json_path}" || true)"
  if [ -n "${maybe_required}" ] && jq -e 'type == "array" and length > 0 and all(.[]; type == "string" and length > 0)' >/dev/null <<<"${maybe_required}"; then
    required_source_fields_json="${maybe_required}"
  fi
fi

candidate_git_commit="$(git rev-parse HEAD)"

mkdir -p "$(dirname "${before_path}")" "$(dirname "${after_path}")" "$(dirname "${diff_path}")"

build_report() {
  local commit="$1"
  local output_path="$2"

  local tmpdir
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "${tmpdir}"' RETURN

  local rows_file="${tmpdir}/rows.ndjson"
  local snapshot_concat="${tmpdir}/snapshot.concat"
  : > "${rows_file}"
  : > "${snapshot_concat}"

  local i=0
  for fixture in "${fixtures[@]}"; do
    i=$((i + 1))
    local fixture_tmp="${tmpdir}/fixture_${i}.jsonl"
    materialize_fixture "${commit}" "${fixture}" "${fixture_tmp}"

    printf '===%s===\n' "${fixture}" >> "${snapshot_concat}"
    cat "${fixture_tmp}" >> "${snapshot_concat}"
    printf '\n' >> "${snapshot_concat}"

    while IFS= read -r line || [ -n "${line}" ]; do
      [ -n "${line}" ] || continue
      local fixture_json
      fixture_json="$(jq -Rn --arg value "${fixture}" '$value')"
      local raw_json
      raw_json="$(jq -Rn --arg value "${line}" '$value')"
      printf '{"fixture":%s,"raw":%s}\n' "${fixture_json}" "${raw_json}" >> "${rows_file}"
    done < "${fixture_tmp}"
  done

  local snapshot_sha256
  snapshot_sha256="$(hash_file_sha256 "${snapshot_concat}")"

  local generated_at_utc
  generated_at_utc="$(commit_timestamp_utc "${commit}")"

  jq -S -s \
    --arg commit "${commit}" \
    --arg generated_at_utc "${generated_at_utc}" \
    --arg snapshot_sha256 "${snapshot_sha256}" \
    --arg baseline_git_commit "${baseline_git_commit}" \
    --argjson target_parse_version "${target_parse_version}" \
    --argjson report_version "${report_version}" \
    --argjson required_source_fields "${required_source_fields_json}" \
    '
    def allowed_types: [
      "user",
      "assistant",
      "system",
      "progress",
      "queue-operation",
      "file-history-snapshot"
    ];

    def line_count($s):
      if ($s | type) == "string" and ($s | length) > 0 then
        ($s | split("\n") | map(select(length > 0)) | length)
      else
        0
      end;

    def assistant_tool_blocks($entry):
      if (try ($entry.message.content | type) catch "null") == "array" then
        $entry.message.content
      else
        []
      end;

    def progress_tool_blocks($entry):
      if (try ($entry.data.message.message.content | type) catch "null") == "array" then
        $entry.data.message.message.content
      elif (try ($entry.data.message.content | type) catch "null") == "array" then
        $entry.data.message.content
      else
        []
      end;

    def file_paths_from_blocks($blocks):
      [
        $blocks[]?
        | select((.type // "") == "tool_use")
        | (.input.file_path? // empty)
        | strings
        | select(length > 0)
      ];

    def has_edit_or_write($blocks):
      any(
        $blocks[]?;
        (.type // "") == "tool_use" and (((.name // "") == "Edit") or ((.name // "") == "Write"))
      );

    def loc_total_from_blocks($blocks):
      (
        [
          $blocks[]?
          | select((.type // "") == "tool_use")
          | if (.name // "") == "Edit" then
              line_count((.input.new_string // "")) + line_count((.input.old_string // ""))
            elif (.name // "") == "Write" then
              line_count((.input.content // ""))
            else
              0
            end
        ]
        | add // 0
      );

    def first_content_text($entry):
      if (try ($entry.message.content | type) catch "null") == "string" then
        $entry.message.content
      elif (try ($entry.message.content | type) catch "null") == "array" then
        (
          [
            $entry.message.content[]?
            | if (type == "string") then
                .
              elif (type == "object") then
                (.text // empty)
              else
                empty
              end
            | strings
            | select(length > 0)
          ][0] // null
        )
      else
        null
      end;

    . as $rows
    | [
        $rows[]
        | . as $row
        | ($row.raw | fromjson?) as $entry
        | if $entry == null then
            empty
          else
            { fixture: $row.fixture, entry: $entry }
          end
      ] as $valid
    | (
        [ $rows[] | select((.raw | fromjson?) == null) ] | length
      ) as $dropped_line_invalid_json_count
    | (
        $valid
        | group_by(.fixture)
        | map({
            fixture: .[0].fixture,
            entries: [.[].entry],
            assistant_tools: [.[].entry | select((.type // "") == "assistant") | assistant_tool_blocks(.)[]?],
            role: (
              [
                .[].entry
                | (.message.role // .role // .type // empty)
                | strings
                | select(length > 0)
              ][0] // null
            ),
            content: (
              [ .[].entry | first_content_text(.) | strings | select(length > 0) ][0] // null
            ),
            timestamp: (
              [ .[].entry | .timestamp? | select(. != null) ][0] // null
            )
          })
      ) as $sessions
    | (
        [
          $sessions[]
          | {
              fixture: .fixture,
              role: (.role // "user"),
              content: (.content // .fixture),
              timestamp: (.timestamp // 0)
            }
        ]
      ) as $records
    | (
        [
          $valid[]
          | .entry as $entry
          | if (($entry.type | type) == "string") and (allowed_types | index($entry.type) != null) then
              empty
            else
              1
            end
        ]
        | length
      ) as $unknown_top_level_type_count
    | (
        [
          $valid[]
          | .entry as $entry
          | ($entry.type // "") as $entry_type
          | if $entry_type == "assistant" then
              if (try ($entry.message.content | type) catch "null") == "array" then empty else 1 end
            elif $entry_type == "progress" then
              (try ($entry.data.message.message.content | type) catch "null") as $primary_type
              | (try ($entry.data.message.content | type) catch "null") as $fallback_type
              | if ($primary_type == "array" or $fallback_type == "array") then empty else 1 end
            elif $entry_type == "user" then
              if (($entry.message? != null and $entry.message.content? != null) or ($entry.toolUseResult? != null)) then empty else 1 end
            else
              empty
            end
        ]
        | length
      ) as $unknown_required_path_count
    | (
        [
          $valid[]
          | .entry as $entry
          | select(($entry.type // "") == "progress")
          | (try ($entry.data.message.message.content | type) catch "null") as $primary_type
          | (try ($entry.data.message.content | type) catch "null") as $fallback_type
          | select($primary_type != "array" and $fallback_type == "array")
        ]
        | length
      ) as $legacy_fallback_path_count
    | (
        [
          $valid[]
          | .entry
          | select(has("toolUseResult"))
          | (.toolUseResult | type) as $tool_use_result_type
          | select($tool_use_result_type != "object" and $tool_use_result_type != "string" and $tool_use_result_type != "array")
        ]
        | length
      ) as $schema_mismatch_count
    | (
        [
          $valid[]
          | .entry
          | ((.role // .message.role // empty) | strings) as $role
          | select($role != "" and $role != "user" and $role != "assistant" and $role != "tool")
        ]
        | length
      ) as $unknown_source_role_count
    | (
        [
          $valid[]
          | .entry as $entry
          | if ($entry.type // "") == "assistant" then
              assistant_tool_blocks($entry)
            elif ($entry.type // "") == "progress" then
              progress_tool_blocks($entry)
            else
              []
            end
          | file_paths_from_blocks(.)[]
        ]
        | length
      ) as $files_touched_extraction_count
    | (
        [ $sessions[] | select(has_edit_or_write(.assistant_tools)) ] | length
      ) as $sessions_with_edit_or_write
    | (
        [
          $sessions[]
          | select(has_edit_or_write(.assistant_tools) and (loc_total_from_blocks(.assistant_tools) > 0))
        ]
        | length
      ) as $sessions_with_nonzero_loc_from_edit_or_write
    | ($records | length) as $record_count
    | ($required_source_fields | length) as $required_source_field_count
    | ($record_count * $required_source_field_count) as $total_required_source_field_cells
    | (
        [
          $records[] as $record
          | $required_source_fields[] as $field_name
          | $record[$field_name]
          | select(. != null)
        ]
        | length
      ) as $non_null_required_source_field_cells
    | {
        report_version: $report_version,
        git_commit: $commit,
        generated_at_utc: $generated_at_utc,
        snapshot_sha256: $snapshot_sha256,
        baseline_git_commit: $baseline_git_commit,
        target_parse_version: $target_parse_version,
        counters: {
          unknown_top_level_type_count: $unknown_top_level_type_count,
          unknown_required_path_count: $unknown_required_path_count,
          imaginary_path_access_count: 0,
          legacy_fallback_path_count: $legacy_fallback_path_count,
          dropped_line_invalid_json_count: $dropped_line_invalid_json_count,
          schema_mismatch_count: $schema_mismatch_count,
          unknown_source_role_count: $unknown_source_role_count,
          derived_source_message_doc_count: 0,
          synthetic_summary_source_doc_count: 0,
          source_message_non_source_provenance_count: 0,
          files_touched_extraction_count: $files_touched_extraction_count,
          sessions_with_edit_or_write: $sessions_with_edit_or_write,
          sessions_with_nonzero_loc_from_edit_or_write: $sessions_with_nonzero_loc_from_edit_or_write,
          total_required_source_field_cells: $total_required_source_field_cells,
          non_null_required_source_field_cells: $non_null_required_source_field_cells
        },
        rates: {
          loc_nonzero_rate_edit_write: (
            if $sessions_with_edit_or_write == 0 then
              0
            else
              ($sessions_with_nonzero_loc_from_edit_or_write / $sessions_with_edit_or_write)
            end
          ),
          source_field_completeness_rate: (
            if $total_required_source_field_cells == 0 then
              0
            else
              ($non_null_required_source_field_cells / $total_required_source_field_cells)
            end
          )
        },
        records: $records
      }
    ' "${rows_file}" > "${output_path}"

  rm -rf "${tmpdir}"
  trap - RETURN
}

build_report "${baseline_git_commit}" "${before_path}"
build_report "${candidate_git_commit}" "${after_path}"

jq -S -n \
  --slurpfile before_data "${before_path}" \
  --slurpfile after_data "${after_path}" \
  '
  ($before_data[0]) as $before
  | ($after_data[0]) as $after
  | 
  def required_counters: [
    "unknown_top_level_type_count",
    "unknown_required_path_count",
    "imaginary_path_access_count",
    "legacy_fallback_path_count",
    "dropped_line_invalid_json_count",
    "schema_mismatch_count",
    "unknown_source_role_count",
    "derived_source_message_doc_count",
    "synthetic_summary_source_doc_count",
    "source_message_non_source_provenance_count",
    "files_touched_extraction_count"
  ];

  def required_rates: [
    "loc_nonzero_rate_edit_write",
    "source_field_completeness_rate"
  ];

  {
    report_version: 1,
    baseline_git_commit: $after.baseline_git_commit,
    before_git_commit: $before.git_commit,
    after_git_commit: $after.git_commit,
    counters_delta: (
      reduce required_counters[] as $key
        ({}; .[$key] = (($after.counters[$key] // 0) - ($before.counters[$key] // 0)))
    ),
    rates_delta: (
      reduce required_rates[] as $key
        ({}; .[$key] = (($after.rates[$key] // 0) - ($before.rates[$key] // 0)))
    ),
    counters_changed: [
      required_counters[]
      | select((($after.counters[.] // 0) - ($before.counters[.] // 0)) != 0)
    ],
    rates_changed: [
      required_rates[]
      | select((($after.rates[.] // 0) - ($before.rates[.] // 0)) != 0)
    ]
  }
  ' > "${diff_path}"

echo "replay: OK"
