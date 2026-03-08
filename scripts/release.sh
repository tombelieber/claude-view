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
  if ! EVIDENCE_QUICK=1 cargo run -p claude-view-core --bin evidence-audit --release; then
    echo ""
    echo "ERROR: Evidence audit failed — parser/indexer may not handle current JSONL schema."
    echo "Fix the drift or set SKIP_EVIDENCE=1 to bypass (NOT recommended)."
    exit 1
  fi
  echo ""
fi

cd "$ROOT/npx-cli"

# Bump version in npx-cli/package.json (no git tag from npm)
npm version "$BUMP" --no-git-tag-version
VERSION=$(node -p "require('./package.json').version")

cd "$ROOT"

# Bump Cargo.toml workspace version to match
sed -i '' "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml

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

# Bump landing page VERSION constant (stays in sync even though landing has its own release tag)
SITE_TS="$ROOT/apps/landing/src/data/site.ts"
if [ -f "$SITE_TS" ]; then
  sed -i '' "s/VERSION = '.*'/VERSION = '${VERSION}'/" "$SITE_TS"
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
