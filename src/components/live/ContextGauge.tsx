const MODEL_CONTEXT_LIMITS: Record<string, number> = {
  'claude-opus-4': 200_000,
  'claude-sonnet-4': 200_000,
  'claude-haiku-4': 200_000,
  'claude-3': 200_000,
}

function getContextLimit(model: string | null): number {
  if (!model) return 200_000
  for (const [prefix, limit] of Object.entries(MODEL_CONTEXT_LIMITS)) {
    if (model.startsWith(prefix)) return limit
  }
  return 200_000
}

import type { AgentStateGroup } from './types'

interface ContextGaugeProps {
  /** Current context window fill (last turn's total input tokens). */
  contextWindowTokens: number
  model: string | null
  group: AgentStateGroup
}

export function ContextGauge({ contextWindowTokens, model, group }: ContextGaugeProps) {
  const contextLimit = getContextLimit(model)
  const usedPct = Math.min((contextWindowTokens / contextLimit) * 100, 100)

  const formatTokens = (n: number) => {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
    if (n >= 1_000) return `${(n / 1_000).toFixed(0)}k`
    return String(n)
  }

  // Needs-you sessions get muted grey gauge; autonomous stays colored
  const isInactive = group === 'needs_you'
  const barColor = isInactive
    ? 'bg-zinc-500 opacity-50'
    : usedPct > 90
      ? 'bg-red-500'
      : usedPct > 75
        ? 'bg-amber-500'
        : 'bg-sky-500'

  return (
    <div className="space-y-1">
      <div className="h-1.5 rounded-full bg-gray-200 dark:bg-gray-800 overflow-hidden">
        {usedPct > 0 && (
          <div
            className={`${barColor} h-full transition-all duration-300`}
            style={{ width: `${usedPct}%` }}
            title={`Context: ${formatTokens(contextWindowTokens)} / ${formatTokens(contextLimit)} tokens (${usedPct.toFixed(0)}%)`}
          />
        )}
      </div>
      <div className="flex items-center justify-between text-[10px] text-gray-400 dark:text-gray-500">
        <span>{formatTokens(contextWindowTokens)}/{formatTokens(contextLimit)} tokens</span>
        {usedPct > 75 && (
          <span className={usedPct > 90 ? 'text-red-500' : 'text-amber-500'} title="Context is filling up. Auto-compaction may occur soon.">
            {usedPct.toFixed(0)}% used
          </span>
        )}
      </div>
    </div>
  )
}
