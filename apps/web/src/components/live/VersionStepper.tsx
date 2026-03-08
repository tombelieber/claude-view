import { ChevronLeft, ChevronRight } from 'lucide-react'
import { cn } from '../../lib/utils'

interface VersionStepperProps {
  maxVersion: number
  fromVersion: number
  toVersion: number
  onSelect: (from: number, to: number) => void
}

export function VersionStepper({
  maxVersion,
  fromVersion,
  toVersion,
  onSelect,
}: VersionStepperProps) {
  if (maxVersion < 2) return null

  const pairs: [number, number][] = []
  for (let i = 1; i < maxVersion; i++) {
    pairs.push([i, i + 1])
  }

  const activeIdx = pairs.findIndex(([f, t]) => f === fromVersion && t === toVersion)
  const canPrev = activeIdx > 0
  const canNext = activeIdx < pairs.length - 1

  return (
    <div className="flex items-center gap-1 px-3 py-1.5 border-b border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900/50">
      <button
        type="button"
        onClick={() => canPrev && onSelect(pairs[activeIdx - 1][0], pairs[activeIdx - 1][1])}
        disabled={!canPrev}
        className={cn(
          'p-0.5 rounded transition-colors',
          canPrev
            ? 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 cursor-pointer'
            : 'text-gray-300 dark:text-gray-700 cursor-default',
        )}
        aria-label="Previous version"
      >
        <ChevronLeft className="w-3 h-3" />
      </button>

      {pairs.map(([f, t]) => {
        const isActive = f === fromVersion && t === toVersion
        return (
          <button
            type="button"
            key={`${f}-${t}`}
            onClick={() => onSelect(f, t)}
            className={cn(
              'text-[10px] font-mono px-2 py-0.5 rounded-full transition-colors cursor-pointer',
              isActive
                ? 'bg-indigo-500 text-white'
                : 'bg-gray-100 dark:bg-gray-800 text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700',
            )}
          >
            v{f}→v{t}
          </button>
        )
      })}

      <button
        type="button"
        onClick={() => canNext && onSelect(pairs[activeIdx + 1][0], pairs[activeIdx + 1][1])}
        disabled={!canNext}
        className={cn(
          'p-0.5 rounded transition-colors',
          canNext
            ? 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 cursor-pointer'
            : 'text-gray-300 dark:text-gray-700 cursor-default',
        )}
        aria-label="Next version"
      >
        <ChevronRight className="w-3 h-3" />
      </button>
    </div>
  )
}
