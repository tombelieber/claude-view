/**
 * Shared constants and utilities used by both codegen-from-openapi.ts and gen-skill-docs.ts.
 * Single source of truth — edit here, both scripts pick up changes.
 */

/** SSE operationIds — cannot be called via simple HTTP request/response */
export const SSE_OPERATION_IDS = new Set([
  'stream_classification',
  'facet_ingest_stream',
  'indexing_progress',
  'stream_jobs',
  'live_stream',
  'monitor_stream',
  'generate_report',
  'git_sync_progress',
  'chat_workflow',
])

/** Tags with hand-written tool files — no longer blocks generation (use SKIP_OPERATION_IDS instead) */
export const HAND_WRITTEN_TAGS = new Set<string>([])

/** operationIds that duplicate hand-written tools or should not be MCP tools */
export const SKIP_OPERATION_IDS = new Set([
  // Hand-written in sessions.ts (tag: sessions)
  'list_sessions',
  'get_session_detail',
  // Hand-written in sessions.ts (tag: search) — already skipped
  'search_handler',
  // Hand-written in stats.ts (tag: stats)
  'dashboard_stats',
  'stats_tokens',
  // Hand-written in stats.ts (tag: insights) — already skipped
  'get_fluency_score',
  // Hand-written in live.ts (tag: live)
  'list_live_sessions',
  'get_live_summary',
  // Large-payload — megabytes of JSONL, not suitable for MCP response
  'get_session_parsed',
  'get_session_rich',
  // Internal/sidecar control — not user-facing
  'handle_hook',
  'handle_statusline',
  'bind_control',
  'unbind_control',
  'kill_session',
  'get_session_statusline_debug',
])

/** Convert a string to snake_case */
export function toSnakeCase(s: string): string {
  return s
    .replace(/([a-z])([A-Z])/g, '$1_$2')
    .replace(/[^a-zA-Z0-9]/g, '_')
    .replace(/_+/g, '_')
    .replace(/^_|_$/g, '')
    .toLowerCase()
}

/** Build a tool name from tag + operationId, avoiding stutter (e.g. sessions_sessions_list) */
export function makeToolName(tag: string, operationId: string): string {
  const snakeOp = toSnakeCase(operationId)
  const snakeTag = toSnakeCase(tag)
  if (snakeOp.startsWith(`${snakeTag}_`)) return snakeOp
  return `${snakeTag}_${snakeOp}`
}
