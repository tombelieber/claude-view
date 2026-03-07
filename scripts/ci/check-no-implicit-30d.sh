#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

failures=0

check_forbidden_pattern() {
  local description="$1"
  local pattern="$2"
  local file="$3"

  if matches="$(rg -n --no-heading --fixed-strings "$pattern" "$file" || true)" && [ -n "$matches" ]; then
    echo "ERROR: $description"
    echo "$matches"
    echo ""
    failures=1
  fi
}

check_forbidden_regex() {
  local description="$1"
  local regex="$2"
  local file="$3"

  if matches="$(rg -n --no-heading -e "$regex" "$file" || true)" && [ -n "$matches" ]; then
    echo "ERROR: $description"
    echo "$matches"
    echo ""
    failures=1
  fi
}

# 1) Block implicit 30-day clamp in in-scope insights handler.
check_forbidden_regex \
  "Implicit 30d clamp found in insights handler." \
  'query\.from\.unwrap_or\(\s*now\s*-\s*30\s*\*\s*86400\s*\)' \
  "crates/server/src/routes/insights.rs"

# 2) Block implicit trends 6mo default in query contract.
check_forbidden_regex \
  "Implicit trends default range hook still present." \
  '#\[\s*serde\s*\(\s*default\s*=\s*"default_range"\s*\)\s*\]' \
  "crates/server/src/routes/insights.rs"
check_forbidden_regex \
  "Implicit trends 6mo default function still present." \
  'fn\s+default_range\s*\(\)\s*->\s*String' \
  "crates/server/src/routes/insights.rs"
check_forbidden_pattern \
  "Implicit trends 6mo literal default still present." \
  '"6mo".to_string()' \
  "crates/server/src/routes/insights.rs"

# 3) Block legacy redirect from /insights to /analytics tab.
check_forbidden_regex \
  "Legacy /insights redirect to /analytics?tab=insights is still present." \
  '/analytics\?tab=insights' \
  "apps/web/src/router.tsx"

if [ "$failures" -ne 0 ]; then
  echo "check-no-implicit-30d: FAILED"
  exit 1
fi

echo "check-no-implicit-30d: OK"
