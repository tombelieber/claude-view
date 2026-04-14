#!/usr/bin/env bash
# scripts/ci/post-merge-regen-types.sh
#
# Runs after `git pull` / `git merge`. If upstream changed any .rs file with
# a #[derive(TS)] type, regenerate TS types and warn if they drifted.
#
# This shifts drift detection LEFT: instead of the next pusher paying the
# cost of upstream's un-synced codegen, we catch it at pull time and tell
# the puller to commit the regen as a follow-up.
#
# Behavior:
#  - Exits 0 always (doesn't block git merge/pull)
#  - Prints a warning banner if drift detected
#  - Lists the drifted files so user can `git add` + commit them

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

# Skip if the merge didn't touch any Rust file
# HEAD@{1} is the pre-merge HEAD, HEAD is post-merge
if ! git diff --name-only 'HEAD@{1}' HEAD 2>/dev/null | grep -q '\.rs$'; then
  exit 0
fi

# Regenerate types silently
if ! ./scripts/generate-types.sh > /dev/null 2>&1; then
  # Generation failed (likely compile error from upstream) — don't block
  exit 0
fi

# Check for drift against what was committed
DRIFT=$(git diff --name-only -- \
  apps/web/src/types/generated \
  packages/shared/src/types/generated 2>/dev/null || true)

if [ -n "$DRIFT" ]; then
  cat <<EOF

┌─────────────────────────────────────────────────────────────────────┐
│  Generated TS types drifted after pull — upstream Rust structs      │
│  changed but types weren't regenerated. I've regenerated them for   │
│  you. Please commit as a follow-up:                                 │
│                                                                     │
│    git add apps/web/src/types/generated packages/shared/src/types/generated
│    git commit -m 'chore: regenerate TS types after pull'            │
│                                                                     │
│  Drifted files:                                                     │
EOF
  echo "$DRIFT" | while IFS= read -r f; do
    echo "│    $f"
  done
  echo "└─────────────────────────────────────────────────────────────────────┘"
  echo ""
fi

exit 0
