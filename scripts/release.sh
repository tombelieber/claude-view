#!/bin/bash
set -euo pipefail

# Usage: ./scripts/release.sh [patch|minor|major]
# Default: patch (0.1.0 → 0.1.1)

BUMP="${1:-patch}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# ─── Pre-release evidence audit ──────────────────────────────────
# Validates parser/indexer against real JSONL data structure.
# Catches drift before it ships. Skip with SKIP_EVIDENCE=1.
if [ "${SKIP_EVIDENCE:-0}" != "1" ]; then
  echo "Running evidence audit (JSONL schema guard)..."
  if ! cargo run -p claude-view-core --bin evidence-audit --release; then
    echo ""
    echo "ERROR: Evidence audit failed — parser pipeline invariants violated against real data."
    echo "Fix the drift or set SKIP_EVIDENCE=1 to bypass (NOT recommended)."
    exit 1
  fi
  echo ""
fi

# ─── Pre-release Storybook build check ──────────────────────────
# Verifies all stories compile and render without errors.
# Catches broken component stories before release. Skip with SKIP_STORYBOOK=1.
if [ "${SKIP_STORYBOOK:-0}" != "1" ]; then
  echo "Building Storybook (story compilation check)..."
  if ! (cd "$ROOT/apps/web" && bunx storybook build -o /tmp/storybook-release-check --quiet 2>&1); then
    echo ""
    echo "ERROR: Storybook build failed — broken stories block release."
    echo "Run 'bun run storybook' to debug. Skip with SKIP_STORYBOOK=1."
    exit 1
  fi
  rm -rf /tmp/storybook-release-check
  echo "Storybook build OK"
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
