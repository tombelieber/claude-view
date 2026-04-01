#!/bin/bash
set -euo pipefail

# Usage: ./scripts/release.sh [patch|minor|major]
# Default: patch (0.1.0 → 0.1.1)

BUMP="${1:-patch}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# ─── Local CI ──────────────────────────────────────────────────────
# Runs ALL quality gates: TS lint/typecheck/test, Rust clippy/test,
# evidence audit, Storybook build, integrity gates.
# Skip with SKIP_CI=1 if you've already run ./scripts/ci-local.sh.
# Individual gates: SKIP_RUST=1, SKIP_TS=1, SKIP_EVIDENCE=1, etc.
if [ "${SKIP_CI:-0}" != "1" ]; then
  "$ROOT/scripts/ci-local.sh"
  echo ""
else
  echo "Skipping local CI (SKIP_CI=1)"
  echo ""
fi

cd "$ROOT/npx-cli"

# Bump version in npx-cli/package.json (no git tag from npm)
npm version "$BUMP" --no-git-tag-version
VERSION=$(node -p "require('./package.json').version")

cd "$ROOT"

# Bump Cargo.toml workspace version to match
node -e "
  const fs = require('fs');
  const cargo = fs.readFileSync('Cargo.toml', 'utf8');
  fs.writeFileSync('Cargo.toml',
    cargo.replace(/^version = \".*\"/m, 'version = \"${VERSION}\"'));
"

# Bump ALL workspace package.json files that have a "version" field.
# Excludes: node_modules, .git, .tmp, .worktrees, apps/landing (separate release track).
BUMPED_PKGS=()
while IFS= read -r pkg; do
  # Skip files without a top-level "version" field
  if ! node -e "
    const p = JSON.parse(require('fs').readFileSync('$pkg','utf8'));
    if (!p.version) process.exit(1);
  " 2>/dev/null; then
    continue
  fi

  node -e "
    const fs = require('fs');
    const pkg = JSON.parse(fs.readFileSync('$pkg', 'utf8'));
    pkg.version = '${VERSION}';
    fs.writeFileSync('$pkg', JSON.stringify(pkg, null, 2) + '\n');
  "
  BUMPED_PKGS+=("$pkg")
done < <(find "$ROOT" -name package.json \
  -not -path '*/node_modules/*' \
  -not -path '*/.git/*' \
  -not -path '*/.tmp/*' \
  -not -path '*/.worktrees/*' \
  -not -path '*/apps/landing/*' \
  | sort)

# Bump .claude-plugin/plugin.json version (not named package.json, so find misses it)
PLUGIN_JSON="$ROOT/packages/plugin/.claude-plugin/plugin.json"
if [ -f "$PLUGIN_JSON" ]; then
  node -e "
    const fs = require('fs');
    const pkg = JSON.parse(fs.readFileSync('${PLUGIN_JSON}', 'utf8'));
    pkg.version = '${VERSION}';
    fs.writeFileSync('${PLUGIN_JSON}', JSON.stringify(pkg, null, 2) + '\n');
  "
  BUMPED_PKGS+=("$PLUGIN_JSON")
fi

# Bump landing page VERSION constant (stays in sync even though landing has its own release tag)
SITE_TS="$ROOT/apps/landing/src/data/site.ts"
if [ -f "$SITE_TS" ]; then
  node -e "
    const fs = require('fs');
    const f = fs.readFileSync('${SITE_TS}', 'utf8');
    fs.writeFileSync('${SITE_TS}',
      f.replace(/VERSION = '.*'/, \"VERSION = '${VERSION}'\"));
  "
  BUMPED_PKGS+=("$SITE_TS")
fi

# Regenerate Cargo.lock with new version
cargo generate-lockfile --quiet

# Commit and tag — include Cargo files + all bumped package.json files + site.ts
git add Cargo.toml Cargo.lock "${BUMPED_PKGS[@]}"
git commit -m "release: v${VERSION}"
git tag "v${VERSION}"

echo ""
echo "Bumped ${#BUMPED_PKGS[@]} files to v${VERSION}:"
printf '  %s\n' "${BUMPED_PKGS[@]}"
echo ""
echo "Tagged v${VERSION}. Next steps:"
echo "  git push origin main --tags    # triggers CI build + auto npm publish"
