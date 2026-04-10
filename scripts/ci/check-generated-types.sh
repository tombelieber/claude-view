#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

./scripts/generate-types.sh

git diff --exit-code -- apps/web/src/types/generated packages/shared/src/types/generated || {
  echo "ERROR: Generated types are out of date. To fix:"
  echo "  ./scripts/generate-types.sh"
  echo "  git add apps/web/src/types/generated packages/shared/src/types/generated"
  exit 1
}

echo "check-generated-types: OK"
