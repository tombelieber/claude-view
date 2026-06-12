import { Bot } from 'lucide-react'
import { type ProviderUsage, useProvidersUsage } from '../../hooks/use-providers-usage'
import { formatCostUsd, formatTokenCount } from '../../lib/format-utils'
import { ProviderBadge } from '../shared/ProviderBadge'

interface ByAgentCardProps {
  /** Rolling window, in days, passed straight to /api/providers/usage. */
  days: number
}

/**
 * Cost cell with a hard trust gate (寧願唔顯示，都唔顯示錯嘅嘢):
 * - every usage session priced → exact total
 * - partial coverage → "≥ $X.XX" + tooltip, never implying a complete total
 * - nothing priced → em dash
 */
function CostCell({ provider }: { provider: ProviderUsage }) {
  const { costUsd, pricedSessions, usageSessions } = provider

  if (pricedSessions === 0 || costUsd === undefined) {
    return <span className="text-gray-300 dark:text-gray-600">&mdash;</span>
  }

  if (pricedSessions === usageSessions && usageSessions > 0) {
    return <span className="text-gray-700 dark:text-gray-300">{formatCostUsd(costUsd)}</span>
  }

  return (
    <span
      className="text-gray-500 dark:text-gray-400"
      title={`Cost known for ${pricedSessions} of ${usageSessions} sessions with usage`}
    >
      &ge;&nbsp;{formatCostUsd(costUsd)}
    </span>
  )
}

/**
 * "By agent" usage leaderboard — foreign agents only (Codex, OpenCode,
 * Gemini CLI, …). Rows arrive pre-sorted by token volume desc from
 * /api/providers/usage. Renders nothing until data exists: no skeleton,
 * no empty state — most machines have zero foreign-agent sessions.
 */
export function ByAgentCard({ days }: ByAgentCardProps) {
  const { data } = useProvidersUsage(days)
  const providers = data?.providers ?? []

  if (providers.length === 0) return null

  return (
    <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider flex items-center gap-1.5">
          <Bot className="w-4 h-4" />
          By Agent
        </h2>
        <span className="text-xs text-gray-400 dark:text-gray-500">Last {days} days</span>
      </div>

      {/* Column headers */}
      <div className="grid grid-cols-[1fr_4rem_5rem_5rem] gap-2 text-xs text-gray-400 dark:text-gray-500 pb-2 border-b border-gray-100 dark:border-gray-800">
        <span>Agent</span>
        <span className="text-right">Sessions</span>
        <span className="text-right">Tokens</span>
        <span className="text-right">Cost</span>
      </div>

      <div className="divide-y divide-gray-50 dark:divide-gray-800/50">
        {providers.map((provider) => (
          <div
            key={provider.id}
            className="grid grid-cols-[1fr_4rem_5rem_5rem] gap-2 items-center py-2.5 text-sm"
          >
            <div className="min-w-0">
              <ProviderBadge provider={provider.id} size="md" />
            </div>
            <span className="text-right tabular-nums text-gray-700 dark:text-gray-300">
              {provider.sessions.toLocaleString()}
            </span>
            <span
              className="text-right tabular-nums text-gray-700 dark:text-gray-300"
              title={`${provider.inputTokens.toLocaleString()} in + ${provider.outputTokens.toLocaleString()} out`}
            >
              {formatTokenCount(provider.inputTokens + provider.outputTokens)}
            </span>
            <span className="text-right tabular-nums">
              <CostCell provider={provider} />
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}
