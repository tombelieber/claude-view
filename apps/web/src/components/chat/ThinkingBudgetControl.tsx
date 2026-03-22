import { Fragment } from 'react'
import { cn } from '../../lib/utils'

const EFFORT_LEVELS = [
  { label: 'Low', tokens: 1024 },
  { label: 'Medium', tokens: 4096 },
  { label: 'High', tokens: 16384 },
  { label: 'Max', tokens: 0 },
] as const

interface ThinkingBudgetControlProps {
  value: number | null
  onChange: (tokens: number | null) => void
  disabled?: boolean
}

function resolveIndex(value: number | null): number {
  if (value === null || value === 0) return EFFORT_LEVELS.length - 1
  const idx = EFFORT_LEVELS.findIndex((l) => l.tokens === value)
  return idx >= 0 ? idx : EFFORT_LEVELS.length - 1
}

export function ThinkingBudgetControl({ value, onChange, disabled }: ThinkingBudgetControlProps) {
  const idx = resolveIndex(value)

  return (
    <div className="flex items-center gap-2" title="Thinking budget">
      <span className="text-xs text-gray-400 dark:text-gray-500 whitespace-nowrap">
        Effort{' '}
        <span className="text-gray-600 dark:text-gray-300">({EFFORT_LEVELS[idx].label})</span>
      </span>
      <div className="flex items-center">
        {EFFORT_LEVELS.map((level, i) => (
          <Fragment key={level.label}>
            {i > 0 && (
              <div
                className={cn(
                  'w-2.5 h-0.5',
                  i <= idx ? 'bg-blue-500' : 'bg-gray-300 dark:bg-gray-600',
                )}
              />
            )}
            <button
              type="button"
              onClick={() => onChange(level.tokens)}
              disabled={disabled}
              className={cn(
                'rounded-full transition-all cursor-pointer disabled:cursor-not-allowed disabled:opacity-50',
                i === idx
                  ? 'w-3 h-3 bg-gray-700 dark:bg-gray-300'
                  : i < idx
                    ? 'w-1.5 h-1.5 bg-blue-500'
                    : 'w-1.5 h-1.5 bg-gray-300 dark:bg-gray-600',
              )}
              title={level.label}
            />
          </Fragment>
        ))}
      </div>
    </div>
  )
}
