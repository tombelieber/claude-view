import { useSearchParams } from 'react-router-dom'
import { FlaskConical } from 'lucide-react'
import { StatsDashboard } from '../components/StatsDashboard'
import { ContributionsPage } from './ContributionsPage'
import { InsightsPage } from '../components/InsightsPage'
import { cn } from '../lib/utils'

type AnalyticsTab = 'overview' | 'contributions' | 'insights'

const TABS: { id: AnalyticsTab; label: string; experimental?: boolean }[] = [
  { id: 'overview', label: 'Overview' },
  { id: 'contributions', label: 'Contributions' },
  { id: 'insights', label: 'Insights', experimental: true },
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
            {tab.experimental && (
              <span className={cn(
                'inline-flex items-center gap-0.5 px-1.5 py-0 text-[10px] font-medium rounded-full border',
                activeTab === tab.id
                  ? 'border-white/30 text-white/80'
                  : 'border-amber-300 dark:border-amber-700 text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/40'
              )}>
                <FlaskConical className="w-2.5 h-2.5" />
                Experimental
              </span>
            )}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-hidden">
        {activeTab === 'overview' && <StatsDashboard />}
        {activeTab === 'contributions' && <ContributionsPage />}
        {activeTab === 'insights' && <InsightsPage />}
      </div>
    </div>
  )
}
