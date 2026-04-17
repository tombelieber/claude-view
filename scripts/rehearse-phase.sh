#!/usr/bin/env bash
# scripts/rehearse-phase.sh
#
# Rehearsal harness for irreversible DB phases.
#
# For any phase that performs an irreversible change (table drop, column
# drop, etc.), this harness:
#   1. Copies the production DB to a temp workdir (never mutates prod)
#   2. Tags current git HEAD as the rollback target
#   3. Applies forward migrations on the copy
#   4. Smoke-tests the migrated copy
#   5. Checks out the tag and re-smoke-tests on the pre-migration snapshot
#   6. Writes a rehearsal log to be signed off before production execution
#
# A phase is registered by adding scripts/rehearse/<phase>.sh that defines
# phase_forward_migrations, phase_smoke_test, phase_rollback_check.
#
# Usage:
#   ./scripts/rehearse-phase.sh <phase> [--apply]
#
# Default is dry-run. Re-run with --apply to actually copy, migrate, and
# smoke-test. The production DB at ~/.claude-view/claude-view.db is NEVER
# mutated by this script — all work happens on copies in a temp workdir.
#
# After a rehearsal completes, commit the generated log to
# private/config/docs/plans/rehearsals/<phase>-<timestamp>.md (use TEMPLATE.md)
# with the observed wall-clock, any unexpected warnings, and a sign-off name.

set -euo pipefail

PHASE="${1:-}"
APPLY=0
for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    --help|-h)
      sed -n '2,/^set -euo/p' "$0" | sed -n 's/^# \{0,1\}//p' | sed '/^set /d'
      exit 0
      ;;
  esac
done

if [ -z "$PHASE" ]; then
  echo "ERROR: phase required. Run with --help for details."
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PHASE_SCRIPT="scripts/rehearse/$PHASE.sh"
if [ ! -f "$PHASE_SCRIPT" ]; then
  echo "ERROR: no phase script at $PHASE_SCRIPT"
  echo "Supported phases:"
  found=0
  for f in scripts/rehearse/*.sh; do
    [ -e "$f" ] || continue
    basename "$f" .sh | sed 's/^/  - /'
    found=1
  done
  [ "$found" = 0 ] && echo "  (none registered yet — add scripts/rehearse/<phase>.sh when the phase's forward migrations land)"
  exit 1
fi

# Phase scripts must define these:
#   phase_forward_migrations()  — run forward migrations on $REHEARSAL_DB
#   phase_smoke_test()          — boot app + run smoke test against $ACTIVE_DB
#   phase_rollback_check()      — post-restore smoke test against old code
# shellcheck source=/dev/null
source "$PHASE_SCRIPT"

PROD_DB="${CLAUDE_VIEW_DB:-$HOME/.claude-view/claude-view.db}"
STAMP="$(date +%Y%m%d-%H%M%S)"
WORKDIR="/tmp/rehearsal-$PHASE-$STAMP"
SNAPSHOT="$WORKDIR/snapshot-pre-$PHASE.db"
REHEARSAL_DB="$WORKDIR/rehearsal-$PHASE.db"
LOG_FILE="private/config/docs/plans/rehearsals/$PHASE-$STAMP.md"
TAG="rehearsal-$PHASE-pre-$STAMP"

bar() { printf -- "--- %s ---\n" "$*"; }

if [ ! -f "$PROD_DB" ]; then
  echo "ERROR: production DB not found at $PROD_DB"
  echo "Set CLAUDE_VIEW_DB to override, or boot claude-view once to create one."
  exit 1
fi

PROD_SIZE="$(du -h "$PROD_DB" | cut -f1)"
PROD_SHA="$(shasum -a 256 "$PROD_DB" | cut -d' ' -f1)"

echo ""
bar "Rehearsal: $PHASE"
echo "  Mode:        $([ "$APPLY" = 1 ] && echo APPLY || echo DRY-RUN)"
echo "  Source DB:   $PROD_DB ($PROD_SIZE, sha256=${PROD_SHA:0:16}...)"
echo "  Workdir:     $WORKDIR"
echo "  Git tag:     $TAG"
echo "  Log target:  $LOG_FILE"
echo ""

if [ "$APPLY" != 1 ]; then
  bar "DRY RUN — no side effects"
  echo "  Would:"
  echo "    1. Create $WORKDIR/"
  echo "    2. Copy source DB → $SNAPSHOT (rollback snapshot)"
  echo "    3. Copy source DB → $REHEARSAL_DB (working copy)"
  echo "    4. git tag $TAG (rollback target)"
  echo "    5. Apply forward migrations to $REHEARSAL_DB"
  echo "    6. Boot app against \$REHEARSAL_DB + run smoke test"
  echo "    7. git checkout $TAG (restore old code)"
  echo "    8. Boot app against \$SNAPSHOT + run smoke test (verifies rollback path)"
  echo "    9. Write rehearsal log to $LOG_FILE"
  echo ""
  echo "  Re-run with --apply to execute."
  exit 0
fi

trap 'echo ""; bar "FAILED — see $WORKDIR and $LOG_FILE for forensics"' ERR

bar "Step 1: Workdir + snapshots"
mkdir -p "$WORKDIR" "$(dirname "$LOG_FILE")"
cp "$PROD_DB" "$SNAPSHOT"
cp "$PROD_DB" "$REHEARSAL_DB"
chmod 0400 "$SNAPSHOT"
echo "  snapshot:  $SNAPSHOT ($(shasum -a 256 "$SNAPSHOT" | cut -d' ' -f1 | cut -c1-16)...)"
echo "  rehearsal: $REHEARSAL_DB"

bar "Step 2: Tag current git HEAD"
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  echo "  tag $TAG already exists — reusing"
else
  git tag "$TAG"
  echo "  tagged $TAG"
fi

START_TS="$(date +%s)"

bar "Step 3: Forward migrations"
export REHEARSAL_DB
export SNAPSHOT
MIG_START="$(date +%s)"
phase_forward_migrations
MIG_SEC=$(( $(date +%s) - MIG_START ))
echo "  took ${MIG_SEC}s"

bar "Step 4: Smoke test against migrated DB"
ACTIVE_DB="$REHEARSAL_DB"
export ACTIVE_DB
SMOKE1_START="$(date +%s)"
phase_smoke_test
SMOKE1_SEC=$(( $(date +%s) - SMOKE1_START ))
echo "  took ${SMOKE1_SEC}s"

bar "Step 5: Simulate restore (snapshot + old code)"
echo "  git checkout $TAG"
git checkout "$TAG"
ACTIVE_DB="$SNAPSHOT"
export ACTIVE_DB
SMOKE2_START="$(date +%s)"
phase_rollback_check
SMOKE2_SEC=$(( $(date +%s) - SMOKE2_START ))
echo "  took ${SMOKE2_SEC}s"
git checkout -
echo "  restored branch"

TOTAL_SEC=$(( $(date +%s) - START_TS ))

bar "Step 6: Write rehearsal log"
TEMPLATE="private/config/docs/plans/rehearsals/TEMPLATE.md"
if [ -f "$TEMPLATE" ]; then
  sed \
    -e "s|{{PHASE}}|$PHASE|g" \
    -e "s|{{DATE}}|$(date -u +%Y-%m-%dT%H:%M:%SZ)|g" \
    -e "s|{{REHEARSER}}|$(git config user.name 2>/dev/null || echo TBD)|g" \
    -e "s|{{PROD_DB_SIZE}}|$PROD_SIZE|g" \
    -e "s|{{PROD_DB_SHA256}}|$PROD_SHA|g" \
    -e "s|{{TAG}}|$TAG|g" \
    -e "s|{{MIG_SEC}}|$MIG_SEC|g" \
    -e "s|{{SMOKE1_SEC}}|$SMOKE1_SEC|g" \
    -e "s|{{SMOKE2_SEC}}|$SMOKE2_SEC|g" \
    -e "s|{{TOTAL_SEC}}|$TOTAL_SEC|g" \
    "$TEMPLATE" > "$LOG_FILE"
else
  cat > "$LOG_FILE" <<LOG
# Rehearsal log — $PHASE ($(date -u +%Y-%m-%dT%H:%M:%SZ))

(TEMPLATE.md not found — skeleton only. Fill in manually.)

- phase: $PHASE
- tag: $TAG
- migration time: ${MIG_SEC}s
- smoke-test (migrated): ${SMOKE1_SEC}s
- smoke-test (restored): ${SMOKE2_SEC}s
- total wall-clock: ${TOTAL_SEC}s
LOG
fi
echo "  wrote $LOG_FILE"

echo ""
bar "Rehearsal complete"
echo "  Fill in remaining fields in $LOG_FILE and commit."
echo "  The snapshot ($SNAPSHOT) is chmod 0400 — delete manually once satisfied."
