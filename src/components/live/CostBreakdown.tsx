import type { LiveSession } from './use-live-sessions'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

interface CostBreakdownProps {
  cost: LiveSession['cost']
  subAgents?: SubAgentInfo[]
}

export function CostBreakdown({ cost, subAgents }: CostBreakdownProps) {
  const subAgentTotal = subAgents?.reduce((sum, a) => sum + (a.costUsd ?? 0), 0) ?? 0
  // cost.totalUsd is the PARENT session's cost only (sub-agent tokens are
  // separate API calls, not in the parent's cumulative token accumulation).
  // True total = parent + all sub-agents.
  const grandTotal = cost.totalUsd + subAgentTotal

  return (
    <div className="space-y-4 p-4">
      {/* Total */}
      <div className="flex items-baseline justify-between">
        <span className="text-sm text-gray-500 dark:text-gray-400">Total Cost</span>
        <span className="text-2xl font-mono font-semibold text-gray-900 dark:text-gray-100">
          ${grandTotal.toFixed(2)}
        </span>
      </div>

      {/* Breakdown table */}
      <div className="space-y-2">
        <CostRow label="Input tokens" value={cost.inputCostUsd} />
        <CostRow label="Output tokens" value={cost.outputCostUsd} />
        {cost.cacheReadCostUsd > 0 && <CostRow label="Cache reads" value={cost.cacheReadCostUsd} />}
        {cost.cacheCreationCostUsd > 0 && <CostRow label="Cache creation" value={cost.cacheCreationCostUsd} />}
        {cost.cacheSavingsUsd > 0 && (
          <CostRow label="Cache savings" value={-cost.cacheSavingsUsd} className="text-green-600 dark:text-green-400" />
        )}
      </div>

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

function CostRow({ label, value, className }: { label: string; value: number; className?: string }) {
  return (
    <div className="flex items-center justify-between text-sm">
      <span className="text-gray-500 truncate mr-4">{label}</span>
      <span className={`font-mono tabular-nums ${className ?? 'text-gray-700 dark:text-gray-300'}`}>
        ${Math.abs(value).toFixed(2)}
      </span>
    </div>
  )
}
