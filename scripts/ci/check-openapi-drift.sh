#!/usr/bin/env bash
# OpenAPI drift check — ensures every registered route handler has a
# corresponding entry in openapi.rs paths(...).
#
# Catches the silent drift where a new endpoint is added to a router()
# function but never annotated with #[utoipa::path] / registered in openapi.rs.
#
# Exit 0 = no drift, exit 1 = drift detected.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ROUTES_DIR="$ROOT/crates/server/src/routes"
OPENAPI_RS="$ROOT/crates/server/src/openapi.rs"

# Known exemptions — endpoints that legitimately skip OpenAPI docs.
EXEMPT="ws_terminal_handler ws_session_handler ws_subagent_terminal_handler ws_proxy_handler metrics_handler generate_key revoke_key"

# ── Step 1: Extract handler names from .route() calls ──
# Uses perl to properly handle multi-line .route() calls and chained
# methods like .route("/path", get(a).post(b).delete(c))
registered=$(
  cat "$ROUTES_DIR"/*.rs "$ROUTES_DIR"/*/*.rs 2>/dev/null |
  perl -0777 -ne '
    # Strip line comments
    s|//[^\n]*||g;
    # Find all .route( blocks — match from .route( to the next ; or .route(
    while (/\.route\((.*?)(?=\.route\(|\.with_state|\.layer\(|\.fallback|;\s)/sg) {
      my $block = $1;
      # Extract handler names from get(h), post(h), put(h), delete(h), patch(h)
      while ($block =~ /(?:get|post|put|delete|patch)\(([a-zA-Z_:]+)\)/g) {
        my $h = $1;
        $h =~ s/.*:://;  # strip module prefix (handlers::foo → foo)
        print "$h\n";
      }
    }
  ' |
  sort -u
)

# ── Step 2: Extract handler names from openapi.rs paths(...) ──
documented=$(
  sed -n '/^[[:space:]]*paths(/,/^[[:space:]]*),/p' "$OPENAPI_RS" |
  grep 'crate::routes::' |
  sed -E 's/.*::([a-zA-Z_]+),?[[:space:]]*/\1/' |
  sort -u
)

# ── Step 3: Remove exemptions ──
registered_filtered="$registered"
for h in $EXEMPT; do
  registered_filtered=$(echo "$registered_filtered" | grep -v "^${h}$" || true)
done

# ── Step 4: Diff ──
missing=$(comm -23 <(echo "$registered_filtered") <(echo "$documented"))
orphaned=$(comm -13 <(echo "$registered_filtered") <(echo "$documented"))

# ── Step 5: Report ──
exit_code=0

if [ -n "$missing" ]; then
  count=$(echo "$missing" | wc -l | tr -d ' ')
  echo "FAIL: $count handler(s) registered in .route() but MISSING from openapi.rs:"
  echo "$missing" | while IFS= read -r h; do
    file=$(grep -rl "async fn $h\b" "$ROUTES_DIR" --include="*.rs" 2>/dev/null | head -1 || echo "?")
    file=${file#"$ROOT/"}
    echo "  - $h  ($file)"
  done
  echo ""
  echo "Fix: add #[utoipa::path(...)] to the handler and add it to openapi.rs paths(...)."
  exit_code=1
fi

if [ -n "$orphaned" ]; then
  echo ""
  echo "WARN: handler(s) in openapi.rs but not found in any .route() call:"
  echo "$orphaned" | while IFS= read -r h; do echo "  - $h"; done
fi

reg_count=$(echo "$registered_filtered" | grep -c . || echo 0)
doc_count=$(echo "$documented" | grep -c . || echo 0)
echo ""
echo "OpenAPI drift: $doc_count documented / $reg_count registered."

exit $exit_code
