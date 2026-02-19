import { LayoutGrid, Columns3, List } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { LiveViewMode } from './types'

interface MobileTabBarProps {
  activeTab: LiveViewMode
  onTabChange: (tab: LiveViewMode) => void
}

const tabs: { label: string; icon: typeof LayoutGrid; mode: LiveViewMode }[] = [
  { label: 'Board', icon: Columns3, mode: 'kanban' },
  { label: 'Grid', icon: LayoutGrid, mode: 'grid' },
  { label: 'List', icon: List, mode: 'list' },
]

export function MobileTabBar({ activeTab, onTabChange }: MobileTabBarProps) {
  // Monitor mode doesn't have a mobile tab; treat it as grid
  const resolvedTab = activeTab === 'monitor' ? 'grid' : activeTab

  return (
    <nav className="flex sm:hidden fixed bottom-0 inset-x-0 z-40 bg-white/95 dark:bg-gray-950/95 backdrop-blur-md border-t border-gray-200 dark:border-gray-800 pb-[env(safe-area-inset-bottom)]">
      {tabs.map(({ label, icon: Icon, mode }) => (
        <button
          key={mode}
          type="button"
          onClick={() => onTabChange(mode)}
          className={cn(
            'flex-1 flex flex-col items-center justify-center min-h-[44px] min-w-[44px] py-2',
            resolvedTab === mode ? 'text-indigo-400' : 'text-gray-400 dark:text-gray-500'
          )}
        >
          <Icon className="w-5 h-5" />
          <span className="text-[10px] mt-0.5">{label}</span>
        </button>
      ))}
    </nav>
  )
}
