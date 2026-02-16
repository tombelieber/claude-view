import { useState, useRef, type ReactNode } from 'react'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

interface CostTooltipProps {
  cost: {
    totalUsd: number
    inputCostUsd: number
    outputCostUsd: number
    cacheReadCostUsd: number
    cacheCreationCostUsd: number
    cacheSavingsUsd: number
  }
  cacheStatus: 'warm' | 'cold' | 'unknown'
  subAgents?: SubAgentInfo[]
  children: ReactNode
}

export function CostTooltip({ cost, cacheStatus, subAgents, children }: CostTooltipProps) {
  const [isOpen, setIsOpen] = useState(false)
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>()

  const handleMouseEnter = () => {
    clearTimeout(timeoutRef.current)
    timeoutRef.current = setTimeout(() => setIsOpen(true), 200)
  }

  const handleMouseLeave = () => {
    clearTimeout(timeoutRef.current)
    timeoutRef.current = setTimeout(() => setIsOpen(false), 100)
  }

  const cacheStatusLabel = {
    warm: 'Warm',
    cold: 'Cold (expired)',
    unknown: 'Unknown',
  }[cacheStatus]

  const cacheStatusColor = {
    warm: 'text-green-500',
    cold: 'text-red-400',
    unknown: 'text-gray-400',
  }[cacheStatus]

  // Calculate sub-agent breakdown if applicable
  const subAgentsWithCost = subAgents?.filter((sa) => sa.costUsd != null && sa.costUsd > 0) ?? []
  const hasSubAgentCosts = subAgentsWithCost.length > 0
  const totalSubAgentCost = hasSubAgentCosts
    ? subAgentsWithCost.reduce((sum, sa) => sum + (sa.costUsd ?? 0), 0)
    : 0
  const mainAgentCost = hasSubAgentCosts ? cost.totalUsd - totalSubAgentCost : 0

  return (
    <div className="relative inline-block" onMouseEnter={handleMouseEnter} onMouseLeave={handleMouseLeave}>
      {children}
      {isOpen && (
        <div className="absolute z-50 right-0 top-full mt-1 w-56 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg p-3 text-xs">
          <div className="font-medium text-gray-900 dark:text-gray-100 mb-2">Cost Breakdown</div>
          <div className="space-y-1">
            {hasSubAgentCosts ? (
              <>
                <div className="font-medium text-gray-900 dark:text-gray-100 mb-1">
                  Session Cost: ${cost.totalUsd.toFixed(2)}
                </div>
                <div className="space-y-0.5 font-mono text-gray-500 dark:text-gray-400">
                  <AgentCostRow label="Main agent:" cost={mainAgentCost} isLast={false} />
                  {subAgentsWithCost.map((sa, idx) => (
                    <AgentCostRow
                      key={sa.toolUseId}
                      label={sa.agentType}
                      cost={sa.costUsd ?? 0}
                      isLast={idx === subAgentsWithCost.length - 1}
                    />
                  ))}
                </div>
                <div className="border-t border-gray-200 dark:border-gray-700 pt-1 mt-2" />
              </>
            ) : (
              <>
                <CostRow label="Input" cost={cost.inputCostUsd} />
                <CostRow label="Output" cost={cost.outputCostUsd} />
                <CostRow label="Cache read" cost={cost.cacheReadCostUsd} />
                <CostRow label="Cache write" cost={cost.cacheCreationCostUsd} />
                <div className="border-t border-gray-200 dark:border-gray-700 pt-1 mt-1">
                  <CostRow label="Total" cost={cost.totalUsd} bold />
                </div>
              </>
            )}
            {cost.cacheSavingsUsd > 0 && (
              <div className="text-green-600 dark:text-green-400 pt-1">
                Saved ${cost.cacheSavingsUsd.toFixed(2)} via caching
              </div>
            )}
            <div className={`pt-1 ${cacheStatusColor}`}>
              Cache: {cacheStatusLabel}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

function CostRow({ label, cost, bold }: { label: string; cost: number; bold?: boolean }) {
  return (
    <div className={`flex items-center justify-between ${bold ? 'font-medium text-gray-900 dark:text-gray-100' : 'text-gray-500 dark:text-gray-400'}`}>
      <span>{label}</span>
      <span className="tabular-nums font-mono">${cost.toFixed(4)}</span>
    </div>
  )
}

function AgentCostRow({ label, cost, isLast }: { label: string; cost: number; isLast: boolean }) {
  const treeChar = isLast ? '└──' : '├──'
  return (
    <div className="flex items-center justify-between">
      <span>
        {treeChar} {label}
      </span>
      <span className="tabular-nums">${cost.toFixed(2)}</span>
    </div>
  )
}
