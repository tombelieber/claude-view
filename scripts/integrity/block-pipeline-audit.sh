#!/usr/bin/env bash
set -euo pipefail

# ── Block Pipeline Audit ─────────────────────────────────────────────
# Scans JSONL corpus → Rust structs → TS types → React components → Storybook
# and produces a structured gap report.
#
# Usage:
#   bash scripts/integrity/block-pipeline-audit.sh --output artifacts/integrity/pipeline-audit.json
#   bash scripts/integrity/block-pipeline-audit.sh --output /dev/stdout  # stream to stdout
#
# Methodology: jq structural parsing only (never text grep on JSONL content)
# to avoid miscategorizing message content as structural fields.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${ROOT_DIR}"

# ── Defaults ──────────────────────────────────────────────────────────

CORPUS_DIR="${HOME}/.claude/projects"
OUTPUT=""
BASELINE="scripts/integrity/evidence-baseline.json"
MAX_FILES=1000

# ── Args ──────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
  case "$1" in
    --corpus-dir) CORPUS_DIR="$2"; shift 2 ;;
    --output)     OUTPUT="$2"; shift 2 ;;
    --baseline)   BASELINE="$2"; shift 2 ;;
    --max-files)  MAX_FILES="$2"; shift 2 ;;
    -h|--help)
      echo "Usage: $0 --output <path> [--corpus-dir <path>] [--baseline <path>] [--max-files <n>]"
      exit 0 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "$OUTPUT" ]]; then
  echo "Error: --output is required" >&2
  exit 1
fi

# ── Dependencies ──────────────────────────────────────────────────────

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || { echo "Missing required command: $1" >&2; exit 1; }
}
require_cmd jq
require_cmd rg

# ── Temp dir ──────────────────────────────────────────────────────────

TMPDIR_AUDIT="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_AUDIT"' EXIT

step() { echo "[audit] $1" >&2; }

# ══════════════════════════════════════════════════════════════════════
# STEP 1: JSONL Field Inventory
# ══════════════════════════════════════════════════════════════════════

step "Step 1/7: Scanning JSONL field paths..."

JSONL_FILES="$TMPDIR_AUDIT/jsonl-files.txt"
find "$CORPUS_DIR" -name "*.jsonl" -type f 2>/dev/null | head -n "$MAX_FILES" > "$JSONL_FILES" || true
JSONL_COUNT=$(wc -l < "$JSONL_FILES" | tr -d ' ')
step "  Found $JSONL_COUNT JSONL files (max $MAX_FILES)"

# Extract all unique dot-separated field paths using jq.
# Normalize array indices to [] for consistency with evidence-baseline convention.
JSONL_PATHS="$TMPDIR_AUDIT/jsonl-paths.txt"
JSONL_TYPES="$TMPDIR_AUDIT/jsonl-types.txt"
touch "$JSONL_PATHS" "$JSONL_TYPES"

if [[ "$JSONL_COUNT" -gt 0 ]]; then
  # Sample up to 200 files, 50 lines each for field path inventory
  SAMPLE_FILES="$TMPDIR_AUDIT/sample-files.txt"
  head -n 200 "$JSONL_FILES" > "$SAMPLE_FILES"

  while IFS= read -r f; do
    head -n 50 "$f" 2>/dev/null | jq -r '
      [path(..)] | .[] |
      map(if type == "number" then "[]" else tostring end) |
      join(".")
    ' 2>/dev/null >> "$TMPDIR_AUDIT/raw-paths.txt" || true
  done < "$SAMPLE_FILES"

  sort -u "$TMPDIR_AUDIT/raw-paths.txt" 2>/dev/null | sed 's/\.\[\]/[]/g' > "$JSONL_PATHS" || true

  # Extract top-level type values
  while IFS= read -r f; do
    head -n 100 "$f" 2>/dev/null | jq -r '.type // empty' 2>/dev/null || true
  done < "$SAMPLE_FILES" | sort -u > "$JSONL_TYPES"
fi

JSONL_PATH_COUNT=$(wc -l < "$JSONL_PATHS" | tr -d ' ')

# ══════════════════════════════════════════════════════════════════════
# STEP 2: Rust Struct Fields
# ══════════════════════════════════════════════════════════════════════

step "Step 2/7: Extracting Rust struct fields..."

BLOCK_TYPES="crates/core/src/block_types.rs"
RUST_FIELDS="$TMPDIR_AUDIT/rust-fields.json"

# Extract struct name → fields mapping
python3 -c "
import re, json, sys

with open('$BLOCK_TYPES') as f:
    content = f.read()

structs = {}
# Match pub struct Name { ... }
for m in re.finditer(r'pub struct (\w+)\s*\{([^}]+)\}', content, re.DOTALL):
    name = m.group(1)
    body = m.group(2)
    fields = re.findall(r'pub\s+(\w+):', body)
    structs[name] = fields

# Match enum variants
enums = {}
for m in re.finditer(r'pub enum (\w+)\s*\{([^}]+)\}', content, re.DOTALL):
    name = m.group(1)
    body = m.group(2)
    # Simple variants (no fields)
    variants = re.findall(r'^\s*(\w+)\s*[,{]', body, re.MULTILINE)
    # Also catch variants with fields like Text { text: String }
    variants += re.findall(r'^\s*(\w+)\s*\{', body, re.MULTILINE)
    enums[name] = sorted(set(variants))

json.dump({'structs': structs, 'enums': enums}, sys.stdout, indent=2)
" > "$RUST_FIELDS"

# ══════════════════════════════════════════════════════════════════════
# STEP 3: TypeScript Type Fields
# ══════════════════════════════════════════════════════════════════════

step "Step 3/7: Extracting TypeScript type fields..."

BLOCKS_TS="packages/shared/src/types/blocks.ts"
TS_FIELDS="$TMPDIR_AUDIT/ts-fields.json"

python3 -c "
import re, json, sys

with open('$BLOCKS_TS') as f:
    content = f.read()

types = {}
# Match export type Name = { ... }
for m in re.finditer(r'export type (\w+)\s*=\s*\{([^}]+)\}', content, re.DOTALL):
    name = m.group(1)
    body = m.group(2)
    fields = re.findall(r'^\s*(\w+)\??\s*:', body, re.MULTILINE)
    types[name] = fields

json.dump(types, sys.stdout, indent=2)
" > "$TS_FIELDS"

# ══════════════════════════════════════════════════════════════════════
# STEP 4: Component Field Usage
# ══════════════════════════════════════════════════════════════════════

step "Step 4/7: Scanning component field references..."

COMPONENT_DIR="packages/shared/src/components/conversation/blocks"
COMPONENT_USAGE="$TMPDIR_AUDIT/component-usage.json"

python3 -c "
import os, re, json, sys

usage = {}
base = '$COMPONENT_DIR'
for root, dirs, files in os.walk(base):
    for fname in files:
        if not fname.endswith('.tsx') or fname.endswith('.stories.tsx') or fname.endswith('.test.tsx'):
            continue
        fpath = os.path.join(root, fname)
        rel = os.path.relpath(fpath, '$ROOT_DIR')
        with open(fpath) as f:
            content = f.read()
        # Find block.fieldName and props.fieldName references
        refs = set(re.findall(r'block\.(\w+)', content))
        refs |= set(re.findall(r'data\.(\w+)', content))
        # Filter out common non-field names
        refs -= {'type', 'map', 'length', 'join', 'filter', 'find', 'some', 'every',
                 'toString', 'valueOf', 'reduce', 'forEach', 'includes', 'indexOf',
                 'slice', 'concat', 'push', 'pop', 'trim', 'split', 'replace',
                 'toLowerCase', 'toUpperCase', 'substring', 'startsWith', 'endsWith'}
        if refs:
            usage[rel] = sorted(refs)

json.dump(usage, sys.stdout, indent=2)
" > "$COMPONENT_USAGE"

# ══════════════════════════════════════════════════════════════════════
# STEP 5: Storybook Coverage
# ══════════════════════════════════════════════════════════════════════

step "Step 5/7: Checking Storybook coverage..."

STORYBOOK_COVERAGE="$TMPDIR_AUDIT/storybook-coverage.json"

python3 -c "
import os, re, json, sys

stories = {}
fixtures = {}

# Scan story files
for root, dirs, files in os.walk('packages/shared/src/components/conversation/blocks'):
    for fname in files:
        if not fname.endswith('.stories.tsx'):
            continue
        fpath = os.path.join(root, fname)
        rel = os.path.relpath(fpath, '$ROOT_DIR')
        with open(fpath) as f:
            content = f.read()
        exports = re.findall(r'export const (\w+):\s*Story', content)
        stories[rel] = exports

# Scan fixture files
for fpath in ['apps/web/src/stories/fixtures.ts', 'apps/web/src/stories/fixtures-developer.ts']:
    if not os.path.exists(fpath):
        continue
    with open(fpath) as f:
        content = f.read()
    # Find all fixture object keys (one level deep)
    blocks = re.findall(r'export const (\w+Blocks?)\s*=\s*\{', content)
    for block_name in blocks:
        # Find the keys within that object
        pattern = rf'{block_name}\s*=\s*\{{(.*?)\}}\s*(?://|$|\n\n)'
        # Simpler: find all "key: {" patterns after the export
        idx = content.index(block_name)
        rest = content[idx:]
        keys = re.findall(r'^\s+(\w+)\s*:', rest, re.MULTILINE)
        # Stop at next export
        next_export = re.search(r'\nexport ', rest[1:])
        if next_export:
            chunk = rest[:next_export.start() + 1]
            keys = re.findall(r'^\s+(\w+)\s*:', chunk, re.MULTILINE)
        fixtures[block_name] = keys[:50]  # cap

json.dump({'stories': stories, 'fixtures': fixtures}, sys.stdout, indent=2)
" > "$STORYBOOK_COVERAGE"

# ══════════════════════════════════════════════════════════════════════
# STEP 6: Dark Mode Gap Detection
# ══════════════════════════════════════════════════════════════════════

step "Step 6/7: Checking dark mode class pairs..."

DARK_MODE_GAPS="$TMPDIR_AUDIT/dark-mode-gaps.json"

# Find all color utility classes and check for dark: counterparts
python3 -c "
import os, re, json, sys

gaps = []
pattern = re.compile(r'\b(bg|border|text|ring|shadow|divide)-([a-z]+)-(\d+)(?:/\d+)?\b')
dark_pattern = re.compile(r'\bdark:(bg|border|text|ring|shadow|divide)-([a-z]+)-(\d+)(?:/\d+)?\b')

base = 'packages/shared/src/components/conversation/blocks'
for root, dirs, files in os.walk(base):
    for fname in files:
        if not fname.endswith('.tsx') or fname.endswith('.stories.tsx'):
            continue
        fpath = os.path.join(root, fname)
        rel = os.path.relpath(fpath, '$ROOT_DIR')
        with open(fpath) as f:
            content = f.read()

        # Collect light-mode classes per line
        for lineno, line in enumerate(content.split('\n'), 1):
            light_classes = set()
            dark_classes = set()
            for m in pattern.finditer(line):
                # Skip grays and neutral colors (often intentionally unpaired)
                if m.group(2) in ('gray', 'slate', 'zinc', 'neutral', 'stone', 'white', 'black'):
                    continue
                light_classes.add(f'{m.group(1)}-{m.group(2)}-{m.group(3)}')
            for m in dark_pattern.finditer(line):
                dark_classes.add(f'{m.group(1)}-{m.group(2)}-{m.group(3)}')

            # Check: any light semantic color without a dark counterpart on same line?
            for lc in light_classes:
                prop = lc.split('-')[0]  # bg, border, text, etc.
                has_dark = any(dc.startswith(prop + '-') for dc in dark_classes)
                if not has_dark:
                    gaps.append({
                        'file': rel,
                        'line': lineno,
                        'class': lc,
                        'issue': f'{lc} has no dark: counterpart on this line'
                    })

json.dump(gaps, sys.stdout, indent=2)
" > "$DARK_MODE_GAPS"

DARK_GAP_COUNT=$(jq 'length' "$DARK_MODE_GAPS")
step "  Found $DARK_GAP_COUNT dark mode gaps"

# ══════════════════════════════════════════════════════════════════════
# STEP 7: Cross-Layer Gap Analysis
# ══════════════════════════════════════════════════════════════════════

step "Step 7/7: Building cross-layer gap report..."

# Load baseline intentionally_ignored list
IGNORED=$(jq -r '.field_extraction_paths.intentionally_ignored[]' "$BASELINE" 2>/dev/null | sort -u)

# Build JSON arrays from text files safely
jq -R -s 'split("\n") | map(select(length > 0))' "$JSONL_TYPES" > "$TMPDIR_AUDIT/jsonl-types.json" 2>/dev/null || echo '[]' > "$TMPDIR_AUDIT/jsonl-types.json"

# Build final report using slurpfile to avoid shell quoting issues
jq -n \
  --slurpfile rust_fields "$RUST_FIELDS" \
  --slurpfile ts_fields "$TS_FIELDS" \
  --slurpfile component_usage "$COMPONENT_USAGE" \
  --slurpfile storybook "$STORYBOOK_COVERAGE" \
  --slurpfile dark_mode_gaps "$DARK_MODE_GAPS" \
  --slurpfile jsonl_types "$TMPDIR_AUDIT/jsonl-types.json" \
  --arg audit_date "$(date -u +%Y-%m-%d)" \
  --argjson corpus_files "${JSONL_COUNT:-0}" \
  --argjson field_path_count "${JSONL_PATH_COUNT:-0}" \
  '{
    audit_version: 2,
    audit_date: $audit_date,
    corpus_stats: {
      files_scanned: $corpus_files,
      unique_field_paths: $field_path_count
    },
    layers: {
      jsonl: { top_level_types: $jsonl_types[0] },
      rust: $rust_fields[0],
      typescript: $ts_fields[0],
      components: $component_usage[0],
      storybook: $storybook[0],
      dark_mode: { gaps: $dark_mode_gaps[0], gap_count: ($dark_mode_gaps[0] | length) }
    },
    cross_layer_gaps: {
      note: "Claude should analyze these layers and identify: (1) JSONL fields not in Rust structs, (2) Rust fields not in TS types, (3) TS fields not referenced in components, (4) block variants without Storybook stories"
    }
  }' > "$OUTPUT"

step "Audit complete → $OUTPUT"
step "  Corpus: $JSONL_COUNT files, $JSONL_PATH_COUNT unique paths"
step "  Dark mode gaps: $DARK_GAP_COUNT"
