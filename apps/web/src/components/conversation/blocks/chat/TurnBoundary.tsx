import type { TurnBoundaryBlock } from '@claude-view/shared/types/blocks'

interface TurnBoundaryProps {
  block: TurnBoundaryBlock
}

function formatCost(usd: number): string {
  if (usd <= 0) return ''
  if (usd < 0.01) return `$${usd.toFixed(4)}`
  return `$${usd.toFixed(3)}`
}

export function ChatTurnBoundary({ block }: TurnBoundaryProps) {
  const costLabel = formatCost(block.totalCostUsd)

  return (
    <div className="flex items-center gap-3 py-1">
      <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
      {costLabel && (
        <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500 tabular-nums">
          {costLabel}
        </span>
      )}
      {!block.success && (
        <span className="text-[10px] font-medium text-red-500 dark:text-red-400 px-1.5 py-0.5 rounded bg-red-50 dark:bg-red-900/20">
          Error
        </span>
      )}
      <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
    </div>
  )
}
