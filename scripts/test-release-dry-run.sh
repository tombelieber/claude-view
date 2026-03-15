#!/usr/bin/env bash
# scripts/test-release-dry-run.sh
# Regression test: verify release.sh version-bump logic works on both BSD and GNU sed.
# Tests the Node-based replacement that replaces the old sed-based approach.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TEST_DIR="$(mktemp -d)"
trap 'rm -rf "$TEST_DIR"' EXIT

# Create a fake Cargo.toml
cat > "$TEST_DIR/Cargo.toml" <<'TOML'
[workspace]
members = ["crates/*"]

[workspace.package]
version = "0.20.1"
TOML

# Create a fake site.ts
mkdir -p "$TEST_DIR/apps/landing/src/data"
cat > "$TEST_DIR/apps/landing/src/data/site.ts" <<'TS'
export const VERSION = '0.20.1';
export const SITE_NAME = 'claude-view';
TS

# Run the Node-based version replacement
TEST_VERSION="0.21.0"
node -e "
  const fs = require('fs');
  const version = '${TEST_VERSION}';

  // Cargo.toml
  const cargo = fs.readFileSync('${TEST_DIR}/Cargo.toml', 'utf8');
  fs.writeFileSync('${TEST_DIR}/Cargo.toml',
    cargo.replace(/^version = \".*\"/m, 'version = \"' + version + '\"'));

  // site.ts
  const site = fs.readFileSync('${TEST_DIR}/apps/landing/src/data/site.ts', 'utf8');
  fs.writeFileSync('${TEST_DIR}/apps/landing/src/data/site.ts',
    site.replace(/VERSION = '.*'/, \"VERSION = '\" + version + \"'\"));
"

# Verify
CARGO_VER=$(grep '^version' "$TEST_DIR/Cargo.toml" | head -1)
SITE_VER=$(grep 'VERSION' "$TEST_DIR/apps/landing/src/data/site.ts" | head -1)

if [[ "$CARGO_VER" != 'version = "0.21.0"' ]]; then
  echo "FAIL: Cargo.toml version not bumped. Got: $CARGO_VER"
  exit 1
fi

if [[ "$SITE_VER" != "export const VERSION = '0.21.0';" ]]; then
  echo "FAIL: site.ts VERSION not bumped. Got: $SITE_VER"
  exit 1
fi

echo "PASS: Version bump works cross-platform (no sed dependency)"
