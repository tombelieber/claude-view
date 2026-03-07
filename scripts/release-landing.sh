#!/bin/bash
set -euo pipefail

# Usage: ./scripts/release-landing.sh [patch|minor|major]
# Default: patch (0.1.0 → 0.1.1)
#
# Tags as landing-v{VERSION}, builds, and deploys to Cloudflare Pages.
# Separate version track from the main claude-view binary.

BUMP="${1:-patch}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LANDING="$ROOT/apps/landing"

# Read current version, bump it
cd "$LANDING"
npm version "$BUMP" --no-git-tag-version
VERSION=$(node -p "require('./package.json').version")

cd "$ROOT"

# Build (runs prebuild: og-image + llms-full.txt, then astro build)
echo "Building landing page v${VERSION}..."
bun run --filter @claude-view/landing build

# Commit and tag
git add apps/landing/package.json
git commit -m "release(landing): v${VERSION}"
git tag "landing-v${VERSION}"

echo ""
echo "Tagged landing-v${VERSION}. Next steps:"
echo "  git push origin HEAD --tags                    # push the tag"
echo "  cd apps/landing && bunx wrangler pages deploy dist  # deploy to Cloudflare"
echo ""
echo "Or deploy now:"
echo "  bun run deploy:landing"
