#!/bin/bash
# test/post-merge-regen-types.test.sh — verify post-merge hook failure handling
#
# Regression for the "~190 raw .ts files dirty after pull, zero explanation" bug.
# generate-types.sh is non-atomic (stage 1 writes RAW ts-rs, stage 2 biome
# reformats). The hook must:
#   - on generation FAILURE: restore the generated dirs (no raw residue) and
#     warn LOUDLY — never silently leave a dirty tree
#   - on generation SUCCESS with drift: keep the change + print a follow-up banner
#
# Hermetic: builds a throwaway git repo and injects a mock generator via
# GENERATE_TYPES_CMD. No cargo, no network. Fast (<1s).
#
# Override the hook under test with HOOK_UNDER_TEST=<path> (used to prove this
# test FAILS against the old silent-swallow hook).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
HOOK_SRC="${HOOK_UNDER_TEST:-$SCRIPT_DIR/scripts/ci/post-merge-regen-types.sh}"

PASS=0
FAIL=0
TOTAL=0
inc() { eval "$1=\$(( $1 + 1 ))"; }

check_eq() { # desc expected actual
  inc TOTAL
  if [[ "$2" == "$3" ]]; then echo "  PASS: $1"; inc PASS
  else echo "  FAIL: $1"; echo "    expected: $2"; echo "    actual:   $3"; inc FAIL; fi
}
check_contains() { # desc haystack needle
  inc TOTAL
  if printf '%s' "$2" | grep -qF -- "$3"; then echo "  PASS: $1"; inc PASS
  else echo "  FAIL: $1 (missing substring: $3)"; inc FAIL; fi
}

# ─── Hermetic temp repo with the layout the hook expects ───
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
cd "$TMP"
git init -q
git config user.email t@t.test
git config user.name tester
git config commit.gpgsign false
git config core.hooksPath "$TMP/.nohooks"   # isolate from any global lefthook

mkdir -p scripts/ci apps/web/src/types/generated packages/shared/src/types/generated
cp "$HOOK_SRC" scripts/ci/post-merge-regen-types.sh
chmod +x scripts/ci/post-merge-regen-types.sh

# Committed FORMATTED generated file (project style: single-quoted, multi-line).
FORMATTED=$'export type X =\n  | \'a\'\n  | \'b\';'
printf '%s\n' "$FORMATTED" > apps/web/src/types/generated/X.ts
echo "fn a() {}" > foo.rs
git add -A
git commit -q -m c1
# Second commit changes a .rs so HEAD@{1}..HEAD contains a .rs (guard passes).
echo "fn a() { /* changed */ }" > foo.rs
git add -A
git commit -q -m c2

HOOK=(bash scripts/ci/post-merge-regen-types.sh)

# ─── Case 1: generator FAILS after writing raw output (the bug scenario) ───
echo "Case 1: generation fails after writing raw output"
cat > "$TMP/fail-gen.sh" <<'MOCK'
#!/bin/bash
# simulate stage 1 raw write, then fail before biome (CWD = repo root)
printf 'export type X = "a" | "b";\n' > apps/web/src/types/generated/X.ts
echo "boom: simulated cargo compile error" >&2
exit 1
MOCK
chmod +x "$TMP/fail-gen.sh"

out1=$(GENERATE_TYPES_CMD="$TMP/fail-gen.sh" "${HOOK[@]}" 2>&1 || true)
restored=$(cat apps/web/src/types/generated/X.ts)
porcelain=$(git status --porcelain -- apps/web/src/types/generated packages/shared/src/types/generated)

check_eq      "raw residue restored to committed formatted output" "$FORMATTED" "$restored"
check_eq      "generated tree is clean after restore"               ""           "$porcelain"
check_contains "warns loudly (points to manual regen)"  "$out1" "generate-types.sh"
check_contains "warning says tree was restored"          "$out1" "RESTORED"
check_contains "surfaces the failure tail (diagnosable)" "$out1" "boom"

# ─── Case 2: generator SUCCEEDS with drift ───
echo "Case 2: generation succeeds with drift"
cat > "$TMP/ok-gen.sh" <<'MOCK'
#!/bin/bash
printf 'export type X =\n  | '\''a'\''\n  | '\''c'\'';\n' > apps/web/src/types/generated/X.ts
exit 0
MOCK
chmod +x "$TMP/ok-gen.sh"

out2=$(GENERATE_TYPES_CMD="$TMP/ok-gen.sh" "${HOOK[@]}" 2>&1 || true)
kept=$(cat apps/web/src/types/generated/X.ts)

check_contains "regenerated change is KEPT (not restored)" "$kept" "'c'"
check_contains "prints follow-up commit banner"            "$out2" "git add"
check_contains "lists the drifted file"                    "$out2" "X.ts"

# ─── Summary ───
echo ""
echo "═══════════════════════════════════"
echo "Results: $PASS/$TOTAL passed, $FAIL failed"
echo "═══════════════════════════════════"

[[ $FAIL -eq 0 ]]
