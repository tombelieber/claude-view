interface ChatStatusBarProps {
  contextUsage: number // 0-100 percentage
  turnCount: number
  sessionCost: number | null
  lastTurnCost: number | null
  status: string
}

export function ChatStatusBar({
  contextUsage,
  turnCount,
  sessionCost,
  lastTurnCost,
  status,
}: ChatStatusBarProps) {
  const isActive =
    status === 'active' || status === 'waiting_input' || status === 'waiting_permission'
  const sessionCostLabel = sessionCost == null ? '--' : `$${sessionCost.toFixed(4)}`

  return (
    <div className="flex items-center gap-4 px-4 py-1.5 border-t border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 text-xs text-gray-500 dark:text-gray-400">
      {/* Context usage */}
      <div className="flex items-center gap-2 min-w-[140px]">
        <span className="whitespace-nowrap">Context</span>
        <div className="flex-1 h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
          <div
            className={`h-full rounded-full transition-all ${
              contextUsage > 80 ? 'bg-red-500' : contextUsage > 60 ? 'bg-amber-500' : 'bg-blue-500'
            }`}
            style={{ width: `${Math.min(contextUsage, 100)}%` }}
          />
        </div>
        <span className="tabular-nums font-mono">{Math.round(contextUsage)}%</span>
      </div>

      {/* Divider */}
      <span className="text-gray-300 dark:text-gray-600">|</span>

      {/* Turn count */}
      <span className="tabular-nums">
        {turnCount} turn{turnCount !== 1 ? 's' : ''}
      </span>

      {/* Divider */}
      <span className="text-gray-300 dark:text-gray-600">|</span>

      {/* Running cost */}
      <span className="tabular-nums font-mono">{sessionCostLabel}</span>

      {/* Last turn cost */}
      {lastTurnCost != null && lastTurnCost > 0 && isActive && (
        <>
          <span className="text-gray-300 dark:text-gray-600">|</span>
          <span className="tabular-nums font-mono text-gray-400 dark:text-gray-500">
            +${lastTurnCost.toFixed(4)}
          </span>
        </>
      )}
    </div>
  )
}
