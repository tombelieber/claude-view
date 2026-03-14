import { useTweenedValue } from '../../hooks/use-tweened-value'

interface GaugeCardProps {
  label: string
  value: number
  max: number
  unit?: string
  detail?: string
  formatValue?: (v: number) => string
  /** Override automatic bar color (bypasses red/amber/green threshold logic). */
  barColor?: string
  /** Override automatic value text color. */
  valueColor?: string
}

function defaultFormat(v: number): string {
  return v.toFixed(1)
}

function gaugeColor(pct: number): string {
  if (pct >= 90) return 'bg-red-500'
  if (pct >= 70) return 'bg-amber-500'
  return 'bg-green-500'
}

function gaugeTextColor(pct: number): string {
  if (pct >= 90) return 'text-red-600 dark:text-red-400'
  if (pct >= 70) return 'text-amber-600 dark:text-amber-400'
  return 'text-green-600 dark:text-green-400'
}

export function GaugeCard({
  label,
  value,
  max,
  unit = '%',
  detail,
  formatValue,
  barColor,
  valueColor,
}: GaugeCardProps) {
  const pct = max > 0 ? (value / max) * 100 : 0
  const tweenedPct = useTweenedValue(pct)
  const displayValue = (formatValue ?? defaultFormat)(value)

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-4 transition-shadow hover:shadow-md">
      <div className="flex items-baseline justify-between">
        <p className="text-xs text-gray-500 dark:text-gray-400 uppercase tracking-wide font-medium">
          {label}
        </p>
        {detail && <p className="text-xs text-gray-400 dark:text-gray-500">{detail}</p>}
      </div>
      <p className={`text-2xl font-semibold mt-1 ${valueColor ?? gaugeTextColor(pct)}`}>
        {displayValue}
        <span className="text-sm font-normal ml-0.5">{unit}</span>
      </p>
      <div className="mt-2 h-1.5 rounded-full bg-gray-100 dark:bg-gray-800 overflow-hidden">
        <div
          className={`h-full rounded-full transition-colors ${barColor ?? gaugeColor(pct)}`}
          style={{ width: `${Math.min(tweenedPct, 100)}%` }}
        />
      </div>
    </div>
  )
}
