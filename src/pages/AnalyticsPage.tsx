import { useSearchParams } from 'react-router-dom'
import { StatsDashboard } from '../components/StatsDashboard'
import { ContributionsPage } from './ContributionsPage'
import { cn } from '../lib/utils'

type AnalyticsTab = 'overview' | 'contributions'

const TABS: { id: AnalyticsTab; label: string }[] = [
  { id: 'overview', label: 'Overview' },
  { id: 'contributions', label: 'Contributions' },
]

function isValidTab(value: string | null): value is AnalyticsTab {
  return value !== null && TABS.some(t => t.id === value)
}

export function AnalyticsPage() {
  const [searchParams, setSearchParams] = useSearchParams()
  const activeTab: AnalyticsTab = isValidTab(searchParams.get('tab'))
    ? (searchParams.get('tab') as AnalyticsTab)
    : 'overview'

  const handleTabChange = (tab: AnalyticsTab) => {
    const params = new URLSearchParams(searchParams)
    if (tab === 'overview') {
      params.delete('tab')
    } else {
      params.set('tab', tab)
    }
    setSearchParams(params)
  }

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Tab bar */}
      <div className="flex items-center gap-1 px-6 pt-4 pb-0">
        {TABS.map(tab => (
          <button
            key={tab.id}
            type="button"
            onClick={() => handleTabChange(tab.id)}
            className={cn(
              'inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-md transition-colors duration-150 cursor-pointer',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
              activeTab === tab.id
                ? 'bg-blue-500 text-white'
                : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-hidden">
        {activeTab === 'overview' && <StatsDashboard />}
        {activeTab === 'contributions' && <ContributionsPage />}
      </div>
    </div>
  )
}
