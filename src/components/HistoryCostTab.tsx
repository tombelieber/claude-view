import type { RichSessionData } from '../types/generated/RichSessionData'

interface HistoryCostTabProps {
  richData: RichSessionData
}

export function HistoryCostTab({ richData }: HistoryCostTabProps) {
  const { cost, tokens } = richData

  return (
    <div className="space-y-4">
      {/* Total */}
      <div className="text-center py-3">
        <div className="text-2xl font-mono font-semibold text-gray-900 dark:text-gray-100">
          ${cost.totalUsd.toFixed(4)}
        </div>
        <div className="text-xs text-gray-500 dark:text-gray-400 mt-1">
          {cost.isEstimated ? 'Estimated total cost' : 'Total cost'}
        </div>
      </div>

      {/* Breakdown */}
      <div className="space-y-2 text-xs">
        <CostRow label="Input" amount={cost.inputCostUsd} tokens={tokens.inputTokens} />
        <CostRow label="Output" amount={cost.outputCostUsd} tokens={tokens.outputTokens} />
        <CostRow label="Cache Read" amount={cost.cacheReadCostUsd} tokens={tokens.cacheReadTokens} />
        <CostRow label="Cache Write" amount={cost.cacheCreationCostUsd} tokens={tokens.cacheCreationTokens} />
        {cost.cacheSavingsUsd > 0 && (
          <div className="flex items-center justify-between pt-2 border-t border-gray-200 dark:border-gray-700">
            <span className="text-green-500">Cache Savings</span>
            <span className="font-mono text-green-500">-${cost.cacheSavingsUsd.toFixed(4)}</span>
          </div>
        )}
      </div>
    </div>
  )
}

function CostRow({ label, amount, tokens }: { label: string; amount: number; tokens: number }) {
  if (tokens === 0 && amount === 0) return null
  return (
    <div className="flex items-center justify-between">
      <span className="text-gray-500 dark:text-gray-400">{label}</span>
      <div className="flex items-center gap-2">
        <span className="text-gray-400 dark:text-gray-500">{(tokens / 1000).toFixed(1)}k</span>
        <span className="font-mono text-gray-700 dark:text-gray-300 w-16 text-right">${amount.toFixed(4)}</span>
      </div>
    </div>
  )
}
