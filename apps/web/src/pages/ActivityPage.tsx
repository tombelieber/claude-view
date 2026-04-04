import { CalendarDays, X } from 'lucide-react'
import { useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import { CalendarHeatmap } from '../components/activity/CalendarHeatmap'
import { DailyTimeline } from '../components/activity/DailyTimeline'
import { ProjectBreakdown } from '../components/activity/ProjectBreakdown'
import { SummaryStats } from '../components/activity/SummaryStats'
import { DateRangePicker, TimeRangeSelector } from '../components/ui'
import { useActivityCombined } from '../hooks/use-activity-combined'
import { useIsMobile } from '../hooks/use-media-query'
import { useTimeRange } from '../hooks/use-time-range'
import type { TimeRangePreset } from '../hooks/use-time-range'

/** Human-readable labels for the summary header */
const PRESET_LABELS: Record<TimeRangePreset, string> = {
  today: 'Today',
  '7d': 'This Week',
  '30d': 'This Month',
  '90d': '3 Months',
  all: 'All Time',
  custom: 'Custom',
}

export function ActivityPage() {
  const [searchParams] = useSearchParams()
  const sidebarProject = searchParams.get('project')
  const sidebarBranch = searchParams.get('branch')
  const isMobile = useIsMobile()

  const { state: timeRange, setPreset, setCustomRange } = useTimeRange()
  const { data, isLoading, error } = useActivityCombined(
    timeRange.fromTimestamp,
    timeRange.toTimestamp,
    sidebarProject,
    sidebarBranch,
  )

  const [selectedDate, setSelectedDate] = useState<string | null>(null)
  const [selectedProject, setSelectedProject] = useState<string | null>(null)

  const activeLabel = PRESET_LABELS[timeRange.preset] ?? 'Custom'

  // Clear sub-filters when time range changes
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
        {/* Time range picker — shared component with HistoryView */}
        <div className="flex items-center gap-2">
          <TimeRangeSelector
            value={timeRange.preset}
            onChange={handlePresetChange}
            options={[
              { value: 'today', label: isMobile ? 'Today' : 'Today' },
              { value: '7d', label: isMobile ? '7 days' : '7d' },
              { value: '30d', label: isMobile ? '30 days' : '30d' },
              { value: '90d', label: isMobile ? '90 days' : '90d' },
              { value: 'all', label: isMobile ? 'All time' : 'All' },
              { value: 'custom', label: 'Custom' },
            ]}
          />
          {timeRange.preset === 'custom' && (
            <DateRangePicker value={timeRange.customRange} onChange={setCustomRange} />
          )}
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
              {selectedDate} <X className="w-3 h-3 inline ml-1" />
            </button>
          )}
          {selectedProject && (
            <button
              type="button"
              onClick={() => setSelectedProject(null)}
              className="bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 px-2 py-0.5 rounded cursor-pointer hover:bg-blue-200 dark:hover:bg-blue-900/60"
            >
              {selectedProject.split('/').pop()} <X className="w-3 h-3 inline ml-1" />
            </button>
          )}
        </div>
      )}

      {/* Content */}
      <div className="px-6 pb-6 space-y-6">
        {isLoading && (
          <div className="space-y-6 animate-pulse">
            {/* Stats skeleton */}
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
              {[...Array(8)].map((_, i) => (
                <div key={i} className="h-16 rounded-lg bg-gray-100 dark:bg-gray-800" />
              ))}
            </div>
            {/* Heatmap skeleton */}
            <div className="space-y-1">
              {[...Array(5)].map((_, i) => (
                <div key={i} className="h-9 rounded bg-gray-100 dark:bg-gray-800" />
              ))}
            </div>
            {/* Timeline skeleton */}
            <div className="space-y-2">
              {[...Array(3)].map((_, i) => (
                <div key={i} className="h-10 rounded-lg bg-gray-100 dark:bg-gray-800" />
              ))}
            </div>
          </div>
        )}
        {error && (
          <div className="text-sm text-red-500">Failed to load activity: {error.message}</div>
        )}
        {data && (
          <>
            <SummaryStats summary={data.summary} label={activeLabel} />
            {data.summary.sessionCount > 0 && (
              <>
                <div className="md:grid md:grid-cols-[auto_1fr] md:gap-6 md:items-start">
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
                </div>
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
