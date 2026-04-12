#!/bin/bash
# test/cq-nextest-routing.sh — verify cq nextest routing logic
#
# Uses mock perl/cargo-nextest to test the arg rewriting without
# actually compiling anything. Fast (<1s).
#
# How it works: cq ends with `exec perl ... "$LOCK_FILE" "$CARGO" "$@"`.
# We replace perl with a mock that echoes the final cargo command.
# This lets us verify the $@ rewriting in isolation.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CQ="$SCRIPT_DIR/scripts/cq"

PASS=0
FAIL=0
TOTAL=0

# Helper: safe arithmetic increment (((x++)) returns exit 1 when x starts at 0,
# which trips set -e. Use this instead.)
inc() { eval "$1=\$(( $1 + 1 ))"; }

# ─── Setup mock environment ───
MOCK_DIR=$(mktemp -d)
trap 'rm -rf "$MOCK_DIR"' EXIT

# Mock perl: extract the cargo command that would be executed
# cq invokes: exec perl -MFcntl=... -MPOSIX=... -e '...' $LOCK $CARGO $@
# We skip the perl flags and -e script, then echo lock_file + cargo + args
cat > "$MOCK_DIR/perl" << 'MOCK'
#!/bin/bash
# Skip perl module flags and -e script
while [[ $# -gt 0 ]]; do
  case "$1" in
    -M*) shift ;;
    -e)  shift; shift ;;  # -e '<script>'
    *)   break ;;
  esac
done
# $1=lock_file, $2=cargo, rest=rewritten args
shift  # skip lock_file
cargo_bin="$1"; shift
echo "CARGO_CMD: $(basename "$cargo_bin") $*"
MOCK
chmod +x "$MOCK_DIR/perl"

# Mock cargo-nextest: just needs to exist so `command -v` finds it
cat > "$MOCK_DIR/cargo-nextest" << 'MOCK'
#!/bin/bash
echo "mock-nextest $*"
MOCK
chmod +x "$MOCK_DIR/cargo-nextest"

# ─── Test helpers ───
assert_eq() {
  local desc="$1" expected="$2" actual="$3"
  inc TOTAL
  if [[ "$actual" == "$expected" ]]; then
    echo "  PASS: $desc"
    inc PASS
  else
    echo "  FAIL: $desc"
    echo "    expected: $expected"
    echo "    actual:   $actual"
    inc FAIL
  fi
}

run_cq() {
  # Run cq with mock perl first in PATH (so `exec perl` hits our mock)
  # and mock cargo-nextest available
  PATH="$MOCK_DIR:$PATH" "$CQ" "$@" 2>/dev/null || true
}

run_cq_no_nextest() {
  # Run cq with mock perl but WITHOUT cargo-nextest on PATH.
  # Build a path that excludes:
  #   - MOCK_DIR itself (avoid old mock stacking)
  #   - any directory that contains cargo-nextest (hides the real system binary)
  local clean_path=""
  while IFS=: read -ra parts; do
    for p in "${parts[@]}"; do
      [[ "$p" == "$MOCK_DIR" ]] && continue
      [[ -x "$p/cargo-nextest" ]] && continue
      clean_path="${clean_path:+$clean_path:}$p"
    done
  done <<< "$PATH"
  # Mock dir comes first (provides mock perl), but has no cargo-nextest.
  # Use trap to restore on interrupt (I1: teardown atomicity).
  mv "$MOCK_DIR/cargo-nextest" "$MOCK_DIR/_cargo-nextest.bak"
  local _restore="mv '$MOCK_DIR/_cargo-nextest.bak' '$MOCK_DIR/cargo-nextest'"
  trap "$_restore; trap - RETURN" RETURN
  PATH="$MOCK_DIR${clean_path:+:$clean_path}" "$CQ" "$@" 2>/dev/null || true
  eval "$_restore"
  trap - RETURN
}

# ─── Test 1: cq test routes through nextest when installed ───
echo "Test 1: cq test should route through nextest"
result=$(run_cq test -p claude-view-types)
assert_eq "rewrites to nextest run" \
  "CARGO_CMD: cargo nextest run -p claude-view-types" \
  "$result"

# ─── Test 2: cq test with -- separator falls back to cargo test ───
echo "Test 2: cq test with -- should fall back to cargo test"
result=$(run_cq test -p claude-view-types -- --nocapture)
assert_eq "keeps cargo test with -- args" \
  "CARGO_CMD: cargo test -p claude-view-types -- --nocapture" \
  "$result"

# ─── Test 3: cq test --doc falls back to cargo test ───
echo "Test 3: cq test --doc should fall back to cargo test"
result=$(run_cq test --doc -p claude-view-types)
assert_eq "keeps cargo test for --doc" \
  "CARGO_CMD: cargo test --doc -p claude-view-types" \
  "$result"

# ─── Test 4: cq test falls back when nextest not installed ───
echo "Test 4: cq test should fall back when nextest not installed"
result=$(run_cq_no_nextest test -p claude-view-types)
assert_eq "falls back to cargo test" \
  "CARGO_CMD: cargo test -p claude-view-types" \
  "$result"

# ─── Test 5: cq test --workspace routes through nextest ───
echo "Test 5: cq test --workspace should route through nextest"
result=$(run_cq test --workspace)
assert_eq "workspace test uses nextest" \
  "CARGO_CMD: cargo nextest run --workspace" \
  "$result"

# ─── Test 6: --doc before -- falls back (I2 edge case) ───
echo "Test 6: cq test --doc -- --other should fall back (--doc seen first)"
result=$(run_cq test --doc -- --other-arg)
assert_eq "--doc triggers fallback before -- is reached" \
  "CARGO_CMD: cargo test --doc -- --other-arg" \
  "$result"

# ─── Test 7: -- before --doc falls back (I2 edge case) ───
echo "Test 7: cq test -- --doc should fall back (-- seen first)"
result=$(run_cq test -p claude-view-types -- --doc)
assert_eq "-- triggers fallback before --doc is reached" \
  "CARGO_CMD: cargo test -p claude-view-types -- --doc" \
  "$result"

# ─── Test 8: non-test commands are unaffected ───
echo "Test 8: cq check should be unaffected by nextest routing"
result=$(run_cq check)
assert_eq "check is unchanged" \
  "CARGO_CMD: cargo check" \
  "$result"

# ─── Summary ───
echo ""
echo "═══════════════════════════════════"
echo "Results: $PASS/$TOTAL passed, $FAIL failed"
echo "═══════════════════════════════════"

[[ $FAIL -eq 0 ]]
