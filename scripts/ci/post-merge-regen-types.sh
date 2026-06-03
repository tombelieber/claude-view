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
# IMPORTANT — generate-types.sh is NON-ATOMIC:
#   stage 1 (`cq test … export_bindings`) writes RAW ts-rs output to the tree
#     (double-quoted, single-line: `export type X = "a" | "b";`)
#   stage 2 (`biome check --write`) reformats it to project style
#     (single-quoted, multi-line, organized imports)
# So if generation does NOT run to completion — a cold/interrupted compile, a
# transient cargo failure, a concurrent regen racing the same files — the tree
# is left polluted with stage-1 RAW output. The previous version of this hook
# ran `generate-types.sh > /dev/null 2>&1` and `exit 0` on failure, which
# SILENTLY left ~190 raw .ts files dirty with zero explanation (the puller then
# faced an inexplicable giant diff). This version is robust:
#
# Behavior:
#  - Exits 0 always (never blocks git merge/pull)
#  - On generation FAILURE: restores the generated dirs (discards any raw
#    residue so the tree matches HEAD) and prints a LOUD warning to stderr with
#    the failure tail + manual-regen instruction — never a silent dirty tree
#  - On generation SUCCESS with drift: prints the follow-up banner + drift list

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

# Single source of truth for the generated TS output dirs (regen target +
# drift check + failure restore all read this — no drift between them).
GEN_DIRS=(
  apps/web/src/types/generated
  packages/shared/src/types/generated
)

# The generator command. Overridable for tests (dependency injection) so the
# failure/success paths can be exercised without a real multi-minute cargo run.
GENERATE_TYPES_CMD="${GENERATE_TYPES_CMD:-./scripts/generate-types.sh}"

# Skip if the merge didn't touch any Rust file.
# HEAD@{1} is the pre-merge HEAD, HEAD is post-merge.
if ! git diff --name-only 'HEAD@{1}' HEAD 2>/dev/null | grep -q '\.rs$'; then
  exit 0
fi

# Regenerate, capturing output to a log (diagnosable, not swallowed).
GEN_LOG="$(mktemp -t post-merge-regen.XXXXXX)"
if ! "$GENERATE_TYPES_CMD" >"$GEN_LOG" 2>&1; then
  # Generation did not complete (compile error, interruption, transient cargo
  # failure, concurrent regen). Discard any half-written RAW residue so the
  # working tree matches HEAD again — NEVER leave the puller with raw, dirty,
  # unformatted .ts files. Restore each dir independently: `git checkout -- A B`
  # is all-or-nothing on pathspec match, so one empty/renamed gen dir would
  # otherwise silently block restoring the other.
  for _gd in "${GEN_DIRS[@]}"; do
    git checkout -- "$_gd" 2>/dev/null || true
  done
  {
    echo ""
    echo "┌─────────────────────────────────────────────────────────────────────┐"
    echo "│  ⚠  Post-merge TS type regen FAILED — working tree was RESTORED.      │"
    echo "│                                                                       │"
    echo "│  Upstream changed Rust types but codegen could not finish (usually a  │"
    echo "│  cold or interrupted compile). Your generated .ts dirs were reset to  │"
    echo "│  HEAD, so you are NOT left with raw, unformatted output.              │"
    echo "│                                                                       │"
    echo "│  Regenerate manually when ready:                                      │"
    echo "│    ./scripts/generate-types.sh                                        │"
    echo "└─────────────────────────────────────────────────────────────────────┘"
    echo "  --- last lines of the failed regen ---"
    tail -n 10 "$GEN_LOG" 2>/dev/null | sed 's/^/  /'
    echo ""
  } >&2
  rm -f "$GEN_LOG"
  exit 0
fi
rm -f "$GEN_LOG"

# Generation succeeded. Check for drift against what was committed.
DRIFT=$(git diff --name-only -- "${GEN_DIRS[@]}" 2>/dev/null || true)

if [ -n "$DRIFT" ]; then
  cat <<EOF

┌─────────────────────────────────────────────────────────────────────┐
│  Generated TS types drifted after pull — upstream Rust structs      │
│  changed but types weren't regenerated. I've regenerated them for   │
│  you. Please commit as a follow-up:                                 │
│                                                                     │
│    git add ${GEN_DIRS[*]}
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
