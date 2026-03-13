interface SessionRollupBarProps {
  label: string
  value: number
  max: number
  suffix?: string
  formatValue?: (v: number) => string
}

function barColor(pct: number): string {
  if (pct >= 90) return 'bg-red-500'
  if (pct >= 70) return 'bg-amber-500'
  return 'bg-green-500'
}

function textColor(pct: number): string {
  if (pct >= 90) return 'text-red-600 dark:text-red-400'
  if (pct >= 70) return 'text-amber-600 dark:text-amber-400'
  return 'text-green-600 dark:text-green-400'
}

/** Compact inline progress bar with color thresholds for CPU/RAM rollup. */
export function SessionRollupBar({
  label,
  value,
  max,
  suffix,
  formatValue,
}: SessionRollupBarProps) {
  const pct = max > 0 ? (value / max) * 100 : 0
  const display = formatValue ? formatValue(value) : `${pct.toFixed(1)}%`

  return (
    <div className="flex items-center gap-2">
      <span className="text-xs font-medium text-gray-500 dark:text-gray-400 w-8 shrink-0">
        {label}
      </span>
      <div
        className="flex-1 h-1.5 rounded-full bg-gray-100 dark:bg-gray-800 overflow-hidden"
        role="progressbar"
        aria-valuenow={Math.round(pct)}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={label}
      >
        <div
          data-testid="rollup-fill"
          className={`h-full rounded-full transition-all duration-200 ${barColor(pct)}`}
          style={{ width: `${Math.min(pct, 100)}%` }}
        />
      </div>
      <span className={`text-xs tabular-nums font-medium shrink-0 ${textColor(pct)}`}>
        {display}
      </span>
      {suffix && (
        <span className="text-xs text-gray-400 dark:text-gray-500 shrink-0">{suffix}</span>
      )}
    </div>
  )
}
