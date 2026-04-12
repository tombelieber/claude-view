#!/usr/bin/env bash
# scripts/dev-cleanup.sh
# Full dev-stack cleanup: kills port holders AND orphan process by name.
#
# Why both? `cleanupport` (port-only) misses orphan tsx/node/cargo-watch
# processes that crashed without releasing a port. They hold file locks,
# module caches, and can resurface as "Cannot find module './cjs/index.cjs'"
# ghost errors when the next `bun dev` starts.
#
# This script is the ONLY safe pre-dev hook. Run it before any `bun dev`.
#
# Usage:
#   bash scripts/dev-cleanup.sh          # kill ports + orphan processes (default)
#   bash scripts/dev-cleanup.sh --quiet  # only print if something was killed
#   bash scripts/dev-cleanup.sh --ports-only  # legacy behaviour
set -euo pipefail

QUIET=0
PORTS_ONLY=0
for arg in "$@"; do
  case "$arg" in
    --quiet) QUIET=1 ;;
    --ports-only) PORTS_ONLY=1 ;;
  esac
done

log() {
  if [ "$QUIET" -eq 0 ]; then
    echo "$@"
  fi
}

# Ports we own — keep in sync with package.json scripts and vite.config.ts.
# SHOULD be the single source of truth for dev ports.
# 47892=rust-server, 5173=vite-web, 3001=sidecar, 6006=storybook
PORTS=(47892 5173 3001 6006)

# Process name patterns that match our own stack only. We NEVER kill by
# bare "node" — that would nuke every Node process on the machine.
#
# Patterns matched with pgrep -f (command-line match):
#   sidecar/src/index.ts     — the dev sidecar (tsx watch)
#   sidecar/dist/index.js    — the prod sidecar (built)
#   claude-view-server       — our Rust binary name
#   cargo-watch.*claude-view-server — cargo watcher running our binary
PROCESS_PATTERNS=(
  'sidecar/src/index.ts'
  'sidecar/dist/index.js'
  'claude-view-server'
  'cargo-watch.*claude-view-server'
  # Agent-spawned orphans: storybook, esbuild, preview servers.
  # Scoped to this repo's node_modules to avoid killing other projects.
  'claude-view.*storybook dev'
  'claude-view.*esbuild.*--ping'
  'claude-view.*astro preview'
  'claude-view.*turbo.*preview'
)

killed_any=0

# 1. Kill by port
for port in "${PORTS[@]}"; do
  pids="$(lsof -ti "tcp:$port" 2>/dev/null || true)"
  if [ -n "$pids" ]; then
    log "  • port :$port held by PID $pids → killing"
    # Best-effort; `|| true` so a dead PID doesn't fail the script
    echo "$pids" | xargs kill -9 2>/dev/null || true
    killed_any=1
  fi
done

# 2. Kill orphan processes by name pattern
if [ "$PORTS_ONLY" -eq 0 ]; then
  for pattern in "${PROCESS_PATTERNS[@]}"; do
    # pgrep -f matches the full command line. -a prints pid + cmd for logging.
    # Exclude our own PID and shell's PID from the match set.
    matches="$(pgrep -f "$pattern" 2>/dev/null || true)"
    if [ -n "$matches" ]; then
      for pid in $matches; do
        # Skip self and parent (the dev-cleanup.sh invocation)
        if [ "$pid" = "$$" ] || [ "$pid" = "$PPID" ]; then
          continue
        fi
        # Safety: verify the process still exists before killing
        if kill -0 "$pid" 2>/dev/null; then
          cmd="$(ps -p "$pid" -o command= 2>/dev/null | head -c 80 || echo '?')"
          log "  • orphan PID $pid ($cmd...) → killing"
          kill -9 "$pid" 2>/dev/null || true
          killed_any=1
        fi
      done
    fi
  done
fi

if [ "$killed_any" -eq 0 ]; then
  log "  • nothing to clean up"
fi

# 3. Verify ports are actually free (race guard)
sleep 0.2
for port in "${PORTS[@]}"; do
  if lsof -ti "tcp:$port" >/dev/null 2>&1; then
    echo "✗ port :$port STILL in use after cleanup — manual intervention required:" >&2
    echo "    lsof -i :$port" >&2
    exit 1
  fi
done

exit 0
