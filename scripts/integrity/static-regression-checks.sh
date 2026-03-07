#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${ROOT_DIR}"

failures=0

check_forbidden_pattern() {
  local description="$1"
  local pattern="$2"
  local file="$3"

  if matches="$(rg -n --no-heading --fixed-strings "${pattern}" "${file}" || true)" && [ -n "${matches}" ]; then
    echo "ERROR: ${description}"
    echo "${matches}"
    echo ""
    failures=1
  fi
}

check_forbidden_regex() {
  local description="$1"
  local regex="$2"
  local file="$3"

  if matches="$(rg -n --no-heading -U -e "${regex}" "${file}" || true)" && [ -n "${matches}" ]; then
    echo "ERROR: ${description}"
    echo "${matches}"
    echo ""
    failures=1
  fi
}

# 1) Forbidden top-level LOC read path: value.get("input") on the full assistant line.
check_forbidden_regex \
  'Forbidden top-level LOC path detected (extract_loc_from_tool_use -> value.get("input")).' \
  '(?s)fn\\s+extract_loc_from_tool_use\\s*\\([^)]*\\)\\s*->\\s*\\(u32,\\s*u32\\).*?value\\.get\\("input"\\)' \
  "crates/db/src/indexer_parallel.rs"

# 2) Forbidden substring-based type classification in live_parser.
check_forbidden_pattern \
  'Forbidden substring-type finder detected in live_parser (type_user uses bare "user" needle).' \
  'type_user: memmem::Finder::new(b"\"user\""),' \
  "crates/core/src/live_parser.rs"
check_forbidden_pattern \
  'Forbidden substring-type finder detected in live_parser (type_assistant uses bare "assistant" needle).' \
  'type_assistant: memmem::Finder::new(b"\"assistant\""),' \
  "crates/core/src/live_parser.rs"
check_forbidden_pattern \
  'Forbidden substring-type finder detected in live_parser (type_system uses bare "system" needle).' \
  'type_system: memmem::Finder::new(b"\"system\""),' \
  "crates/core/src/live_parser.rs"
check_forbidden_pattern \
  'Forbidden substring-type finder detected in live_parser (type_progress uses bare "progress" needle).' \
  'type_progress: memmem::Finder::new(b"\"progress\""),' \
  "crates/core/src/live_parser.rs"
check_forbidden_pattern \
  'Forbidden substring-type finder detected in live_parser (type_summary uses bare "summary" needle).' \
  'type_summary: memmem::Finder::new(b"\"summary\""),' \
  "crates/core/src/live_parser.rs"
check_forbidden_pattern \
  'Forbidden substring-type finder detected in live_parser (type_result uses bare "result" needle).' \
  'type_result: memmem::Finder::new(b"\"result\""),' \
  "crates/core/src/live_parser.rs"
check_forbidden_regex \
  "Forbidden substring-based type dispatch still present in parse_single_line." \
  'finders\\.type_(result|progress|summary|user|assistant|system)\\.find\\(raw\\)' \
  "crates/core/src/live_parser.rs"

# 3) Forbidden synthetic source role indexing pattern.
check_forbidden_regex \
  'Forbidden synthetic source role "summary" indexing pattern in db indexer.' \
  'role:\\s*"summary"\\.to_string\\(\\)' \
  "crates/db/src/indexer_parallel.rs"

if [ "${failures}" -ne 0 ]; then
  echo "static-regression-checks: FAILED"
  exit 1
fi

echo "static-regression-checks: OK"
