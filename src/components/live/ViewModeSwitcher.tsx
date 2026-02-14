import { LayoutGrid, List, Columns3, Monitor } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { LiveViewMode } from '../../types/live'
import { LIVE_VIEW_MODES } from '../../types/live'

const ICON_MAP = {
  LayoutGrid,
  List,
  Columns3,
  Monitor,
} as const

interface ViewModeSwitcherProps {
  mode: LiveViewMode
  onChange: (mode: LiveViewMode) => void
}

export function ViewModeSwitcher({ mode, onChange }: ViewModeSwitcherProps) {
  return (
    <div className="hidden sm:flex items-center gap-1 p-1 rounded-lg bg-gray-100/50 dark:bg-gray-900/50 border border-gray-200 dark:border-gray-800">
      {LIVE_VIEW_MODES.map((vm) => {
        const Icon = ICON_MAP[vm.icon]
        const isActive = mode === vm.id
        return (
          <button
            key={vm.id}
            type="button"
            onClick={() => onChange(vm.id)}
            className={cn(
              'flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-colors',
              isActive
                ? 'bg-indigo-500/10 text-indigo-400 border-b-2 border-indigo-500'
                : 'text-gray-500 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-100/50 dark:hover:bg-gray-800/50'
            )}
            aria-pressed={isActive}
          >
            <Icon className="w-3.5 h-3.5" />
            <span>{vm.label}</span>
            <kbd className="hidden md:inline-block ml-1 px-1 py-0.5 text-[10px] font-mono text-gray-400 dark:text-gray-500 bg-gray-100/50 dark:bg-gray-800/50 rounded">
              {vm.shortcut}
            </kbd>
          </button>
        )
      })}
    </div>
  )
}
