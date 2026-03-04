import { Minimize2 } from 'lucide-react'
import { type ReactNode, useCallback, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { formatCostUsd, formatTokenCount } from '../../lib/format-utils'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
import { hasUnavailableCost, pricedCoveragePercent, unpricedTokenTotal } from './cost-display'
import type { LiveSession } from './use-live-sessions'

interface CostTooltipProps {
  cost: LiveSession['cost']
  cacheStatus: 'warm' | 'cold' | 'unknown'
  tokens?: LiveSession['tokens']
  subAgents?: SubAgentInfo[]
  compactCount?: number
  children: ReactNode
}

const TOOLTIP_W = 224 // w-56
const MARGIN = 8

export function CostTooltip({
  cost,
  cacheStatus,
  tokens,
  subAgents,
  compactCount,
  children,
}: CostTooltipProps) {
  const [isOpen, setIsOpen] = useState(false)
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const triggerRef = useRef<HTMLDivElement>(null)
  const tooltipRef = useRef<HTMLDivElement>(null)
  const [tooltipStyle, setTooltipStyle] = useState<React.CSSProperties>({})

  const computePlacement = useCallback(() => {
    const trigger = triggerRef.current
    const tooltip = tooltipRef.current
    if (!trigger || !tooltip) return
    const rect = trigger.getBoundingClientRect()
    const tipH = tooltip.offsetHeight
    const spaceBelow = window.innerHeight - rect.bottom
    // Don't let tooltip render behind the App header (h-12 = 48px + margin)
    const topBoundary = 56

    let top: number
    if (spaceBelow >= tipH + MARGIN) {
      top = rect.bottom + MARGIN
    } else if (rect.top - tipH - MARGIN >= topBoundary) {
      top = rect.top - tipH - MARGIN
    } else {
      // Not enough room above or below — prefer below, clamp to viewport
      top = rect.bottom + MARGIN
    }

    // Anchor right edge to trigger's right edge
    const left = Math.max(MARGIN, rect.right - TOOLTIP_W)

    setTooltipStyle({ position: 'fixed', top, left, zIndex: 9999 })
  }, [])

  useLayoutEffect(() => {
    if (isOpen) computePlacement()
  }, [isOpen, computePlacement])

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
  const sessionTotalUsd = cost.totalUsd + totalSubAgentCost
  const totalTokens = tokens?.totalTokens ?? 0
  const cacheCreation5mTokens = tokens?.cacheCreation5mTokens ?? 0
  const cacheCreation1hrTokens = tokens?.cacheCreation1hrTokens ?? 0
  const hasCacheCreationSplit = cacheCreation5mTokens > 0 || cacheCreation1hrTokens > 0
  const showUnavailableSessionTotal = hasUnavailableCost(sessionTotalUsd, cost, totalTokens)
  const showUnavailableMainAgent = hasUnavailableCost(cost.totalUsd, cost, totalTokens)
  const unpricedTokens = unpricedTokenTotal(cost)
  const pricedCoverage = pricedCoveragePercent(cost)

  return (
    <div
      ref={triggerRef}
      className="relative inline-block"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      {children}
      {isOpen &&
        createPortal(
          <div
            ref={tooltipRef}
            style={tooltipStyle}
            className="w-56 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg p-3 text-xs"
            onMouseEnter={handleMouseEnter}
            onMouseLeave={handleMouseLeave}
          >
            <div className="font-medium text-gray-900 dark:text-gray-100 mb-2">Cost Breakdown</div>
            <div className="space-y-1">
              <div className="font-medium text-gray-900 dark:text-gray-100 mb-1">
                Session Cost:{' '}
                {showUnavailableSessionTotal ? 'Unavailable' : formatCostUsd(sessionTotalUsd)}
              </div>
              <CostRow label="Input" cost={cost.inputCostUsd} />
              <CostRow label="Output" cost={cost.outputCostUsd} />
              <CostRow label="Cache read" cost={cost.cacheReadCostUsd} />
              <CostRow
                label={hasCacheCreationSplit ? 'Cache write (total)' : 'Cache write'}
                cost={cost.cacheCreationCostUsd}
              />
              {hasCacheCreationSplit && (
                <>
                  {cacheCreation5mTokens > 0 && (
                    <TokenRow label="  ↳ 5m tokens" tokens={cacheCreation5mTokens} />
                  )}
                  {cacheCreation1hrTokens > 0 && (
                    <TokenRow label="  ↳ 1h tokens" tokens={cacheCreation1hrTokens} />
                  )}
                </>
              )}
              <div className="border-t border-gray-200 dark:border-gray-700 pt-1 mt-1">
                <CostRow
                  label="Main agent total"
                  cost={cost.totalUsd}
                  valueText={showUnavailableMainAgent ? 'Unavailable' : undefined}
                  bold
                />
              </div>
              {hasSubAgentCosts && (
                <div className="pt-1 mt-1 border-t border-gray-200 dark:border-gray-700">
                  <div className="text-gray-500 dark:text-gray-400 mb-0.5">Sub-agents</div>
                  <div className="space-y-0.5 font-mono text-gray-500 dark:text-gray-400">
                    {subAgentsWithCost.map((sa, idx) => (
                      <AgentCostRow
                        key={sa.toolUseId}
                        label={sa.agentType}
                        cost={sa.costUsd ?? 0}
                        isLast={idx === subAgentsWithCost.length - 1}
                      />
                    ))}
                  </div>
                </div>
              )}
              {cost.cacheSavingsUsd > 0 && (
                <div className="text-green-600 dark:text-green-400 pt-1">
                  Saved {formatCostUsd(cost.cacheSavingsUsd)} via caching
                </div>
              )}
              <div className={`pt-1 ${cacheStatusColor}`}>Cache: {cacheStatusLabel}</div>
              {compactCount != null && compactCount > 0 && (
                <div
                  className={`pt-1 ${
                    compactCount >= 4
                      ? 'text-red-500'
                      : compactCount >= 2
                        ? 'text-amber-500 dark:text-amber-400'
                        : 'text-gray-500 dark:text-gray-400'
                  }`}
                >
                  <Minimize2 className="inline h-3 w-3 mr-0.5" />
                  {compactCount} {compactCount === 1 ? 'compaction' : 'compactions'} — context was
                  full {compactCount}×
                </div>
              )}
              {cost.hasUnpricedUsage && (
                <div className="text-amber-500 dark:text-amber-400 pt-1 text-[10px]">
                  Partial pricing: {formatTokenCount(unpricedTokens)} unpriced tokens excluded from
                  USD totals ({pricedCoverage}% priced coverage).
                </div>
              )}
            </div>
          </div>,
          document.body,
        )}
    </div>
  )
}

function CostRow({
  label,
  cost,
  bold,
  valueText,
}: {
  label: string
  cost: number
  bold?: boolean
  valueText?: string
}) {
  return (
    <div
      className={`flex items-center justify-between ${bold ? 'font-medium text-gray-900 dark:text-gray-100' : 'text-gray-500 dark:text-gray-400'}`}
    >
      <span>{label}</span>
      <span className="tabular-nums font-mono">{valueText ?? formatCostUsd(cost)}</span>
    </div>
  )
}

function AgentCostRow({ label, cost, isLast }: { label: string; cost: number; isLast: boolean }) {
  const treeChar = isLast ? '└──' : '├──'
  return (
    <div className="flex items-center justify-between">
      <span>
        {treeChar} {label}:
      </span>
      <span className="tabular-nums">{formatCostUsd(cost)}</span>
    </div>
  )
}

function TokenRow({ label, tokens }: { label: string; tokens: number }) {
  return (
    <div className="flex items-center justify-between text-gray-400 dark:text-gray-500">
      <span>{label}</span>
      <span className="tabular-nums font-mono">{tokens.toLocaleString()}</span>
    </div>
  )
}
