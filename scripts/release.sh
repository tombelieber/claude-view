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

# Commit and tag
git add npx-cli/package.json
git commit -m "release: v${VERSION}"
git tag "v${VERSION}"

echo ""
echo "Tagged v${VERSION}. Next steps:"
echo "  git push origin main --tags    # triggers CI build"
echo "  # wait for CI to finish, then:"
echo "  cd npx-cli && npm publish      # publish to npm"
