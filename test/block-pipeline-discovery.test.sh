#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURE=$(mktemp -d)
trap 'rm -rf "$FIXTURE"' EXIT
mkdir -p "$FIXTURE/corpus/project/session/subagents" "$FIXTURE/corpus/project/session/workflows/run/cache"
printf '%s\n' '{"type":"assistant","message":{"content":[{"type":"text","text":"Synthetic"}]}}' > "$FIXTURE/corpus/project/session.jsonl"
printf '%s\n' '{"type":"user","message":{"content":[{"type":"text","text":"Synthetic"}]}}' > "$FIXTURE/corpus/project/session/subagents/agent.jsonl"
printf '%s\n' '{"type":"launched"}' > "$FIXTURE/corpus/project/session/workflows/run/cache/memo.jsonl"
bash "$ROOT/scripts/integrity/block-pipeline-audit.sh" --corpus-dir "$FIXTURE/corpus" --output "$FIXTURE/report.json" > "$FIXTURE/audit.log" 2>&1
jq -e '.corpus_stats.files_scanned == 2 and (.cross_layer_gaps.new_unhandled_types | length == 0)' "$FIXTURE/report.json" >/dev/null
printf '%s\n' '{"type":"future_unhandled_session_type"}' > "$FIXTURE/corpus/project/future.jsonl"
bash "$ROOT/scripts/integrity/block-pipeline-audit.sh" --corpus-dir "$FIXTURE/corpus" --output "$FIXTURE/report.json" > "$FIXTURE/audit.log" 2>&1
jq -e '.cross_layer_gaps.new_unhandled_types | index("future_unhandled_session_type") != null' "$FIXTURE/report.json" >/dev/null
echo 'PASS: runtime session/subagent scope; cache excluded; unknown session type still fails'
