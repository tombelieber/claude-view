import { Columns3, LayoutGrid, List, Monitor, Workflow } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { LiveViewMode } from './types'
import { LIVE_VIEW_MODES } from './types'

const ICON_MAP = {
  LayoutGrid,
  List,
  Columns3,
  Monitor,
  Workflow,
} as const

interface ViewModeSwitcherProps {
  mode: LiveViewMode
  onChange: (mode: LiveViewMode) => void
}

export function ViewModeSwitcher({ mode, onChange }: ViewModeSwitcherProps) {
  return (
    <div className="hidden sm:flex items-center gap-1 p-1 rounded-lg bg-gray-100/50 dark:bg-gray-800/60 border border-gray-200/80 dark:border-gray-700/60">
      {LIVE_VIEW_MODES.map((vm) => {
        const Icon = ICON_MAP[vm.icon]
        const isActive = mode === vm.id
        return (
          <button
            key={vm.id}
            type="button"
            onClick={() => onChange(vm.id)}
            title={`${vm.label} (${vm.shortcut})`}
            className={cn(
              'flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs font-medium transition-all duration-150',
              isActive
                ? 'bg-white dark:bg-gray-700 text-indigo-500 dark:text-indigo-400 shadow-sm'
                : 'text-gray-400 dark:text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-white/50 dark:hover:bg-gray-700/50',
            )}
            aria-pressed={isActive}
            aria-label={`${vm.label} view`}
          >
            <Icon className="w-3.5 h-3.5" />
            <span>{vm.label}</span>
          </button>
        )
      })}
    </div>
  )
}
