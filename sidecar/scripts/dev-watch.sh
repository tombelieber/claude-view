#!/usr/bin/env bash
# sidecar/scripts/dev-watch.sh
#
# Crash-aware dev supervisor for the sidecar. Replaces `node --watch`
# because node's built-in watcher (and nodemon, and tsx-watch) all
# SWALLOW CRASHES — they keep the parent process alive after any error
# and silently wait for a file change. That breaks `concurrently
# --kill-others-on-fail`: the sidecar is dead, frontend hangs at
# "connecting", no error surfaces, and you waste hours debugging.
#
# Why not use a node library?
#   - nodemon:  same bug (waits on crash, unless you pass --exitcrash,
#               but --exitcrash disables file-watch too — no middle ground).
#   - tsx watch: same crash-swallowing behaviour as node --watch.
#   - pm2:       production process manager, too heavy for dev.
#
# This script runs the sidecar via `node --watch` (for fast file-change
# reloads) BUT wraps it with a health-check loop that detects:
#   - boot-phase crash (port never opens within 15s) → exit non-zero
#   - runtime crash (port dies while we're running) → exit non-zero
# In both cases concurrently sees the non-zero exit and tears down the
# whole stack, surfacing the error to the user immediately.
#
# Successful file-change reloads DO reopen the port within ~500ms, so
# we poll liberally (1s interval) and give 15s grace on first boot.

set -u

SIDECAR_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$SIDECAR_DIR" || exit 1

PORT="${SIDECAR_PORT:-3001}"
BOOT_TIMEOUT_SECS=15
HEALTH_POLL_INTERVAL_SECS=2
# How many consecutive health failures before we declare the sidecar dead.
# File-change reloads briefly take the port down for ~300ms. At 2s interval
# that's 1 missed probe max. 3 gives us margin for slow reloads without
# false positives.
MAX_CONSECUTIVE_FAILURES=3

# Start the node --watch process in the background. It handles its own
# file-watch + restart-on-change behaviour; we only supervise liveness.
node --import tsx --watch src/index.ts &
NODE_PID=$!

# Forward signals so parent can cleanly terminate us + the child.
cleanup() {
  kill -TERM "$NODE_PID" 2>/dev/null || true
  wait "$NODE_PID" 2>/dev/null || true
  exit "${1:-0}"
}
trap 'cleanup 0' TERM INT

# Phase 1: wait for boot. Poll /health until it responds or timeout.
boot_start=$(date +%s)
while true; do
  # If node died during boot, surface that immediately.
  if ! kill -0 "$NODE_PID" 2>/dev/null; then
    wait "$NODE_PID" 2>/dev/null
    code=$?
    echo "[sidecar] ✗ node exited during boot (code $code)" >&2
    exit "${code:-1}"
  fi

  if curl -sf "http://localhost:$PORT/health" >/dev/null 2>&1; then
    break
  fi

  now=$(date +%s)
  if [ $((now - boot_start)) -ge "$BOOT_TIMEOUT_SECS" ]; then
    echo "[sidecar] ✗ boot-phase timeout: no response on :$PORT after ${BOOT_TIMEOUT_SECS}s" >&2
    echo "[sidecar]   Check sidecar logs above for the actual error." >&2
    cleanup 1
  fi

  sleep 0.5
done

# Phase 2: liveness watchdog. Poll /health every N seconds; if we get
# $MAX_CONSECUTIVE_FAILURES in a row, declare sidecar dead and exit.
consecutive_failures=0
while true; do
  sleep "$HEALTH_POLL_INTERVAL_SECS"

  # If node itself exited, propagate its exit code.
  if ! kill -0 "$NODE_PID" 2>/dev/null; then
    wait "$NODE_PID" 2>/dev/null
    code=$?
    echo "[sidecar] ✗ node process exited (code $code) — tearing down stack" >&2
    exit "${code:-1}"
  fi

  if curl -sf "http://localhost:$PORT/health" >/dev/null 2>&1; then
    consecutive_failures=0
    continue
  fi

  consecutive_failures=$((consecutive_failures + 1))
  if [ "$consecutive_failures" -ge "$MAX_CONSECUTIVE_FAILURES" ]; then
    echo "[sidecar] ✗ health check failed ${consecutive_failures}x in a row — sidecar unresponsive, tearing down" >&2
    cleanup 1
  fi
done
