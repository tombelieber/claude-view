#!/usr/bin/env bash
# CI gate: ensure no generated TypeScript types use bigint
# (JSON.parse returns number, not bigint — all numeric fields must use #[ts(type = "number")])
set -euo pipefail

GENERATED_DIRS=(
    "apps/web/src/types/generated"
    "packages/shared/src/types/generated"
)

found=0
for dir in "${GENERATED_DIRS[@]}"; do
    if [ -d "$dir" ]; then
        while IFS= read -r line; do
            echo "ERROR: bigint found in generated type: $line"
            found=1
        done < <(grep -rn 'bigint' "$dir" --include='*.ts' || true)
    fi
done

if [ "$found" -eq 1 ]; then
    echo ""
    echo "Fix: Add #[ts(type = \"number\")] to the Rust field, then re-run cargo test"
    exit 1
fi

echo "OK: No bigint in generated types"
