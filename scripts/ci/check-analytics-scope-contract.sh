#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

cargo test -p claude-view-server test_dashboard_stats_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_ai_generation_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_get_contributions_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_get_session_contribution_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_get_branch_sessions_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_insights_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_categories_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_insights_trends_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_benchmarks_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_trends_includes_data_scope_meta -- --nocapture
cargo test -p claude-view-server test_dashboard_stats_includes_session_breakdown_meta -- --nocapture
cargo test -p claude-view-server test_contributions_includes_session_breakdown_meta -- --nocapture
cargo test -p claude-view-server test_insights_includes_session_breakdown_meta -- --nocapture

echo "check-analytics-scope-contract: OK"
