import { useCallback } from 'react'
import { Grid3x3, Maximize2, Minimize2, RotateCcw } from 'lucide-react'
import { cn } from '../../lib/utils'

interface GridControlsProps {
  gridOverride: { cols: number; rows: number } | null
  compactHeaders: boolean
  sessionCount: number
  visibleCount: number
  onGridOverrideChange: (override: { cols: number; rows: number } | null) => void
  onCompactHeadersChange: (compact: boolean) => void
}

/**
 * GridControls — toolbar above the MonitorGrid.
 *
 * Controls:
 * - Cols x Rows sliders: override auto-responsive layout (1-4 each)
 * - Auto button: reset to auto-responsive mode
 * - Compact toggle: shrink pane headers
 * - Session count badge: "N of M sessions"
 */
export function GridControls({
  gridOverride,
  compactHeaders,
  sessionCount,
  visibleCount,
  onGridOverrideChange,
  onCompactHeadersChange,
}: GridControlsProps) {
  const isAutoMode = gridOverride === null

  const handleColsChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const cols = parseInt(e.target.value, 10)
      const rows = gridOverride?.rows ?? 2
      onGridOverrideChange({ cols, rows })
    },
    [gridOverride, onGridOverrideChange]
  )

  const handleRowsChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const rows = parseInt(e.target.value, 10)
      const cols = gridOverride?.cols ?? 2
      onGridOverrideChange({ cols, rows })
    },
    [gridOverride, onGridOverrideChange]
  )

  const handleAutoClick = useCallback(() => {
    onGridOverrideChange(null)
  }, [onGridOverrideChange])

  const handleCompactToggle = useCallback(() => {
    onCompactHeadersChange(!compactHeaders)
  }, [compactHeaders, onCompactHeadersChange])

  return (
    <div className="flex flex-wrap items-center gap-3 px-3 py-2 rounded-lg bg-gray-100/50 dark:bg-gray-800/50 border border-gray-200 dark:border-gray-800">
      {/* Grid layout icon */}
      <Grid3x3 className="h-3.5 w-3.5 text-gray-400 dark:text-gray-500 flex-shrink-0" />

      {/* Cols slider */}
      <div className="flex items-center gap-1.5">
        <label
          htmlFor="grid-cols"
          className="text-[10px] uppercase tracking-wider font-semibold text-gray-400 dark:text-gray-500"
        >
          Cols
        </label>
        <input
          id="grid-cols"
          type="range"
          min={1}
          max={4}
          step={1}
          value={gridOverride?.cols ?? 2}
          onChange={handleColsChange}
          disabled={isAutoMode}
          className={cn(
            'w-16 h-1 rounded-full appearance-none cursor-pointer',
            'accent-indigo-500',
            '[&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:bg-indigo-500',
            isAutoMode && 'opacity-40 cursor-not-allowed'
          )}
        />
        <span className="text-xs font-mono text-gray-500 dark:text-gray-400 tabular-nums w-3 text-center">
          {gridOverride?.cols ?? '-'}
        </span>
      </div>

      {/* Rows slider */}
      <div className="flex items-center gap-1.5">
        <label
          htmlFor="grid-rows"
          className="text-[10px] uppercase tracking-wider font-semibold text-gray-400 dark:text-gray-500"
        >
          Rows
        </label>
        <input
          id="grid-rows"
          type="range"
          min={1}
          max={4}
          step={1}
          value={gridOverride?.rows ?? 2}
          onChange={handleRowsChange}
          disabled={isAutoMode}
          className={cn(
            'w-16 h-1 rounded-full appearance-none cursor-pointer',
            'accent-indigo-500',
            '[&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:bg-indigo-500',
            isAutoMode && 'opacity-40 cursor-not-allowed'
          )}
        />
        <span className="text-xs font-mono text-gray-500 dark:text-gray-400 tabular-nums w-3 text-center">
          {gridOverride?.rows ?? '-'}
        </span>
      </div>

      {/* Separator */}
      <div className="h-4 w-px bg-gray-200 dark:bg-gray-700" />

      {/* Auto button */}
      <button
        type="button"
        onClick={handleAutoClick}
        className={cn(
          'flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium transition-colors',
          isAutoMode
            ? 'bg-indigo-500/10 text-indigo-400 border border-indigo-500/30'
            : 'text-gray-500 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-200/50 dark:hover:bg-gray-700/50 border border-transparent'
        )}
        aria-pressed={isAutoMode}
      >
        <RotateCcw className="h-3 w-3" />
        Auto
      </button>

      {/* Separator */}
      <div className="h-4 w-px bg-gray-200 dark:bg-gray-700" />

      {/* Compact toggle */}
      <button
        type="button"
        onClick={handleCompactToggle}
        className={cn(
          'flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium transition-colors',
          compactHeaders
            ? 'bg-indigo-500/10 text-indigo-400 border border-indigo-500/30'
            : 'text-gray-500 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-200/50 dark:hover:bg-gray-700/50 border border-transparent'
        )}
        aria-pressed={compactHeaders}
      >
        {compactHeaders ? (
          <Minimize2 className="h-3 w-3" />
        ) : (
          <Maximize2 className="h-3 w-3" />
        )}
        Compact
      </button>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Session count badge */}
      <div className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400">
        <span className="font-mono tabular-nums text-gray-700 dark:text-gray-300">
          {visibleCount}
        </span>
        {visibleCount !== sessionCount && (
          <>
            <span>of</span>
            <span className="font-mono tabular-nums text-gray-700 dark:text-gray-300">
              {sessionCount}
            </span>
          </>
        )}
        <span>{sessionCount === 1 ? 'session' : 'sessions'}</span>
      </div>
    </div>
  )
}

export type { GridControlsProps }
