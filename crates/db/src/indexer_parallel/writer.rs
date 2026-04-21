// crates/db/src/indexer_parallel/writer.rs
//
// CQRS Phase 6.4: the previous token-reconciliation check compared
// `sessions.total_input/output_tokens` against `SUM(turns.*)`. Migration
// 87 drops the `turns` table, and per-model token aggregates now live on
// `session_stats.per_model_tokens_json` (written by indexer_v2). The old
// reconciliation point has no source-of-truth to diff against in the
// parallel writer anymore — retire it rather than paper over with a
// stub.

use crate::Database;

/// No-op retained temporarily so callers keep compiling. Will be removed
/// with the rest of indexer_parallel in E.5.
pub(crate) async fn check_token_reconciliation(_db: &Database, _session_ids: &[String]) {}
