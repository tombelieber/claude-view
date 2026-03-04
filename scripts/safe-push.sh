#!/bin/bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  ./scripts/safe-push.sh [--branch <name>] [--remote <name>] [--no-verify] [--backup-tag] [--dry-run]

Description:
  Pushes the current HEAD commit by exact SHA to refs/heads/<branch>, then verifies
  the remote branch tip matches that SHA.

Options:
  --branch <name>   Target branch (default: current branch)
  --remote <name>   Target remote (default: origin)
  --no-verify       Pass --no-verify to git push
  --backup-tag      Create a backup tag before push (backup/safe-push-<branch>-<timestamp>)
  --dry-run         Show actions without pushing
  -h, --help        Show this help
EOF
}

REMOTE="origin"
BRANCH=""
USE_NO_VERIFY=0
USE_BACKUP_TAG=0
DRY_RUN=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --branch)
      if [ "$#" -lt 2 ]; then
        echo "Missing value for --branch" >&2
        exit 1
      fi
      BRANCH="$2"
      shift 2
      ;;
    --remote)
      if [ "$#" -lt 2 ]; then
        echo "Missing value for --remote" >&2
        exit 1
      fi
      REMOTE="$2"
      shift 2
      ;;
    --no-verify)
      USE_NO_VERIFY=1
      shift
      ;;
    --backup-tag)
      USE_BACKUP_TAG=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

if [ -z "$BRANCH" ]; then
  BRANCH="$(git branch --show-current)"
fi

if [ -z "$BRANCH" ]; then
  echo "Unable to resolve branch. Provide --branch when in detached HEAD." >&2
  exit 1
fi

SHA="$(git rev-parse HEAD)"
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"

echo "safe-push: repo=$REPO_ROOT"
echo "safe-push: remote=$REMOTE"
echo "safe-push: branch=$BRANCH"
echo "safe-push: sha=$SHA"

if [ "$USE_BACKUP_TAG" -eq 1 ]; then
  TAG="backup/safe-push-${BRANCH}-${TIMESTAMP}"
  git tag -f "$TAG" "$SHA"
  echo "safe-push: created backup tag $TAG"
fi

PUSH_ARGS=()
if [ "$USE_NO_VERIFY" -eq 1 ]; then
  PUSH_ARGS+=(--no-verify)
fi

REFSPEC="${SHA}:refs/heads/${BRANCH}"

if [ "$DRY_RUN" -eq 1 ]; then
  echo "safe-push: dry-run mode"
  echo "safe-push: would run: git push ${PUSH_ARGS[*]:-} $REMOTE $REFSPEC"
  echo "safe-push: would verify: git rev-parse $REMOTE/$BRANCH == $SHA"
  exit 0
fi

git push ${PUSH_ARGS[@]+"${PUSH_ARGS[@]}"} "$REMOTE" "$REFSPEC"
git fetch "$REMOTE" "$BRANCH" --quiet

REMOTE_SHA="$(git rev-parse "$REMOTE/$BRANCH")"
if [ "$REMOTE_SHA" != "$SHA" ]; then
  echo "safe-push: verification failed" >&2
  echo "safe-push: expected remote SHA: $SHA" >&2
  echo "safe-push: actual remote SHA:   $REMOTE_SHA" >&2
  exit 1
fi

echo "safe-push: OK ($REMOTE/$BRANCH -> $SHA)"
