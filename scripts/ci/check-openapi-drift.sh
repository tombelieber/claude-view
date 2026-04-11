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
# Format: module::handler (matches the module::handler keys used in comparison).
EXEMPT_PATTERN="ws_terminal_handler|ws_session_handler|ws_subagent_terminal_handler|ws_proxy_handler|metrics_handler|generate_key|revoke_key"

# ── Step 1: Extract module::handler names from .route() calls ──
# Processes each file individually to preserve module context and avoid
# namespace collisions (e.g. sessions::list_sessions vs cli_sessions::list_sessions).
# Output format: top_module::handler (e.g. cli_sessions::kill_session)
registered=$(
  find "$ROUTES_DIR" -name "*.rs" -print | perl -e '
    use File::Basename;
    my $routes_dir = shift @ARGV;

    while (my $file = <STDIN>) {
      chomp $file;

      # Derive top-level module from path:
      #   routes/cli_sessions/handlers.rs → cli_sessions
      #   routes/health.rs → health
      my $rel = $file;
      $rel =~ s|^\Q$routes_dir\E/||;
      my @parts = split(m|/|, $rel);
      pop @parts;  # remove filename
      # Use first directory as the module (matches openapi.rs convention)
      my $module = $parts[0] // basename($file, ".rs");

      open(my $fh, "<", $file) or next;
      my $content = do { local $/; <$fh> };
      close($fh);

      # Strip line comments
      $content =~ s|//[^\n]*||g;

      # Find .route() blocks (lookahead includes } to catch the last route in a chain)
      while ($content =~ /\.route\((.*?)(?=\.route\(|\.with_state|\.layer\(|\.fallback|;\s|\)\s*\})/sg) {
        my $block = $1;
        while ($block =~ /(?:get|post|put|delete|patch)\(([a-zA-Z_:]+)\)/g) {
          my $h = $1;
          $h =~ s/.*:://;  # handlers::foo → foo
          print "${module}::${h}\n";
        }
      }
    }
  ' "$ROUTES_DIR" |
  sort -u
)

# ── Step 2: Extract module::handler names from openapi.rs paths(...) ──
# Format: first_module::handler_name
# e.g. crate::routes::interact::handlers::interact_handler → interact::interact_handler
documented=$(
  sed -n '/^[[:space:]]*paths(/,/^[[:space:]]*),/p' "$OPENAPI_RS" |
  grep 'crate::routes::' |
  sed -E 's/.*crate::routes:://; s/,?[[:space:]]*$//' |
  perl -ne '
    chomp;
    my @parts = split(/::/);
    my $handler = pop @parts;
    my $module = $parts[0] // "unknown";
    print "${module}::${handler}\n";
  ' |
  sort -u
)

# ── Step 3: Remove exemptions ──
# Filter any entry whose handler part (after ::) matches an exempt name.
registered_filtered=$(echo "$registered" | grep -Ev "::($EXEMPT_PATTERN)$" || true)

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
