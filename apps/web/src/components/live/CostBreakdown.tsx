import { formatCostUsd, formatTokenCount } from '../../lib/format-utils'
import { COST_CATEGORY_COLORS } from '../../theme'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
import { hasUnavailableCost, pricedCoveragePercent, unpricedTokenTotal } from './cost-display'
import type { LiveSession } from './use-live-sessions'

interface CostBreakdownProps {
  cost: LiveSession['cost']
  tokens?: LiveSession['tokens']
  subAgents?: SubAgentInfo[]
}

export function CostBreakdown({ cost, tokens, subAgents }: CostBreakdownProps) {
  const subAgentTotal = subAgents?.reduce((sum, a) => sum + (a.costUsd ?? 0), 0) ?? 0
  const grandTotal = cost.totalUsd + subAgentTotal
  const cacheCreation5mTokens = tokens?.cacheCreation5mTokens ?? 0
  const cacheCreation1hrTokens = tokens?.cacheCreation1hrTokens ?? 0
  const hasCacheCreationSplit = cacheCreation5mTokens > 0 || cacheCreation1hrTokens > 0
  const unpricedTokens = unpricedTokenTotal(cost)
  const pricedCoverage = pricedCoveragePercent(cost)

  // Effective rate: cost per 1M tokens (total cost / total tokens * 1M)
  const totalTokens = tokens?.totalTokens ?? 0
  const effectiveRate = totalTokens > 0 ? (grandTotal / totalTokens) * 1_000_000 : 0
  const showUnavailableTotal = hasUnavailableCost(grandTotal, cost, totalTokens)
  const showUnavailableMainAgent = hasUnavailableCost(cost.totalUsd, cost, totalTokens)

  // What it would cost without caching (add savings back)
  const uncachedTotal = grandTotal + (cost.cacheSavingsUsd ?? 0)
  const uncachedRate = totalTokens > 0 ? (uncachedTotal / totalTokens) * 1_000_000 : 0

  return (
    <div className="space-y-4 p-4">
      {/* Total */}
      <div className="flex items-baseline justify-between">
        <span className="text-sm text-gray-500 dark:text-gray-400">Total Cost</span>
        <span className="text-2xl font-mono font-semibold text-gray-900 dark:text-gray-100">
          {showUnavailableTotal ? 'Unavailable' : formatCostUsd(grandTotal)}
        </span>
      </div>
      {cost.hasUnpricedUsage && (
        <div className="rounded-md border border-amber-200 dark:border-amber-900/60 bg-amber-50 dark:bg-amber-950/20 p-2 text-xs text-amber-700 dark:text-amber-300">
          Partial pricing: {formatTokenCount(unpricedTokens)} unpriced tokens are excluded from USD
          totals ({pricedCoverage}% priced coverage).
        </div>
      )}

      {/* Breakdown table with tokens + cost columns */}
      <div className="space-y-1">
        {/* Column header */}
        <div className="flex items-center text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wide mb-1">
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
            dot={COST_CATEGORY_COLORS.cacheRead.dot}
            textClassName={COST_CATEGORY_COLORS.cacheRead.text}
          />
        )}
        {(cost.cacheCreationCostUsd > 0 || (tokens?.cacheCreationTokens ?? 0) > 0) && (
          <>
            <CostTokenRow
              label={hasCacheCreationSplit ? 'Cache write (total)' : 'Cache write'}
              tokens={tokens?.cacheCreationTokens}
              cost={cost.cacheCreationCostUsd}
            />
            {hasCacheCreationSplit && (
              <>
                {cacheCreation5mTokens > 0 && (
                  <CostTokenRow
                    label="  ↳ 5m cache write"
                    tokens={cacheCreation5mTokens}
                    tokenClassName="text-gray-400 dark:text-gray-500"
                  />
                )}
                {cacheCreation1hrTokens > 0 && (
                  <CostTokenRow
                    label="  ↳ 1h cache write"
                    tokens={cacheCreation1hrTokens}
                    tokenClassName="text-gray-400 dark:text-gray-500"
                  />
                )}
              </>
            )}
          </>
        )}

        {/* Total row */}
        <div className="flex items-center text-sm pt-1 border-t border-gray-200 dark:border-gray-800 font-medium text-gray-900 dark:text-gray-100">
          <span className="flex-1">
            {subAgents && subAgents.length > 0 ? 'Subtotal (main)' : 'Total'}
          </span>
          <span className="w-20 text-right font-mono tabular-nums">
            {tokens ? formatTokenCount(tokens.totalTokens) : '--'}
          </span>
          <span className="w-20 text-right font-mono tabular-nums">
            {showUnavailableMainAgent ? '—' : formatCostUsd(cost.totalUsd)}
          </span>
        </div>

        {cost.cacheSavingsUsd > 0 && (
          <div
            className={`flex items-center text-sm pt-1 border-t border-gray-100 dark:border-gray-800 ${COST_CATEGORY_COLORS.savings.text}`}
          >
            <span className="flex-1">Cache savings</span>
            <span className="w-20" />
            <span className="w-20 text-right font-mono tabular-nums">
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
              {showUnavailableTotal ? (
                'Unavailable'
              ) : (
                <>
                  {formatCostUsd(effectiveRate)}
                  <span className="text-gray-400 dark:text-gray-500 font-normal"> / 1M tokens</span>
                </>
              )}
            </span>
          </div>
          {cost.cacheSavingsUsd > 0 && uncachedRate > 0 && !showUnavailableTotal && (
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
          <h4 className="text-xs font-medium text-gray-500 uppercase tracking-wide">
            Cost by Agent
          </h4>
          <CostRow
            label="Main agent"
            value={cost.totalUsd}
            valueText={showUnavailableMainAgent ? 'Unavailable' : undefined}
          />
          {subAgents
            .filter((a) => a.costUsd != null && a.costUsd > 0)
            .map((a) => {
              const modelHint = a.model ? ` (${a.model})` : ''
              const hasTokens =
                a.inputTokens != null ||
                a.outputTokens != null ||
                a.cacheReadTokens != null ||
                a.cacheCreationTokens != null
              const totalTokens =
                (a.inputTokens ?? 0) +
                (a.outputTokens ?? 0) +
                (a.cacheReadTokens ?? 0) +
                (a.cacheCreationTokens ?? 0)
              return (
                <div key={a.toolUseId} className="space-y-0.5">
                  <CostRow
                    label={`${a.agentType}${modelHint}: ${a.description}`}
                    value={a.costUsd!}
                  />
                  {hasTokens && totalTokens > 0 && (
                    <div className="ml-4 text-xs text-gray-400 dark:text-gray-500 font-mono tabular-nums">
                      {[
                        a.inputTokens ? `${formatTokenCount(a.inputTokens)} in` : null,
                        a.outputTokens ? `${formatTokenCount(a.outputTokens)} out` : null,
                        a.cacheReadTokens ? `${formatTokenCount(a.cacheReadTokens)} cache` : null,
                      ]
                        .filter(Boolean)
                        .join(' · ')}
                    </div>
                  )}
                </div>
              )
            })}
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
  dot,
  textClassName,
}: {
  label: string
  tokens?: number
  cost?: number
  tokenClassName?: string
  dot?: string
  textClassName?: string
}) {
  return (
    <div className="flex items-center text-sm">
      <span className="flex-1 flex items-center gap-1.5 text-gray-500 dark:text-gray-400 truncate mr-2">
        {dot && <span className={`inline-block h-2 w-2 rounded-full shrink-0 ${dot}`} />}
        {label}
      </span>
      <span
        className={`w-20 text-right font-mono tabular-nums ${textClassName ?? tokenClassName ?? 'text-gray-500 dark:text-gray-400'}`}
      >
        {tokens != null ? formatTokenCount(tokens) : '--'}
      </span>
      <span className="w-20 text-right font-mono tabular-nums text-gray-700 dark:text-gray-300">
        {cost != null ? formatCostUsd(cost) : 'incl.'}
      </span>
    </div>
  )
}

function CostRow({
  label,
  value,
  className,
  valueText,
}: {
  label: string
  value: number
  className?: string
  valueText?: string
}) {
  return (
    <div className="flex items-center justify-between text-sm">
      <span className="text-gray-500 truncate mr-4">{label}</span>
      <span className={`font-mono tabular-nums ${className ?? 'text-gray-700 dark:text-gray-300'}`}>
        {valueText ?? formatCostUsd(Math.abs(value))}
      </span>
    </div>
  )
}
