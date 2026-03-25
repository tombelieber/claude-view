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

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return `${n}`
}

/** Extract short model label from model key (e.g. "claude-sonnet-4-5-20250514" → "sonnet-4.5") */
function shortModelName(key: string): string {
  // Match common patterns: claude-{family}-{major}-{minor}
  const m = key.match(/claude-(\w+)-(\d+)-(\d+)/)
  if (m) return `${m[1]}-${m[2]}.${m[3]}`
  // Fallback: strip "claude-" prefix and date suffix
  return key.replace(/^claude-/, '').replace(/-\d{8}$/, '')
}

/** Build the compact info segments for the divider line. */
function buildInfoSegments(block: TurnBoundaryBlock): string[] {
  const segments: string[] = []

  // Model name — pick the first (usually only) model from modelUsage
  const modelKeys = Object.keys(block.modelUsage ?? {})
  if (modelKeys.length > 0) {
    segments.push(shortModelName(modelKeys[0]))
  }

  // Token flow — aggregate input→output across all models
  const usage = Object.values(block.modelUsage ?? {})
  if (usage.length > 0) {
    const totalIn = usage.reduce((sum, u) => sum + u.inputTokens, 0)
    const totalOut = usage.reduce((sum, u) => sum + u.outputTokens, 0)
    if (totalIn > 0 || totalOut > 0) {
      segments.push(`${formatTokens(totalIn)}→${formatTokens(totalOut)}`)
    }
  }

  const costLabel = formatCost(block.totalCostUsd)
  if (costLabel) segments.push(costLabel)

  if (block.durationMs > 0) segments.push(formatDuration(block.durationMs))

  return segments
}

export function ChatTurnBoundary({ block }: TurnBoundaryProps) {
  const infoSegments = buildInfoSegments(block)
  const hasHookErrors = block.hookErrors && block.hookErrors.length > 0

  return (
    <div className="space-y-0">
      {/* Main divider line with model · tokens · cost · duration */}
      <div className="flex items-center gap-2 py-1">
        <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
        {infoSegments.length > 0 && (
          <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500 tabular-nums">
            {infoSegments.join(' · ')}
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
