import * as Tooltip from '@radix-ui/react-tooltip'

interface ChatContextGaugeProps {
  percent: number
  /** Current context fill in tokens. */
  tokens?: number
  /** Context window limit (200K or 1M). */
  limit?: number
  /** Data source — shown as subtle indicator. */
  source?: 'statusline' | 'sidecar' | 'history'
}

const formatTokens = (n: number) => {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}k`
  return String(n)
}

const formatLimit = (n: number) => formatTokens(n)

/** Auto-compaction fires around 80% — shown as threshold marker. */
const AUTOCOMPACT_PCT = 80

/**
 * Compact context-window gauge for the chat input bar.
 * Shows a segmented progress bar with token counts, limit denominator,
 * and color-coded zones (blue → amber → red).
 * Tooltip reveals full breakdown and data source.
 */
export function ChatContextGauge({ percent, tokens, limit, source }: ChatContextGaugeProps) {
  const clamped = Math.max(0, Math.min(100, percent))

  // Zone colors
  const isWarning = clamped >= 60 && clamped < AUTOCOMPACT_PCT
  const isDanger = clamped >= AUTOCOMPACT_PCT

  const barColor = isDanger
    ? 'bg-red-500 dark:bg-red-400'
    : isWarning
      ? 'bg-amber-500 dark:bg-amber-400'
      : 'bg-blue-500 dark:bg-blue-400'

  const textColor = isDanger
    ? 'text-red-600 dark:text-red-400'
    : isWarning
      ? 'text-amber-600 dark:text-amber-400'
      : 'text-gray-500 dark:text-gray-400'

  const glowColor = isDanger
    ? 'shadow-[0_0_6px_rgba(239,68,68,0.3)]'
    : isWarning
      ? 'shadow-[0_0_6px_rgba(245,158,11,0.2)]'
      : ''

  const hasTokenInfo = tokens != null && limit != null && limit > 0

  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <div
            role="meter"
            aria-valuenow={clamped}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={`Context window usage: ${Math.round(clamped)}%`}
            className={`flex items-center gap-1.5 cursor-default ${glowColor} rounded`}
          >
            {/* Progress bar with threshold marker */}
            <div className="relative w-20 h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
              {/* Fill */}
              <div
                className={`h-full rounded-full transition-all duration-500 ease-out ${barColor}`}
                style={{ width: `${clamped}%` }}
              />
              {/* Auto-compact threshold tick */}
              <div
                className="absolute top-0 h-full w-px bg-gray-400/50 dark:bg-gray-500/50"
                style={{ left: `${AUTOCOMPACT_PCT}%` }}
              />
            </div>

            {/* Token count / limit + percentage */}
            {hasTokenInfo ? (
              <span className={`text-xs font-medium tabular-nums leading-none ${textColor}`}>
                {formatTokens(tokens)}/{formatLimit(limit)}
              </span>
            ) : (
              <span className={`text-xs font-medium tabular-nums leading-none ${textColor}`}>
                {Math.round(clamped)}%
              </span>
            )}
          </div>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="top"
            sideOffset={6}
            className="z-50 rounded-lg px-3 py-2 text-xs bg-gray-900 dark:bg-gray-100 text-gray-100 dark:text-gray-900 shadow-lg animate-in fade-in-0 zoom-in-95"
          >
            <div className="space-y-1">
              <div className="font-medium">Context: {Math.round(clamped)}% used</div>
              {hasTokenInfo && (
                <div className="text-gray-400 dark:text-gray-600">
                  {tokens.toLocaleString()} / {limit.toLocaleString()} tokens
                </div>
              )}
              {hasTokenInfo && (
                <div className="text-gray-400 dark:text-gray-600">
                  ~{formatTokens(Math.max(0, limit - tokens))} remaining
                </div>
              )}
              {source && (
                <div className="text-gray-500 dark:text-gray-500 text-xs pt-0.5 border-t border-gray-700 dark:border-gray-400">
                  {source === 'statusline' && 'Live (statusline)'}
                  {source === 'sidecar' && 'Live (sidecar)'}
                  {source === 'history' && 'From session history'}
                </div>
              )}
            </div>
            <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
