#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# evidence-audit.sh — Pre-release evidence audit for JSONL parsing
#
# Scans real Claude Code JSONL data and compares structural inventories
# against the evidence baseline. Catches drift between real data and
# what the parser/indexer expects.
#
# PERFORMANCE: Chunk-based parallel extraction using all CPU cores.
# File list is split into NCPU chunks, each processed by its own jq
# writing to its own temp file — no output interleaving.
# ~6000 files in ~30-60s on Apple Silicon (vs ~4min sequential).
#
# Usage:
#   ./scripts/integrity/evidence-audit.sh                    # scan ~/.claude
#   ./scripts/integrity/evidence-audit.sh /path/to/jsonl/dir # custom path
#   EVIDENCE_QUICK=1 ./scripts/integrity/evidence-audit.sh   # fast mode (types only)
#
# Exit codes:
#   0 = all checks passed
#   1 = drift detected (new types/fields the parser doesn't handle)
#   2 = missing prerequisites
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BASELINE="$SCRIPT_DIR/evidence-baseline.json"
DATA_DIR="${1:-$HOME/.claude/projects}"
ARTIFACTS_DIR="$REPO_ROOT/artifacts/integrity"
QUICK="${EVIDENCE_QUICK:-0}"

# CPU cores for parallel execution
if command -v sysctl >/dev/null 2>&1; then
    NCPU=$(sysctl -n hw.logicalcpu 2>/dev/null || echo 4)
elif [ -f /proc/cpuinfo ]; then
    NCPU=$(grep -c ^processor /proc/cpuinfo 2>/dev/null || echo 4)
else
    NCPU=4
fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# ─── Prerequisites ───────────────────────────────────────────────────

check_prereqs() {
    local missing=0
    for cmd in jq find split; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            echo -e "${RED}MISSING:${NC} $cmd is required" >&2
            missing=1
        fi
    done
    if [ ! -f "$BASELINE" ]; then
        echo -e "${RED}MISSING:${NC} evidence-baseline.json at $BASELINE" >&2
        missing=1
    fi
    if [ ! -d "$DATA_DIR" ]; then
        echo -e "${RED}MISSING:${NC} JSONL data directory at $DATA_DIR" >&2
        echo "  Set a custom path: $0 /path/to/jsonl/dir" >&2
        missing=1
    fi
    if [ "$missing" -eq 1 ]; then
        exit 2
    fi
}

# ─── Helpers ─────────────────────────────────────────────────────────

# Compare two sorted lists: report NEW and MISSING items
diff_lists() {
    local label="$1"
    local expected_file="$2"
    local actual_file="$3"
    local drift=0

    local new_items
    new_items=$(comm -13 "$expected_file" "$actual_file" || true)
    local missing_items
    missing_items=$(comm -23 "$expected_file" "$actual_file" || true)

    if [ -n "$new_items" ]; then
        echo -e "  ${YELLOW}NEW ${label}${NC} (parser may not handle):"
        echo "$new_items" | while read -r item; do
            echo -e "    ${YELLOW}+ $item${NC}"
        done
        drift=1
    fi

    if [ -n "$missing_items" ]; then
        echo -e "  ${BLUE}ABSENT ${label}${NC} (expected but not found — corpus may be too small):"
        echo "$missing_items" | while read -r item; do
            echo -e "    ${BLUE}- $item${NC}"
        done
    fi

    return $drift
}

# ─── Parallel Extraction ─────────────────────────────────────────────
#
# Splits file list into NCPU chunks, each processed by its own jq
# writing to its own temp file. No shared stdout = no interleaving.
#
# Why not xargs -P piped to stdout?
#   Multiple jq processes writing to the same pipe interleave at byte
#   boundaries, producing garbled chimera lines like "assistantT:progress".
#   POSIX only guarantees atomic writes <= PIPE_BUF (4096), but jq's
#   stdio buffering aggregates many short lines into larger writes.

parallel_extract() {
    local jq_filter="$1"
    local output_file="$2"
    local work_dir
    work_dir=$(mktemp -d)

    # Collect all JSONL files
    find "$DATA_DIR" -name '*.jsonl' -type f > "$work_dir/filelist"

    # Split into NCPU chunks (line-based split)
    split -n "l/$NCPU" "$work_dir/filelist" "$work_dir/chunk_"

    # Process each chunk in its own jq → own temp file
    local pids=()
    local i=0
    for chunk in "$work_dir"/chunk_*; do
        xargs jq -r "$jq_filter" < "$chunk" > "$work_dir/out_$i" 2>/dev/null &
        pids+=($!)
        i=$((i + 1))
    done

    # Wait for all workers
    for pid in "${pids[@]}"; do
        wait "$pid" || true
    done

    # Merge outputs (order doesn't matter — we sort downstream)
    cat "$work_dir"/out_* > "$output_file"

    # Cleanup
    rm -rf "$work_dir"
}

# ─── Single-Pass Extraction Filters ──────────────────────────────────
#
# ONE jq filter extracts ALL signals from every entry.
# Output is tagged lines: "TAG:value"
#
#   T:user              — top-level type
#   S:turn_duration     — system subtype
#   P:hook_progress     — progress data.type
#   A:text              — assistant content block type
#   TK:signature,thinking,type — thinking block keys
#   ND:direct           — agent_progress direct nesting hit
#   NN:nested           — agent_progress double-nested hit

# Quick mode: only extract top-level types (fastest possible)
JQ_QUICK='
  .type // "MISSING"
'

# Full mode: extract everything in one pass
JQ_FULL='
  "T:" + (.type // "MISSING"),

  # System subtypes
  (if .type == "system" then
    "S:" + (.subtype // "MISSING")
  else empty end),

  # Progress data.type
  (if .type == "progress" then
    "P:" + (.data.type // "MISSING")
  else empty end),

  # Assistant content block types
  (if .type == "assistant" and (.message.content | type) == "array" then
    (.message.content[]? | "A:" + (.type // "MISSING"))
  else empty end),

  # Thinking block key signatures
  (if .type == "assistant" and (.message.content | type) == "array" then
    (.message.content[]? | select(.type == "thinking") | "TK:" + (keys | join(",")))
  else empty end),

  # Agent progress nesting: direct path
  (if .type == "progress" and .data.type == "agent_progress" then
    (if (.data.message.content | type) == "array" then "ND:1" else empty end),
    (if (.data.message.message.content | type) == "array" then "NN:1" else empty end)
  else empty end)
'

# ─── Main ────────────────────────────────────────────────────────────

main() {
    check_prereqs
    mkdir -p "$ARTIFACTS_DIR"

    local file_count
    file_count=$(find "$DATA_DIR" -name '*.jsonl' -type f | wc -l | tr -d ' ')

    echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  EVIDENCE AUDIT — Pre-Release JSONL Schema Guard${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "  Data dir:  $DATA_DIR"
    echo -e "  Baseline:  $BASELINE"
    echo -e "  Files:     $file_count JSONL files"
    echo -e "  CPU cores: $NCPU (parallel)"
    echo -e "  Mode:      $([ "$QUICK" = "1" ] && echo "QUICK (types only)" || echo "FULL (chunk-parallel)")"
    echo ""

    if [ "$file_count" -lt 10 ]; then
        echo -e "${YELLOW}WARNING: Only $file_count files — results may be incomplete${NC}"
    fi

    local start_time
    start_time=$(date +%s)

    if [ "$QUICK" = "1" ]; then
        run_quick "$start_time"
    else
        run_full "$start_time"
    fi
}

run_quick() {
    local start_time="$1"
    echo -e "${BLUE}[1/1] Top-level type census (parallel, $NCPU cores)...${NC}"

    local raw_output
    raw_output=$(mktemp)
    parallel_extract "$JQ_QUICK" "$raw_output"

    local actual_types_file
    actual_types_file=$(mktemp)
    sort -u "$raw_output" > "$actual_types_file"

    local expected_types_file
    expected_types_file=$(mktemp)
    jq -r '
        (.top_level_types.handled // [])[] ,
        (.top_level_types.handled_as_progress // [])[] ,
        (.top_level_types.silently_ignored // [])[]
    ' "$BASELINE" | sort -u > "$expected_types_file"

    local drift=0
    if ! diff_lists "top-level types" "$expected_types_file" "$actual_types_file"; then
        drift=1
    else
        echo -e "  ${GREEN}OK${NC} — all types accounted for"
    fi

    rm -f "$raw_output" "$actual_types_file" "$expected_types_file"
    finish "$drift" "$start_time"
}

run_full() {
    local start_time="$1"

    # ─── SINGLE PASS: extract everything in parallel ──────────────────
    echo -e "${BLUE}[scan] Chunk-parallel extraction ($NCPU cores)...${NC}"

    local raw_output
    raw_output=$(mktemp)
    parallel_extract "$JQ_FULL" "$raw_output"

    local line_count
    line_count=$(wc -l < "$raw_output" | tr -d ' ')
    echo -e "  Extracted $line_count signal lines"
    echo ""

    # ─── Post-process: split tagged output into per-check data ────────
    local drift=0

    # Check 1: Top-level types
    echo -e "${BLUE}[1/6] Top-level type census...${NC}"
    local actual_types
    actual_types=$(mktemp)
    grep '^T:' "$raw_output" | cut -d: -f2- | sort -u > "$actual_types"

    local expected_types
    expected_types=$(mktemp)
    jq -r '
        (.top_level_types.handled // [])[] ,
        (.top_level_types.handled_as_progress // [])[] ,
        (.top_level_types.silently_ignored // [])[]
    ' "$BASELINE" | sort -u > "$expected_types"

    if ! diff_lists "top-level types" "$expected_types" "$actual_types"; then
        drift=1
    else
        echo -e "  ${GREEN}OK${NC} — all types accounted for"
    fi

    # Save census to artifacts
    grep '^T:' "$raw_output" | cut -d: -f2- | sort | uniq -c | sort -rn > "$ARTIFACTS_DIR/type-census.txt"

    rm -f "$actual_types" "$expected_types"

    # Check 2: Assistant content block types
    echo -e "${BLUE}[2/6] Assistant content block types...${NC}"
    local actual_cbt
    actual_cbt=$(mktemp)
    grep '^A:' "$raw_output" | cut -d: -f2- | sort -u > "$actual_cbt"

    local expected_cbt
    expected_cbt=$(mktemp)
    jq -r '.content_block_types.assistant[]' "$BASELINE" | sort -u > "$expected_cbt"

    if ! diff_lists "assistant content block types" "$expected_cbt" "$actual_cbt"; then
        drift=1
    else
        echo -e "  ${GREEN}OK${NC} — {text, thinking, tool_use}"
    fi
    rm -f "$actual_cbt" "$expected_cbt"

    # Check 3: System subtypes
    echo -e "${BLUE}[3/6] System subtypes...${NC}"
    local actual_sub
    actual_sub=$(mktemp)
    grep '^S:' "$raw_output" | cut -d: -f2- | sort -u > "$actual_sub"

    local expected_sub
    expected_sub=$(mktemp)
    jq -r '.system_subtypes.known[]' "$BASELINE" | sort -u > "$expected_sub"

    if ! diff_lists "system subtypes" "$expected_sub" "$actual_sub"; then
        drift=1
    else
        echo -e "  ${GREEN}OK${NC} — all subtypes accounted for"
    fi
    rm -f "$actual_sub" "$expected_sub"

    # Check 4: Progress data.type values
    echo -e "${BLUE}[4/6] Progress data.type values...${NC}"
    local actual_pdt
    actual_pdt=$(mktemp)
    grep '^P:' "$raw_output" | cut -d: -f2- | sort -u > "$actual_pdt"

    local expected_pdt
    expected_pdt=$(mktemp)
    jq -r '.progress_data_types.known[]' "$BASELINE" | sort -u > "$expected_pdt"

    if ! diff_lists "progress data.type values" "$expected_pdt" "$actual_pdt"; then
        drift=1
    else
        echo -e "  ${GREEN}OK${NC} — all data.types accounted for"
    fi
    rm -f "$actual_pdt" "$expected_pdt"

    # Check 5: Thinking block keys
    echo -e "${BLUE}[5/6] Thinking block keys (signature field)...${NC}"
    local thinking_keys
    thinking_keys=$(grep '^TK:' "$raw_output" | cut -d: -f2- | sort -u | tr '\n' ',' | sed 's/,$//')

    local expected_thinking
    expected_thinking=$(jq -r '.thinking_block_keys.required | sort | join(",")' "$BASELINE")

    if [ "$thinking_keys" = "$expected_thinking" ]; then
        echo -e "  ${GREEN}OK${NC} — keys: {$thinking_keys}"
    else
        echo -e "  ${YELLOW}DRIFT${NC} — expected {$expected_thinking}, got {$thinking_keys}"
        drift=1
    fi

    # Check 6: Agent progress nesting path
    echo -e "${BLUE}[6/6] Agent progress nesting path...${NC}"
    local direct_count
    direct_count=$(grep -c '^ND:' "$raw_output" || echo 0)
    local nested_count
    nested_count=$(grep -c '^NN:' "$raw_output" || echo 0)

    echo -e "  data.message.content (direct):        $direct_count"
    echo -e "  data.message.message.content (nested): $nested_count"

    if [ "$direct_count" -gt 0 ] && [ "$nested_count" -eq 0 ]; then
        echo -e "  ${YELLOW}DRIFT${NC} — agent_progress nesting has changed to direct path!"
        drift=1
    elif [ "$nested_count" -gt 0 ]; then
        echo -e "  ${GREEN}OK${NC} — double-nested path confirmed"
    else
        echo -e "  ${BLUE}INFO${NC} — no agent_progress entries found"
    fi

    rm -f "$raw_output"
    finish "$drift" "$start_time"
}

finish() {
    local drift="$1"
    local start_time="$2"
    local end_time
    end_time=$(date +%s)
    local elapsed=$(( end_time - start_time ))

    echo ""
    echo -e "${BLUE}───────────────────────────────────────────────────────────${NC}"

    local manifest="$ARTIFACTS_DIR/evidence-manifest.json"
    local ts
    ts=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    cat > "$manifest" <<MANIFEST_EOF
{
  "generated_at": "$ts",
  "data_dir": "$DATA_DIR",
  "baseline": "$BASELINE",
  "drift_detected": $([ "$drift" -eq 0 ] && echo "false" || echo "true"),
  "checks_run": $([ "$QUICK" = "1" ] && echo "1" || echo "6"),
  "cpu_cores": $NCPU,
  "elapsed_seconds": $elapsed
}
MANIFEST_EOF

    if [ "$drift" -eq 0 ]; then
        echo -e "  ${GREEN}ALL CHECKS PASSED${NC} — parser matches real data (${elapsed}s)"
        echo -e "  Manifest: $manifest"
        echo ""
        exit 0
    else
        echo -e "  ${RED}DRIFT DETECTED${NC} — update parser or baseline before release (${elapsed}s)"
        echo -e "  Manifest: $manifest"
        echo ""
        echo -e "  To update baseline after fixing parser:"
        echo -e "    1. Fix the parser to handle new types/fields"
        echo -e "    2. Update scripts/integrity/evidence-baseline.json"
        echo -e "    3. Update docs/architecture/message-types.md"
        echo -e "    4. Re-run this script to verify"
        echo ""
        exit 1
    fi
}

main "$@"
