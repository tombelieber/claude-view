import { Columns, LayoutGrid } from 'lucide-react'
import { cn } from '../../lib/utils'

interface LayoutModeToggleProps {
  mode: 'auto-grid' | 'custom'
  onToggle: () => void
}

export function LayoutModeToggle({ mode, onToggle }: LayoutModeToggleProps) {
  return (
    <div className="flex rounded-lg bg-gray-100/50 dark:bg-gray-900/50 border border-gray-200 dark:border-gray-800 p-0.5 gap-0.5">
      <button
        type="button"
        onClick={onToggle}
        title="Auto: responsive grid"
        aria-pressed={mode === 'auto-grid'}
        className={cn(
          'flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors',
          mode === 'auto-grid'
            ? 'bg-indigo-500/10 text-indigo-400 border-b-2 border-indigo-500'
            : 'text-gray-500 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-100/50 dark:hover:bg-gray-800/50',
        )}
      >
        <LayoutGrid className="w-3.5 h-3.5" />
        <span>Auto</span>
      </button>
      <button
        type="button"
        onClick={onToggle}
        title="Custom: drag to arrange"
        aria-pressed={mode === 'custom'}
        className={cn(
          'flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors',
          mode === 'custom'
            ? 'bg-indigo-500/10 text-indigo-400 border-b-2 border-indigo-500'
            : 'text-gray-500 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-100/50 dark:hover:bg-gray-800/50',
        )}
      >
        <Columns className="w-3.5 h-3.5" />
        <span>Custom</span>
      </button>
    </div>
  )
}

export type { LayoutModeToggleProps }
