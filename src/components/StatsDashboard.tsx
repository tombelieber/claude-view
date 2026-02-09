import { useState, useRef, useEffect, useCallback } from 'react'
import { useNavigate, useSearchParams, Link } from 'react-router-dom'
import { BarChart3, Sparkles, TerminalSquare, Plug, Bot, FolderOpen, Calendar, Pencil, Eye, Terminal, Clock, ArrowRight, Search } from 'lucide-react'
import * as Tooltip from '@radix-ui/react-tooltip'
import { useDashboardStats } from '../hooks/use-dashboard'
import { buildSessionUrl } from '../lib/url-utils'
import { useTimeRange } from '../hooks/use-time-range'
import { cn } from '../lib/utils'
import { DashboardSkeleton, ErrorState, EmptyState } from './LoadingStates'
import { DashboardMetricsGrid } from './DashboardMetricsGrid'
import { AIGenerationStats } from './AIGenerationStats'
import { ContributionSummaryCard } from './ContributionSummaryCard'
import { TimeRangeSelector, DateRangePicker } from './ui'
import { useIsMobile } from '../hooks/use-media-query'

/** Format a timestamp to a human-readable date */
function formatTimestampDate(ts: number | null | undefined): string {
  if (ts === null || ts === undefined || ts <= 0) return ''
  return new Date(ts * 1000).toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  })
}

/** Format a timestamp to short date (month + year) */
function formatShortDate(ts: number | null | undefined): string {
  if (ts === null || ts === undefined || ts <= 0) return ''
  return new Date(ts * 1000).toLocaleDateString('en-US', {
    month: 'short',
    year: 'numeric',
  })
}

export function StatsDashboard() {
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const projectFilter = searchParams.get("project") || undefined
  const branchFilter = searchParams.get("branch") || undefined
  const isMobile = useIsMobile()

  // Time range state
  const { state: timeRange, setPreset, setCustomRange, comparisonLabel } = useTimeRange()

  // Fetch dashboard stats with project/branch + time range filters
  const { data: stats, isLoading, error, refetch } = useDashboardStats(
    projectFilter,
    branchFilter,
    { from: timeRange.fromTimestamp, to: timeRange.toTimestamp }
  )

  // Loading state with skeleton
  if (isLoading) {
    return <DashboardSkeleton />
  }

  // Error state with retry
  if (error) {
    return (
      <div className="h-full flex items-center justify-center">
        <ErrorState
          message={error.message}
          onRetry={() => refetch()}
        />
      </div>
    )
  }

  // Empty state
  if (!stats) {
    return (
      <div className="h-full flex items-center justify-center">
        <EmptyState
          icon={<BarChart3 className="w-6 h-6 text-gray-400" />}
          title="No statistics available"
          description="Stats will appear once you have some session data."
        />
      </div>
    )
  }

  const maxProjectSessions = stats.topProjects[0]?.sessionCount || 1

  const handleInvocableClick = (name: string) => {
    navigate(`/search?q=${encodeURIComponent(`skill:${name.replace('/', '')}`)}`)
  }

  const invocableCategories = [
    { title: 'Top Skills', icon: Sparkles, data: stats.topSkills, color: 'bg-[#7c9885]' },
    { title: 'Top Commands', icon: TerminalSquare, data: stats.topCommands, color: 'bg-blue-500' },
    { title: 'Top MCP Tools', icon: Plug, data: stats.topMcpTools, color: 'bg-purple-500' },
    { title: 'Top Agents', icon: Bot, data: stats.topAgents, color: 'bg-amber-500' },
  ].filter(cat => cat.data.length > 0)

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-4xl mx-auto space-y-6">
      {/* Header Card */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-4 sm:p-6">
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 sm:gap-4 mb-4">
          <div className="flex items-center gap-2">
            <BarChart3 className="w-5 h-5 text-[#7c9885]" />
            <h1 className="text-lg sm:text-xl font-semibold text-gray-900 dark:text-gray-100">
              {projectFilter
                ? `${projectFilter} Usage`
                : "Your Claude Code Usage"}
            </h1>
            {projectFilter && (
              <button
                onClick={() => {
                  const params = new URLSearchParams(searchParams)
                  params.delete("project")
                  params.delete("branch")
                  setSearchParams(params)
                }}
                className="text-xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
              >
                Clear filter
              </button>
            )}
          </div>

          {/* Time Range Selector */}
          <div className="flex items-center gap-2">
            <TimeRangeSelector
              value={timeRange.preset}
              onChange={setPreset}
              options={[
                { value: '7d', label: isMobile ? '7 days' : '7d' },
                { value: '30d', label: isMobile ? '30 days' : '30d' },
                { value: '90d', label: isMobile ? '90 days' : '90d' },
                { value: 'all', label: isMobile ? 'All time' : 'All' },
                { value: 'custom', label: 'Custom' },
              ]}
            />
            {timeRange.preset === 'custom' && (
              <DateRangePicker
                value={timeRange.customRange}
                onChange={setCustomRange}
              />
            )}
          </div>
        </div>

        <div className="flex items-center gap-6 text-sm text-gray-600 dark:text-gray-400">
          <div>
            <span className="text-2xl font-bold text-gray-900 dark:text-gray-100 tabular-nums">{stats.totalSessions}</span>
            <span className="ml-1">sessions</span>
          </div>
          <div className="w-px h-8 bg-gray-200 dark:bg-gray-700" />
          <div>
            <span className="text-2xl font-bold text-gray-900 dark:text-gray-100 tabular-nums">{stats.totalProjects}</span>
            <span className="ml-1">projects</span>
          </div>
        </div>

        {/* Date Range Caption */}
        <div className="mt-3 pt-3 border-t border-gray-100 dark:border-gray-800 text-xs text-gray-500 dark:text-gray-400 flex items-center justify-between">
          <span>
            {stats.periodStart && stats.periodEnd
              ? `Showing stats from ${formatTimestampDate(stats.periodStart)} - ${formatTimestampDate(stats.periodEnd)}`
              : 'Showing all-time stats'}
          </span>
          {stats.dataStartDate && (
            <span className="text-gray-400 dark:text-gray-500">
              since {formatShortDate(stats.dataStartDate)}
            </span>
          )}
        </div>
      </div>

      {/* Phase 3: Week-over-week metrics grid */}
      {stats.trends && (
        <DashboardMetricsGrid trends={stats.trends} comparisonLabel={comparisonLabel} />
      )}

      {/* Theme 3: AI Contribution Summary Card */}
      <ContributionSummaryCard
        timeRange={timeRange}
        project={projectFilter}
        branch={branchFilter}
      />

      {/* AI Generation Breakdown */}
      <AIGenerationStats
        timeRange={{ from: timeRange.fromTimestamp, to: timeRange.toTimestamp }}
        project={projectFilter}
        branch={branchFilter}
      />

      <div className="grid md:grid-cols-2 gap-6">
        {/* Invocable category cards — self-contained leaderboards, items are clickable */}
        {invocableCategories.map(({ title, icon: Icon, data, color }) => {
          const maxCount = data[0]?.count || 1
          return (
            <div key={title} className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
              <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-1.5">
                <Icon className="w-4 h-4" />
                {title}
              </h2>
              <div className="space-y-3">
                {data.map((item) => (
                  <button
                    key={item.name}
                    onClick={() => handleInvocableClick(item.name)}
                    className="w-full min-h-[44px] group text-left py-2 focus-visible:ring-2 focus-visible:ring-blue-400 rounded-md"
                  >
                    <div className="flex items-center justify-between text-sm mb-1">
                      <span className="font-mono text-gray-700 dark:text-gray-300 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors truncate mr-2">
                        {item.name}
                      </span>
                      <span className="tabular-nums text-gray-400 shrink-0">{item.count}</span>
                    </div>
                    <div className="h-2 bg-gray-100 dark:bg-gray-800 rounded-full overflow-hidden">
                      <div
                        className={cn('h-full group-hover:bg-blue-500 transition-colors rounded-full', color)}
                        style={{ width: `${(item.count / maxCount) * 100}%` }}
                      />
                    </div>
                  </button>
                ))}
              </div>
              {/* Hint: items are interactive */}
              <p className="mt-3 pt-3 border-t border-gray-100 dark:border-gray-800 text-[11px] text-gray-400 flex items-center gap-1">
                <Search className="w-3 h-3" />
                Click any item to find matching sessions
              </p>
            </div>
          )
        })}

        {/* Most Active Projects — items link to project pages, hidden when filtered to a single project */}
        {!projectFilter && (
          <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
            <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-1.5">
              <FolderOpen className="w-4 h-4" />
              Most Active Projects
            </h2>
            <div className="space-y-3">
              {stats.topProjects.map((project) => (
                <button
                  key={project.name}
                  onClick={() => {
                    const params = new URLSearchParams(searchParams)
                    params.set('project', project.name)
                    setSearchParams(params)
                  }}
                  className="w-full group text-left focus-visible:ring-2 focus-visible:ring-blue-400"
                >
                  <div className="flex items-center justify-between text-sm mb-1">
                    <span className="text-gray-700 dark:text-gray-300 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors">
                      {project.displayName}
                    </span>
                    <span className="tabular-nums text-gray-400">{project.sessionCount}</span>
                  </div>
                  <div className="h-2 bg-gray-100 dark:bg-gray-800 rounded-full overflow-hidden">
                    <div
                      className="h-full rounded-full transition-colors bg-gray-300 dark:bg-gray-600 group-hover:bg-blue-500"
                      style={{ width: `${(project.sessionCount / maxProjectSessions) * 100}%` }}
                    />
                  </div>
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Longest Sessions — "See all" links to sorted history */}
        {stats.longestSessions.length > 0 && (
          <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider flex items-center gap-1.5">
                <Clock className="w-4 h-4" />
                Longest Sessions
              </h2>
              <Link
                to="/history?sort=duration"
                className="text-xs text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 transition-colors flex items-center gap-0.5"
              >
                See all <ArrowRight className="w-3 h-3" />
              </Link>
            </div>
            <div className="space-y-3">
              {stats.longestSessions.map((session) => {
                const maxDuration = stats.longestSessions[0]?.durationSeconds || 1
                return (
                  <Link
                    key={session.id}
                    to={buildSessionUrl(session.id, searchParams)}
                    className="w-full group block focus-visible:ring-2 focus-visible:ring-blue-400"
                  >
                    <div className="flex items-center justify-between text-sm mb-1">
                      <span className="text-gray-700 dark:text-gray-300 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition-colors truncate mr-2">
                        {session.preview || session.projectDisplayName}
                      </span>
                      <span className="tabular-nums text-gray-400 whitespace-nowrap">
                        {formatDuration(session.durationSeconds)}
                      </span>
                    </div>
                    <div className="h-2 bg-gray-100 dark:bg-gray-800 rounded-full overflow-hidden">
                      <div
                        className="h-full rounded-full transition-colors bg-orange-300 group-hover:bg-orange-500"
                        style={{ width: `${(session.durationSeconds / maxDuration) * 100}%` }}
                      />
                    </div>
                  </Link>
                )
              })}
            </div>
          </div>
        )}
      </div>

      {/* Activity Heatmap */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider flex items-center gap-1.5">
            <Calendar className="w-4 h-4" />
            Activity (Last 90 Days)
          </h2>
          <Link
            to="/sessions"
            className="text-xs text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 transition-colors flex items-center gap-0.5"
          >
            All sessions <ArrowRight className="w-3 h-3" />
          </Link>
        </div>
        <ActivityHeatmap data={stats.heatmap} navigate={navigate} isMobile={isMobile} />
      </div>

      {/* Global Tool Usage */}
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-4 sm:p-6">
        <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-4">
          Tool Usage
        </h2>
        <div className="grid grid-cols-3 gap-2 sm:gap-4">
          {[
            { label: 'Edits', value: stats.toolTotals.edit + stats.toolTotals.write, icon: Pencil, color: 'text-blue-500' },
            { label: 'Reads', value: stats.toolTotals.read, icon: Eye, color: 'text-green-500' },
            { label: 'Bash', value: stats.toolTotals.bash, icon: Terminal, color: 'text-amber-500' },
          ].map(({ label, value, icon: Icon, color }) => (
            <div key={label} className="text-center p-3 sm:p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
              <Icon className={cn('w-5 h-5 sm:w-6 sm:h-6 mx-auto mb-1 sm:mb-2', color)} />
              <p className="text-xl sm:text-2xl font-bold text-gray-900 dark:text-gray-100 tabular-nums">{value}</p>
              <p className="text-xs text-gray-500 dark:text-gray-400">{label}</p>
            </div>
          ))}
        </div>
      </div>
      </div>
    </div>
  )
}

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.round(seconds / 60)
  if (minutes < 60) return `${minutes}m`
  const hours = seconds / 3600
  return `${hours.toFixed(1)}h`
}

/** Format a date string (YYYY-MM-DD) for tooltip display */
function formatHeatmapDate(dateStr: string): string {
  const date = new Date(dateStr + 'T00:00:00')
  return new Intl.DateTimeFormat('en-US', {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
  }).format(date)
}

/** Heatmap cell with rich tooltip and 150ms close delay (Feature 2B) */
function HeatmapCell({
  day,
  isMobile,
  colorClass,
  onClick,
}: {
  day: { date: string; count: number }
  isMobile: boolean
  colorClass: string
  onClick: () => void
}) {
  const [isOpen, setIsOpen] = useState(false)
  const closeTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const handleOpenChange = useCallback((open: boolean) => {
    if (closeTimeoutRef.current) {
      clearTimeout(closeTimeoutRef.current)
      closeTimeoutRef.current = null
    }
    if (open) {
      setIsOpen(true)
    } else {
      // 150ms close delay to prevent flickering
      closeTimeoutRef.current = setTimeout(() => setIsOpen(false), 150)
    }
  }, [])

  useEffect(() => {
    return () => {
      if (closeTimeoutRef.current) clearTimeout(closeTimeoutRef.current)
    }
  }, [])

  const formattedDate = formatHeatmapDate(day.date)
  const label = `${formattedDate}: ${day.count} session${day.count !== 1 ? 's' : ''}`

  return (
    <Tooltip.Root delayDuration={0} open={isOpen} onOpenChange={handleOpenChange}>
      <Tooltip.Trigger asChild>
        <button
          onClick={onClick}
          className={cn(
            isMobile ? 'w-4 h-4' : 'w-3 h-3',
            'rounded-sm transition-colors hover:ring-2 hover:ring-blue-400 focus-visible:ring-2 focus-visible:ring-blue-400',
            colorClass
          )}
          aria-label={label}
        />
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content
          role="tooltip"
          className="bg-gray-900 text-white px-3 py-2 rounded-lg shadow-lg text-sm z-50 animate-in fade-in-0 zoom-in-95"
          sideOffset={5}
          side="top"
          align="center"
        >
          <div className="font-medium">{formattedDate}</div>
          <div className="text-gray-200">{day.count} session{day.count !== 1 ? 's' : ''}</div>
          <div className="text-gray-400 text-xs mt-1">Click to filter</div>
          <Tooltip.Arrow className="fill-gray-900" />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  )
}

// Activity Heatmap Component
function ActivityHeatmap({
  data,
  navigate,
  isMobile,
}: {
  data: { date: string; count: number }[]
  navigate: (path: string) => void
  isMobile: boolean
}) {
  const maxCount = Math.max(...data.map(d => d.count), 1)

  const getColor = (count: number) => {
    if (count === 0) return 'bg-gray-100 dark:bg-gray-800'
    const intensity = count / maxCount
    if (intensity > 0.66) return 'bg-green-500'
    if (intensity > 0.33) return 'bg-green-300 dark:bg-green-400'
    return 'bg-green-200 dark:bg-green-600'
  }

  const handleDayClick = (dateStr: string) => {
    const date = new Date(dateStr)
    const nextDay = new Date(date)
    nextDay.setDate(nextDay.getDate() + 1)
    const nextDateStr = nextDay.toISOString().split('T')[0]
    navigate(`/search?q=${encodeURIComponent(`after:${dateStr} before:${nextDateStr}`)}`)
  }

  // Group by week
  const weeks: typeof data[] = []
  let currentWeek: typeof data = []

  for (const day of data) {
    if (currentWeek.length === 7) {
      weeks.push(currentWeek)
      currentWeek = []
    }
    currentWeek.push(day)
  }
  if (currentWeek.length > 0) weeks.push(currentWeek)

  // On mobile, limit to last 12 weeks for better UX
  const displayWeeks = isMobile ? weeks.slice(-12) : weeks
  const showScrollIndicator = isMobile && weeks.length > 12

  return (
    <Tooltip.Provider delayDuration={0}>
      <div className="relative">
        {/* Mobile: horizontally scrollable container */}
        <div className={cn(
          'flex gap-1',
          isMobile && 'overflow-x-auto pb-2 -mx-1 px-1'
        )}>
          {displayWeeks.map((week, wi) => (
            <div key={wi} className="flex flex-col gap-1 shrink-0">
              {week.map((day) => (
                <HeatmapCell
                  key={day.date}
                  day={day}
                  isMobile={isMobile}
                  colorClass={getColor(day.count)}
                  onClick={() => handleDayClick(day.date)}
                />
              ))}
            </div>
          ))}
          {/* Legend - sticky on mobile scroll */}
          <div className={cn(
            'flex items-center gap-2 text-xs text-gray-400',
            isMobile ? 'sticky right-0 bg-white dark:bg-gray-900 pl-2' : 'ml-2'
          )}>
            <span>Less</span>
            <div className="flex gap-0.5">
              <div className={cn(isMobile ? 'w-4 h-4' : 'w-3 h-3', 'rounded-sm bg-gray-100 dark:bg-gray-800')} />
              <div className={cn(isMobile ? 'w-4 h-4' : 'w-3 h-3', 'rounded-sm bg-green-200 dark:bg-green-600')} />
              <div className={cn(isMobile ? 'w-4 h-4' : 'w-3 h-3', 'rounded-sm bg-green-300 dark:bg-green-400')} />
              <div className={cn(isMobile ? 'w-4 h-4' : 'w-3 h-3', 'rounded-sm bg-green-500')} />
            </div>
            <span>More</span>
          </div>
        </div>
        {/* Scroll indicator for mobile */}
        {showScrollIndicator && (
          <p className="text-xs text-gray-400 mt-1 text-center">
            Swipe to see more weeks
          </p>
        )}
      </div>
    </Tooltip.Provider>
  )
}
