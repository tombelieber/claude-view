interface SessionSummaryProps {
  sessionCost: number
  turnCount: number
  contextUsage: number
}

export function SessionSummary({ sessionCost, turnCount, contextUsage }: SessionSummaryProps) {
  return (
    <div className="rounded-lg bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 p-4 mx-4 my-4">
      <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100 mb-3">
        Session Summary
      </h3>
      <div className="grid grid-cols-3 gap-4">
        <div>
          <p className="text-xs text-gray-500 dark:text-gray-400">Total Cost</p>
          <p className="text-lg font-mono font-semibold text-gray-900 dark:text-gray-100">
            ${sessionCost.toFixed(4)}
          </p>
        </div>
        <div>
          <p className="text-xs text-gray-500 dark:text-gray-400">Turns</p>
          <p className="text-lg font-semibold text-gray-900 dark:text-gray-100">{turnCount}</p>
        </div>
        <div>
          <p className="text-xs text-gray-500 dark:text-gray-400">Context Used</p>
          <p className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            {Math.round(contextUsage)}%
          </p>
        </div>
      </div>
    </div>
  )
}
