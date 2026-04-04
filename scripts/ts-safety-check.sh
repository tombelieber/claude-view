#!/usr/bin/env bash
# ts-safety-check.sh — catch TypeScript patterns that crash at runtime
#
# Runs on staged .ts/.tsx files during pre-commit.
# Exit 1 = block commit, exit 0 = pass.
#
# Pattern caught:
#   Object.values/keys/entries(x.y) without ?? fallback
#   → crashes: "Cannot convert undefined or null to object"
#
# This is a focused check — only catches the pattern that actually causes
# silent runtime crashes. TypeScript's type system catches most other issues.

set -euo pipefail

FILES="$*"
[ -z "$FILES" ] && exit 0

ERRORS=0

for file in $FILES; do
  [ -f "$file" ] || continue

  # Object.values/keys/entries(expr.prop) without ?? fallback
  # Matches: Object.values(data.ops)  Object.keys(resp.items)
  # Skips:   Object.values(data.ops ?? {})  Object.values(localVar)
  #          Object.entries(config.env) preceded by ternary guard
  while IFS= read -r match; do
    [ -z "$match" ] && continue
    # Skip if the line has a ternary guard: `x ? Object.entries(x.y) : []`
    if echo "$match" | grep -qE '\?\s*Object\.(values|keys|entries)'; then
      continue
    fi
    echo "  ⚠  $file: Object.values/keys/entries on nested access without ?? fallback"
    echo "     $match"
    echo "     Fix: add ?? {} or ?? [] fallback, e.g. Object.values(data.ops ?? {})"
    echo
    ERRORS=$((ERRORS + 1))
  done < <(grep -nE 'Object\.(values|keys|entries)\([a-zA-Z_]+\.[a-zA-Z_]+\)' "$file" 2>/dev/null | grep -v '??' || true)

done

if [ "$ERRORS" -gt 0 ]; then
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo "ts-safety-check: $ERRORS unsafe Object.values/keys/entries call(s)."
  echo "Crashes with 'Cannot convert undefined or null to object'."
  echo "Add ?? {} fallback: Object.values(data.ops ?? {})"
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  exit 1
fi
