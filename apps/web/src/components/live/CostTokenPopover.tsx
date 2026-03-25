import * as HoverCard from '@radix-ui/react-hover-card'
import { formatCostUsd, formatTokenCount } from '../../lib/format-utils'
import { COST_CATEGORY_COLORS } from '../../theme'
import type { LiveSummary } from './use-live-sessions'

interface CostTokenPopoverProps {
  summary: LiveSummary
}

export function CostTokenPopover({ summary }: CostTokenPopoverProps) {
  return (
    <HoverCard.Root openDelay={200} closeDelay={100}>
      <HoverCard.Trigger asChild>
        <span className="hidden md:flex items-center gap-2 text-gray-500 dark:text-gray-400 font-mono tabular-nums cursor-default">
          <span>{formatCostUsd(summary.totalCostTodayUsd)}</span>
          <span className="text-gray-300 dark:text-gray-600">&middot;</span>
          <span>{formatTokenCount(summary.totalTokensToday)}</span>
        </span>
      </HoverCard.Trigger>

      <HoverCard.Portal>
        <HoverCard.Content
          side="bottom"
          align="end"
          sideOffset={8}
          className="z-50 w-72 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-xl p-4 text-xs animate-in fade-in-0 zoom-in-95"
        >
          {/* Cost breakdown */}
          <div className="mb-3">
            <h4 className="text-xs font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500 mb-2">
              Cost
            </h4>
            <div className="space-y-1">
              <Row
                label="Input"
                value={formatCostUsd(summary.inputCostUsd)}
                dot={COST_CATEGORY_COLORS.input.dot}
              />
              <Row
                label="Output"
                value={formatCostUsd(summary.outputCostUsd)}
                dot={COST_CATEGORY_COLORS.output.dot}
              />
              <Row
                label="Cache read"
                value={formatCostUsd(summary.cacheReadCostUsd)}
                dot={COST_CATEGORY_COLORS.cacheRead.dot}
              />
              <Row
                label="Cache creation"
                value={formatCostUsd(summary.cacheCreationCostUsd)}
                dot={COST_CATEGORY_COLORS.cacheWrite.dot}
              />
              {summary.cacheSavingsUsd > 0 && (
                <Row
                  label="Cache savings"
                  value={`-${formatCostUsd(summary.cacheSavingsUsd)}`}
                  className={COST_CATEGORY_COLORS.savings.text}
                />
              )}
              <div className="border-t border-gray-200 dark:border-gray-700 pt-1 mt-1">
                <Row
                  label="Total"
                  value={formatCostUsd(summary.totalCostTodayUsd)}
                  className="font-semibold text-gray-900 dark:text-gray-100"
                />
              </div>
            </div>
          </div>

          {/* Token breakdown */}
          <div>
            <h4 className="text-xs font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500 mb-2">
              Tokens
            </h4>
            <div className="space-y-1">
              <Row
                label="Input"
                value={formatTokenCount(summary.inputTokens)}
                dot={COST_CATEGORY_COLORS.input.dot}
              />
              <Row
                label="Output"
                value={formatTokenCount(summary.outputTokens)}
                dot={COST_CATEGORY_COLORS.output.dot}
              />
              <Row
                label="Cache read"
                value={formatTokenCount(summary.cacheReadTokens)}
                dot={COST_CATEGORY_COLORS.cacheRead.dot}
              />
              <Row
                label="Cache creation"
                value={formatTokenCount(summary.cacheCreationTokens)}
                dot={COST_CATEGORY_COLORS.cacheWrite.dot}
              />
              <div className="border-t border-gray-200 dark:border-gray-700 pt-1 mt-1">
                <Row
                  label="Total"
                  value={formatTokenCount(summary.totalTokensToday)}
                  className="font-semibold text-gray-900 dark:text-gray-100"
                />
              </div>
            </div>
          </div>

          <HoverCard.Arrow className="fill-white dark:fill-gray-900" />
        </HoverCard.Content>
      </HoverCard.Portal>
    </HoverCard.Root>
  )
}

function Row({
  label,
  value,
  className = '',
  dot,
}: {
  label: string
  value: string
  className?: string
  dot?: string
}) {
  return (
    <div className="flex items-center justify-between">
      <span className="flex items-center gap-1.5 text-gray-500 dark:text-gray-400">
        {dot && <span className={`inline-block h-2 w-2 rounded-full shrink-0 ${dot}`} />}
        {label}
      </span>
      <span className={`font-mono tabular-nums ${className || 'text-gray-700 dark:text-gray-300'}`}>
        {value}
      </span>
    </div>
  )
}
