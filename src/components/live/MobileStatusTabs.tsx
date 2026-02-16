import type { AgentStateGroup } from './types'
import type { LiveSummary } from './use-live-sessions'
import { cn } from '../../lib/utils'

interface MobileStatusTabsProps {
  activeGroup: AgentStateGroup | null
  onGroupChange: (group: AgentStateGroup | null) => void
  summary: LiveSummary | null
}

const TABS: { group: AgentStateGroup; label: string; color: string }[] = [
  { group: 'needs_you', label: 'Needs You', color: 'text-amber-500 border-amber-500' },
  { group: 'autonomous', label: 'Running', color: 'text-green-500 border-green-500' },
]

export function MobileStatusTabs({ activeGroup, onGroupChange, summary }: MobileStatusTabsProps) {
  function getCount(group: AgentStateGroup): number {
    if (!summary) return 0
    switch (group) {
      case 'needs_you': return summary.needsYouCount
      case 'autonomous': return summary.autonomousCount
    }
  }

  return (
    <div className="flex sm:hidden border-b border-gray-200 dark:border-gray-800 mb-4">
      {TABS.map(tab => {
        const isActive = activeGroup === tab.group
        const count = getCount(tab.group)
        return (
          <button
            key={tab.group}
            onClick={() => onGroupChange(isActive ? null : tab.group)}
            className={cn(
              'flex-1 text-center py-2 text-xs font-medium border-b-2 transition-colors',
              isActive
                ? tab.color
                : 'text-gray-400 dark:text-gray-500 border-transparent hover:text-gray-600 dark:hover:text-gray-300'
            )}
          >
            {tab.label}
            {count > 0 && (
              <span className="ml-1 px-1.5 py-0.5 text-[10px] rounded-full bg-gray-100 dark:bg-gray-800">
                {count}
              </span>
            )}
          </button>
        )
      })}
    </div>
  )
}
