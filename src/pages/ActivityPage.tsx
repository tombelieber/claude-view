import { useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { CalendarDays } from 'lucide-react'
import { useTimeRange } from '../hooks/use-time-range'
import { useActivityData } from '../hooks/use-activity-data'
import { SummaryStats } from '../components/activity/SummaryStats'
import { CalendarHeatmap } from '../components/activity/CalendarHeatmap'
import { ProjectBreakdown } from '../components/activity/ProjectBreakdown'
import { DailyTimeline } from '../components/activity/DailyTimeline'
import { cn } from '../lib/utils'
import type { TimeRangePreset } from '../hooks/use-time-range'

const PRESETS: { id: TimeRangePreset; label: string }[] = [
  { id: 'today', label: 'Today' },
  { id: '7d', label: 'This Week' },
  { id: '30d', label: 'This Month' },
  { id: '90d', label: '3 Months' },
  { id: 'all', label: 'All Time' },
]

export function ActivityPage() {
  const [searchParams] = useSearchParams()
  const sidebarProject = searchParams.get('project')
  const sidebarBranch = searchParams.get('branch')

  const { state: timeRange, setPreset } = useTimeRange()
  const { data, isLoading, error } = useActivityData(
    timeRange.fromTimestamp,
    timeRange.toTimestamp,
    sidebarProject,
    sidebarBranch,
  )

  const [selectedDate, setSelectedDate] = useState<string | null>(null)
  const [selectedProject, setSelectedProject] = useState<string | null>(null)

  const activeLabel = PRESETS.find(p => p.id === timeRange.preset)?.label ?? 'Custom'

  // Clear filters when time range changes
  const handlePresetChange = (preset: TimeRangePreset) => {
    setPreset(preset)
    setSelectedDate(null)
    setSelectedProject(null)
  }

  return (
    <div className="h-full flex flex-col overflow-y-auto">
      {/* Header */}
      <div className="px-6 pt-6 pb-2 flex items-center justify-between flex-wrap gap-2">
        <div className="flex items-center gap-2">
          <CalendarDays className="w-5 h-5 text-blue-500" />
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Activity</h1>
        </div>
        {/* Time range picker */}
        <div className="flex items-center gap-1">
          {PRESETS.map((preset) => (
            <button
              key={preset.id}
              type="button"
              onClick={() => handlePresetChange(preset.id)}
              className={cn(
                'px-3 py-1 text-xs font-medium rounded-md transition-colors duration-150 cursor-pointer',
                'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400',
                timeRange.preset === preset.id
                  ? 'bg-blue-500 text-white'
                  : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
              )}
            >
              {preset.label}
            </button>
          ))}
        </div>
      </div>

      {/* Active filters indicator */}
      {(selectedDate || selectedProject) && (
        <div className="px-6 pb-2 flex items-center gap-2 text-xs">
          <span className="text-gray-400">Filtered by:</span>
          {selectedDate && (
            <button
              type="button"
              onClick={() => setSelectedDate(null)}
              className="bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 px-2 py-0.5 rounded cursor-pointer hover:bg-blue-200 dark:hover:bg-blue-900/60"
            >
              {selectedDate} x
            </button>
          )}
          {selectedProject && (
            <button
              type="button"
              onClick={() => setSelectedProject(null)}
              className="bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 px-2 py-0.5 rounded cursor-pointer hover:bg-blue-200 dark:hover:bg-blue-900/60"
            >
              {selectedProject.split('/').pop()} x
            </button>
          )}
        </div>
      )}

      {/* Content */}
      <div className="px-6 pb-6 space-y-6">
        {isLoading && (
          <div className="flex items-center justify-center py-12 text-sm text-gray-400">Loading activity...</div>
        )}
        {error && (
          <div className="text-sm text-red-500">Failed to load activity: {error.message}</div>
        )}
        {data && (
          <>
            <SummaryStats summary={data.summary} label={activeLabel} />
            {data.summary.sessionCount > 0 && (
              <>
                <CalendarHeatmap
                  days={data.days}
                  onDayClick={setSelectedDate}
                  selectedDate={selectedDate}
                />
                <ProjectBreakdown
                  projects={data.projects}
                  onProjectClick={setSelectedProject}
                  selectedProject={selectedProject}
                />
                <DailyTimeline
                  days={data.days}
                  selectedDate={selectedDate}
                  selectedProject={selectedProject}
                />
              </>
            )}
          </>
        )}
      </div>
    </div>
  )
}
