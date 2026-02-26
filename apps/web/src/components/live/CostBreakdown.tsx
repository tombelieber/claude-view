import type { LiveSession } from './use-live-sessions'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
import { formatTokenCount, formatCostUsd } from '../../lib/format-utils'

interface CostBreakdownProps {
  cost: LiveSession['cost']
  tokens?: LiveSession['tokens']
  subAgents?: SubAgentInfo[]
}

export function CostBreakdown({ cost, tokens, subAgents }: CostBreakdownProps) {
  const subAgentTotal = subAgents?.reduce((sum, a) => sum + (a.costUsd ?? 0), 0) ?? 0
  const grandTotal = cost.totalUsd + subAgentTotal
  const estimatedPrefix = cost?.isEstimated ? '~' : ''

  // Effective rate: cost per 1M tokens (total cost / total tokens * 1M)
  const totalTokens = tokens?.totalTokens ?? 0
  const effectiveRate = totalTokens > 0 ? (grandTotal / totalTokens) * 1_000_000 : 0

  // What it would cost without caching (add savings back)
  const uncachedTotal = grandTotal + (cost.cacheSavingsUsd ?? 0)
  const uncachedRate = totalTokens > 0 ? (uncachedTotal / totalTokens) * 1_000_000 : 0

  return (
    <div className="space-y-4 p-4">
      {/* Total */}
      <div className="flex items-baseline justify-between">
        <span className="text-sm text-gray-500 dark:text-gray-400">Total Cost</span>
        <span className="text-2xl font-mono font-semibold text-gray-900 dark:text-gray-100">
          {estimatedPrefix}{formatCostUsd(grandTotal)}
        </span>
      </div>

      {/* Breakdown table with tokens + cost columns */}
      <div className="space-y-1">
        {/* Column header */}
        <div className="flex items-center text-[10px] font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wide mb-1">
          <span className="flex-1" />
          <span className="w-20 text-right">Tokens</span>
          <span className="w-20 text-right">Cost</span>
        </div>

        <CostTokenRow label="Input" tokens={tokens?.inputTokens} cost={cost.inputCostUsd} />
        <CostTokenRow label="Output" tokens={tokens?.outputTokens} cost={cost.outputCostUsd} />
        {(cost.cacheReadCostUsd > 0 || (tokens?.cacheReadTokens ?? 0) > 0) && (
          <CostTokenRow
            label="Cache read"
            tokens={tokens?.cacheReadTokens}
            cost={cost.cacheReadCostUsd}
            tokenClassName="text-green-600 dark:text-green-400"
          />
        )}
        {(cost.cacheCreationCostUsd > 0 || (tokens?.cacheCreationTokens ?? 0) > 0) && (
          <CostTokenRow label="Cache write" tokens={tokens?.cacheCreationTokens} cost={cost.cacheCreationCostUsd} />
        )}

        {cost.cacheSavingsUsd > 0 && (
          <div className="flex items-center text-sm pt-1 border-t border-gray-100 dark:border-gray-800">
            <span className="flex-1 text-green-600 dark:text-green-400">Cache savings</span>
            <span className="w-20" />
            <span className="w-20 text-right font-mono tabular-nums text-green-600 dark:text-green-400">
              -{formatCostUsd(cost.cacheSavingsUsd)}
            </span>
          </div>
        )}
      </div>

      {/* Effective rate */}
      {totalTokens > 0 && (
        <div className="rounded-md bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-800 p-3 space-y-1">
          <div className="flex items-center justify-between text-sm">
            <span className="text-gray-500 dark:text-gray-400">Effective rate</span>
            <span className="font-mono tabular-nums font-medium text-gray-900 dark:text-gray-100">
              {formatCostUsd(effectiveRate)}<span className="text-gray-400 dark:text-gray-500 font-normal"> / 1M tokens</span>
            </span>
          </div>
          {cost.cacheSavingsUsd > 0 && uncachedRate > 0 && (
            <div className="flex items-center justify-between text-xs">
              <span className="text-gray-400 dark:text-gray-500">Without caching</span>
              <span className="font-mono tabular-nums text-gray-400 dark:text-gray-500">
                {formatCostUsd(uncachedRate)} / 1M tokens
              </span>
            </div>
          )}
        </div>
      )}

      {/* Sub-agent breakdown */}
      {subAgents && subAgents.length > 0 && (
        <div className="border-t border-gray-200 dark:border-gray-800 pt-3 space-y-2">
          <h4 className="text-xs font-medium text-gray-500 uppercase tracking-wide">Cost by Agent</h4>
          <CostRow label="Main agent" value={cost.totalUsd} />
          {subAgents
            .filter((a) => a.costUsd != null && a.costUsd > 0)
            .map((a) => (
              <CostRow key={a.toolUseId} label={`${a.agentType}: ${a.description}`} value={a.costUsd!} />
            ))}
        </div>
      )}
    </div>
  )
}

function CostTokenRow({
  label,
  tokens,
  cost,
  tokenClassName,
}: {
  label: string
  tokens?: number
  cost: number
  tokenClassName?: string
}) {
  return (
    <div className="flex items-center text-sm">
      <span className="flex-1 text-gray-500 dark:text-gray-400 truncate mr-2">{label}</span>
      <span className={`w-20 text-right font-mono tabular-nums ${tokenClassName ?? 'text-gray-500 dark:text-gray-400'}`}>
        {tokens != null ? formatTokenCount(tokens) : '--'}
      </span>
      <span className="w-20 text-right font-mono tabular-nums text-gray-700 dark:text-gray-300">
        {formatCostUsd(cost)}
      </span>
    </div>
  )
}

function CostRow({ label, value, className }: { label: string; value: number; className?: string }) {
  return (
    <div className="flex items-center justify-between text-sm">
      <span className="text-gray-500 truncate mr-4">{label}</span>
      <span className={`font-mono tabular-nums ${className ?? 'text-gray-700 dark:text-gray-300'}`}>
        {formatCostUsd(Math.abs(value))}
      </span>
    </div>
  )
}

