import { getContextLimit } from '../lib/model-context-windows'

/** Authoritative context data from Live Monitor SSE (statusline source). */
export interface LiveContextData {
  contextWindowTokens: number
  statuslineContextWindowSize: number | null
  statuslineUsedPct: number | null
}

/**
 * Derive context window usage percentage using a 3-tier priority chain:
 *  1. statuslineUsedPct: pre-computed by Claude Code — correct numerator AND denominator
 *  2. Live Monitor contextWindowTokens + statuslineContextWindowSize: correct per-turn fill
 *  3. richData contextWindowTokens: JSONL accumulator (history), correct semantics
 *  4. undefined: show "--" — refuse to guess
 *
 * NOTE: WS totalInputTokens is intentionally NOT used — it sums across all models
 * in modelUsage and may be session-cumulative, producing inflated percentages (84% vs 12%).
 */
export function useContextPercent(
  liveContextData: LiveContextData | undefined,
  richDataContextTokens: number | undefined,
): number | undefined {
  if (liveContextData?.statuslineUsedPct != null) {
    return Math.round(liveContextData.statuslineUsedPct)
  }
  if (liveContextData && liveContextData.contextWindowTokens > 0) {
    const limit = getContextLimit(
      null,
      liveContextData.contextWindowTokens,
      liveContextData.statuslineContextWindowSize,
    )
    return Math.round((liveContextData.contextWindowTokens / limit) * 100)
  }
  if (richDataContextTokens != null && richDataContextTokens > 0) {
    const limit = getContextLimit(null, richDataContextTokens)
    return Math.round((richDataContextTokens / limit) * 100)
  }
  return undefined
}
