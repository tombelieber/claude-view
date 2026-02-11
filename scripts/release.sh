#!/bin/bash
set -euo pipefail

# Usage: ./scripts/release.sh [patch|minor|major]
# Default: patch (0.1.0 → 0.1.1)

BUMP="${1:-patch}"

cd "$(dirname "$0")/../npx-cli"

# Bump version in npx-cli/package.json (no git tag from npm)
npm version "$BUMP" --no-git-tag-version
VERSION=$(node -p "require('./package.json').version")

cd ..

# Bump Cargo.toml workspace version to match
sed -i '' "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml

# Bump root package.json version to match
node -e "
const fs = require('fs');
const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8'));
pkg.version = '${VERSION}';
fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
"

# Regenerate Cargo.lock with new version
cargo generate-lockfile --quiet

# Commit and tag
git add npx-cli/package.json Cargo.toml package.json Cargo.lock
git commit -m "release: v${VERSION}"
git tag "v${VERSION}"

echo ""
echo "Tagged v${VERSION}. Next steps:"
echo "  git push origin main --tags    # triggers CI build + auto npm publish"
