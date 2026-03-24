import type { TurnBoundaryBlock } from '../../../../types/blocks'
import { AlertTriangle, ShieldOff } from 'lucide-react'

interface TurnBoundaryProps {
  block: TurnBoundaryBlock
}

function formatCost(usd: number): string {
  if (usd <= 0) return ''
  if (usd < 0.01) return `$${usd.toFixed(4)}`
  return `$${usd.toFixed(3)}`
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
  return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`
}

export function ChatTurnBoundary({ block }: TurnBoundaryProps) {
  const costLabel = formatCost(block.totalCostUsd)
  const durationLabel = block.durationMs > 0 ? formatDuration(block.durationMs) : ''
  const hasHookErrors = block.hookErrors && block.hookErrors.length > 0

  return (
    <div className="space-y-0">
      {/* Main divider line with cost + duration */}
      <div className="flex items-center gap-2 py-1">
        <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
        {durationLabel && (
          <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500 tabular-nums">
            {durationLabel}
          </span>
        )}
        {costLabel && (
          <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500 tabular-nums">
            {costLabel}
          </span>
        )}
        {!block.success && !hasHookErrors && !block.preventedContinuation && (
          <span className="text-[10px] font-medium text-red-500 dark:text-red-400 px-1.5 py-0.5 rounded bg-red-50 dark:bg-red-900/20">
            {block.error?.subtype === 'error_max_turns'
              ? `Max turns (${block.numTurns})`
              : block.error?.subtype === 'error_max_budget_usd'
                ? 'Budget exceeded'
                : 'Error'}
          </span>
        )}
        <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
      </div>

      {/* Hook error details — shown below the line when hooks failed */}
      {hasHookErrors && (
        <div className="flex items-start gap-1.5 px-4 py-1.5 mx-auto max-w-[90%] rounded bg-amber-50 dark:bg-gray-800 border border-amber-200 dark:border-amber-600/30">
          <AlertTriangle className="w-3 h-3 text-amber-600 dark:text-amber-400 flex-shrink-0 mt-0.5" />
          <div className="space-y-0.5">
            {block.hookErrors!.map((err, i) => (
              <p key={i} className="text-[11px] text-amber-700 dark:text-amber-300/90">
                {err}
              </p>
            ))}
          </div>
        </div>
      )}

      {/* Prevented continuation — shown when hooks blocked the agent */}
      {block.preventedContinuation && (
        <div className="flex items-center gap-1.5 px-4 py-1.5 mx-auto max-w-[90%] rounded bg-orange-50 dark:bg-gray-800 border border-orange-200 dark:border-orange-500/30">
          <ShieldOff className="w-3 h-3 text-orange-600 dark:text-orange-400 flex-shrink-0" />
          <span className="text-[11px] text-orange-700 dark:text-orange-300/90">
            Hook blocked continuation
          </span>
          {block.hookCount != null && (
            <span className="text-[10px] font-mono text-orange-600 dark:text-orange-400 ml-auto tabular-nums">
              {block.hookCount} hook{block.hookCount !== 1 ? 's' : ''} ran
            </span>
          )}
        </div>
      )}
    </div>
  )
}
