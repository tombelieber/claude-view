#!/usr/bin/env bash
# scripts/ramdisk-target.sh — create/mount a RAM-disk target/ for faster builds
#
# Usage:
#   source scripts/ramdisk-target.sh              # mount + export CARGO_TARGET_DIR
#   source scripts/ramdisk-target.sh unmount      # tear down (preserves disk target/)
#
# After sourcing, cargo/cq will use the RAM-disk target. Existing target/
# on APFS is left alone. Reboot clears the RAM disk automatically.
#
# Benchmarked 2026-04-14 on M5 Max + 128 GB RAM:
#   B1 touch-rebuild   -17%   vs APFS target
#   B2 clean-check     -16%
#   B3 fanout-check    -42%   ← biggest win, workspace recheck
#   B4 test-compile    -10%
#
# Why it helps: APFS journal + Spotlight + Time Machine all watch writes
# to target/. 41 GB of .o files is a LOT for Spotlight to index. RAM disk
# bypasses all of it.

set -euo pipefail

VOLUME="/Volumes/cv-ramdisk"
SIZE_GB="${CV_RAMDISK_SIZE_GB:-64}"
SIZE_SECTORS=$((SIZE_GB * 1024 * 1024 * 2))

if [ "${1:-}" = "unmount" ]; then
  if mountpoint -q "$VOLUME" 2>/dev/null || df "$VOLUME" >/dev/null 2>&1; then
    echo "Detaching $VOLUME..."
    diskutil eject "$VOLUME" 2>&1 | tail -3
  fi
  unset CARGO_TARGET_DIR
  echo "unset CARGO_TARGET_DIR"
  return 0 2>/dev/null || exit 0
fi

if df "$VOLUME" >/dev/null 2>&1; then
  AVAIL=$(df -h "$VOLUME" | tail -1 | awk '{print $4}')
  echo "$VOLUME already mounted (avail: $AVAIL)"
else
  echo "Creating ${SIZE_GB} GiB APFS RAM disk at $VOLUME..."
  DEV=$(hdiutil attach -nomount ram://"$SIZE_SECTORS" | tr -d ' \t')
  diskutil erasevolume APFS cv-ramdisk "$DEV" >/dev/null 2>&1
  echo "$VOLUME ready."
fi

export CARGO_TARGET_DIR="$VOLUME/target"
mkdir -p "$CARGO_TARGET_DIR"
echo "export CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
echo ""
echo "Now run: cargo build / cq build / bun dev"
echo "To unmount: source scripts/ramdisk-target.sh unmount"
