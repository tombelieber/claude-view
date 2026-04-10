#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
COMMON_GIT_DIR="$(git -C "$ROOT" rev-parse --git-common-dir)"
case "$COMMON_GIT_DIR" in
  /*) ;;
  *) COMMON_GIT_DIR="$ROOT/$COMMON_GIT_DIR" ;;
esac
COMMON_ROOT="$(cd "$COMMON_GIT_DIR/.." && pwd)"
WORKTREES_DIR="$COMMON_ROOT/.worktrees"

REMOVE_ORPHANED=0
PRUNE_ADMIN=0

usage() {
  cat <<'EOF'
Usage: bash scripts/worktree-storage.sh [--remove-orphaned] [--prune-admin]

Reports disk usage for git worktrees in this repo and highlights orphaned
directories still taking space under .worktrees/.

Options:
  --remove-orphaned  Delete .worktrees/* directories that are not reported by
                     `git worktree list`.
  --prune-admin      Run `git worktree prune --verbose` before reporting.
EOF
}

for arg in "$@"; do
  case "$arg" in
    --remove-orphaned) REMOVE_ORPHANED=1 ;;
    --prune-admin) PRUNE_ADMIN=1 ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [ "$PRUNE_ADMIN" -eq 1 ]; then
  git -C "$ROOT" worktree prune --verbose
fi

active_paths_file="$(mktemp)"
trap 'rm -f "$active_paths_file"' EXIT
git -C "$ROOT" worktree list --porcelain | sed -n 's/^worktree //p' >"$active_paths_file"

size_of() {
  local path="$1"
  du -sh "$path" 2>/dev/null | awk '{print $1}'
}

component_note() {
  local note_name="$1"
  local path="$2"
  if [ -d "$path" ]; then
    printf '%s=%s\n' "$note_name" "$(size_of "$path")"
  fi
}

echo "Repo disk report: $COMMON_ROOT"
for path in "$COMMON_ROOT/target" "$COMMON_ROOT/target-playwright" "$COMMON_ROOT/node_modules"; do
  [ -e "$path" ] || continue
  printf "  %-20s %s\n" "$(basename "$path")" "$(size_of "$path")"
done

if [ ! -d "$WORKTREES_DIR" ]; then
  echo
  echo "No .worktrees directory found."
  exit 0
fi

echo
printf "%-10s %-8s %-28s %s\n" "STATUS" "SIZE" "WORKTREE" "DETAILS"

orphan_count=0
for dir in "$WORKTREES_DIR"/*; do
  [ -d "$dir" ] || continue

  status="active"
  details=()

  if ! grep -Fxq "$dir" "$active_paths_file"; then
    status="orphaned"
    orphan_count=$((orphan_count + 1))
  fi

  if [ -f "$dir/.git" ]; then
    gitdir_line="$(sed -n 's/^gitdir: //p' "$dir/.git" | head -n 1)"
    if [ -n "$gitdir_line" ]; then
      case "$gitdir_line" in
        /*) admin_dir="$gitdir_line" ;;
        *) admin_dir="$dir/$gitdir_line" ;;
      esac
      if [ ! -e "$admin_dir" ]; then
        details+=("git-admin-missing")
      fi
    fi
  else
    details+=("missing-.git-marker")
  fi

  note="$(component_note "target" "$dir/target" || true)"
  [ -n "$note" ] && details+=("$note")
  note="$(component_note "target-playwright" "$dir/target-playwright" || true)"
  [ -n "$note" ] && details+=("$note")
  note="$(component_note "node_modules" "$dir/node_modules" || true)"
  [ -n "$note" ] && details+=("$note")

  dir_size="$(size_of "$dir")"
  if [ "$status" = "orphaned" ]; then
    printf "%-10s %-8s %-28s %s\n" "$status" "$dir_size" "$(basename "$dir")" "${details[*]:-not-registered}"
    if [ "$REMOVE_ORPHANED" -eq 1 ]; then
      rm -rf "$dir"
      echo "  removed $dir"
    fi
  else
    printf "%-10s %-8s %-28s %s\n" "$status" "$dir_size" "$(basename "$dir")" "${details[*]:-tracked}"
  fi
done

if [ "$orphan_count" -eq 0 ]; then
  echo
  echo "No orphaned worktree directories detected."
else
  echo
  echo "Detected $orphan_count orphaned worktree director$( [ "$orphan_count" -eq 1 ] && echo "y" || echo "ies" )."
  if [ "$REMOVE_ORPHANED" -eq 0 ]; then
    echo "Run: bash scripts/worktree-storage.sh --remove-orphaned"
  fi
fi
