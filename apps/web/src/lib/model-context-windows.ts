const DEFAULT_CONTEXT_WINDOW = 200_000
const CONTEXT_1M = 1_000_000

/**
 * Returns the context window limit for a model.
 *
 * Priority order:
 * 1. `statuslineSize` — authoritative value from Claude Code's statusline JSON
 *    (context_window.context_window_size). Always correct, knows 1M from turn 1.
 * 2. Infer from `currentFill` — if fill exceeds 200K the session must be 1M.
 * 3. Default 200K.
 */
export function getContextLimit(
  _model?: string | null,
  currentFill?: number,
  statuslineSize?: number | null,
): number {
  if (statuslineSize != null && statuslineSize > 0) return statuslineSize
  if (currentFill != null && currentFill > DEFAULT_CONTEXT_WINDOW) return CONTEXT_1M
  return DEFAULT_CONTEXT_WINDOW
}
