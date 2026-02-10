import { cn } from '../../lib/utils'
import type { TabId } from '../../hooks/use-insights'

interface PatternsTabsProps {
  activeTab: TabId
  onTabChange: (tab: TabId) => void
  disabledTabs?: TabId[]
}

const TABS: { id: TabId; label: string }[] = [
  { id: 'patterns', label: 'Patterns' },
  { id: 'categories', label: 'Categories' },
  { id: 'trends', label: 'Trends' },
  { id: 'benchmarks', label: 'Benchmarks' },
  { id: 'quality', label: 'Quality' },
]

export function PatternsTabs({
  activeTab,
  onTabChange,
  disabledTabs = [],
}: PatternsTabsProps) {
  return (
    <div className="flex items-center gap-1 border-b border-gray-200 dark:border-gray-700">
      {TABS.map((tab) => {
        const isDisabled = disabledTabs.includes(tab.id)
        return (
          <button
            key={tab.id}
            onClick={() => !isDisabled && onTabChange(tab.id)}
            disabled={isDisabled}
            className={cn(
              'px-4 py-2.5 text-sm font-medium transition-colors border-b-2 -mb-px cursor-pointer',
              activeTab === tab.id
                ? 'text-blue-600 dark:text-blue-400 border-blue-600 dark:border-blue-400'
                : 'text-gray-500 dark:text-gray-400 border-transparent hover:text-gray-700 dark:hover:text-gray-300',
              isDisabled && 'opacity-50 cursor-not-allowed'
            )}
          >
            {tab.label}
            {isDisabled && (
              <span className="ml-1.5 text-[10px] text-gray-400 dark:text-gray-500">
                Soon
              </span>
            )}
          </button>
        )
      })}
    </div>
  )
}
