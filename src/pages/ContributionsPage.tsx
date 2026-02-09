import { useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { useContributions, type TimeRange } from '../hooks/use-contributions'
import { ContributionsHeader } from '../components/contributions/ContributionsHeader'
import { OverviewCards } from '../components/contributions/OverviewCards'
import { TrendChart } from '../components/contributions/TrendChart'
import { ContributionsEmptyState } from '../components/contributions/ContributionsEmptyState'
import { EfficiencyMetricsSection } from '../components/contributions/EfficiencyMetrics'
import { ModelComparison } from '../components/contributions/ModelComparison'
import { LearningCurve } from '../components/contributions/LearningCurve'
import { SkillEffectiveness } from '../components/contributions/SkillEffectiveness'
import { BranchList } from '../components/contributions/BranchList'
import { UncommittedWorkSection } from '../components/contributions/UncommittedWork'
import { WarningBanner } from '../components/contributions/WarningBanner'
import { SessionDrillDown } from '../components/contributions/SessionDrillDown'
import { DashboardSkeleton, ErrorState } from '../components/LoadingStates'
import { buildSessionUrl } from '../lib/url-utils'

/**
 * ContributionsPage - AI Contribution Tracking dashboard.
 *
 * Displays:
 * - Overview cards (Fluency, Output, Effectiveness)
 * - Contribution trend chart
 * - Efficiency metrics (ROI)
 * - Model comparison table
 * - Branch breakdown with expand/collapse
 * - Uncommitted work alerts
 * - Session drill-down (modal)
 * - Warnings for incomplete data
 */
export function ContributionsPage() {
  // Navigation + project/branch from query params
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const projectId = searchParams.get('project')
  const branchFilter = searchParams.get('branch') || undefined

  // URL-persisted time range
  const initialRange = (searchParams.get('range') as TimeRange) || 'week'
  const [range, setRange] = useState<TimeRange>(initialRange)

  // Session drill-down state
  const [drillDownSessionId, setDrillDownSessionId] = useState<string | null>(null)
  const [drillDownBranch, setDrillDownBranch] = useState<string | undefined>(undefined)

  // Fetch contributions data (with project + branch filter)
  const { data, isLoading, error, refetch } = useContributions(range, projectId ?? undefined, branchFilter)

  // Update URL when range changes (preserve existing params per CLAUDE.md rule)
  const handleRangeChange = (newRange: TimeRange) => {
    setRange(newRange)
    const params = new URLSearchParams(searchParams)
    params.set('range', newRange)
    setSearchParams(params)
  }

  // Handle branch filter (copy-then-modify per CLAUDE.md rule)
  const handleBranchFilter = (branch: string | null) => {
    const params = new URLSearchParams(searchParams)
    if (branch) {
      params.set('branch', branch)
    } else {
      params.delete('branch')
    }
    setSearchParams(params)
  }

  const handleClearBranchFilter = () => {
    const params = new URLSearchParams(searchParams)
    params.delete('branch')
    setSearchParams(params)
  }

  // Handle session drill-down
  const handleSessionDrillDown = (sessionId: string, branchName?: string) => {
    setDrillDownSessionId(sessionId)
    setDrillDownBranch(branchName)
  }

  const handleCloseDrillDown = () => {
    setDrillDownSessionId(null)
    setDrillDownBranch(undefined)
  }

  // Loading state
  if (isLoading) {
    return (
      <div className="h-full overflow-y-auto p-6">
        <div className="max-w-4xl mx-auto">
          <DashboardSkeleton />
        </div>
      </div>
    )
  }

  // Error state
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

  // Empty state (no sessions)
  if (!data || data.overview.fluency.sessions === 0) {
    return (
      <div className="h-full overflow-y-auto p-6">
        <div className="max-w-4xl mx-auto space-y-6">
          <ContributionsHeader
            range={range}
            onRangeChange={handleRangeChange}
            sessionCount={0}
            projectFilter={projectId}
            onClearProjectFilter={() => {
              const params = new URLSearchParams(searchParams)
              params.delete('project')
              params.delete('branch')
              setSearchParams(params)
            }}
            branchFilter={branchFilter}
            onClearBranchFilter={handleClearBranchFilter}
          />
          <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700">
            <ContributionsEmptyState range={range} onRangeChange={handleRangeChange} />
          </div>
        </div>
      </div>
    )
  }

  const sessionCount = data.overview.fluency.sessions

  return (
    <div className="h-full overflow-y-auto overflow-x-hidden p-6">
      <div className="max-w-4xl mx-auto space-y-6">
        {/* Header with time + project filter */}
        <ContributionsHeader
          range={range}
          onRangeChange={handleRangeChange}
          sessionCount={sessionCount}
          projectFilter={projectId}
          onClearProjectFilter={() => {
            const params = new URLSearchParams(searchParams)
            params.delete('project')
            params.delete('branch')
            setSearchParams(params)
          }}
          branchFilter={branchFilter}
          onClearBranchFilter={handleClearBranchFilter}
        />

        {/* Warnings */}
        {data.warnings.length > 0 && (
          <WarningBanner warnings={data.warnings} onSync={() => refetch()} />
        )}

        {/* Uncommitted Work Alerts (show prominently at top if any) */}
        {data.uncommitted.length > 0 && (
          <UncommittedWorkSection
            uncommitted={data.uncommitted}
            uncommittedInsight={data.uncommittedInsight}
            onRefresh={() => refetch()}
            onViewSession={(sessionId) => handleSessionDrillDown(sessionId)}
          />
        )}

        {/* Overview Cards (3 pillars) */}
        <OverviewCards overview={data.overview} />

        {/* Trend Chart */}
        <TrendChart
          data={data.trend}
          insight={generateTrendInsight(data.trend)}
        />

        {/* Efficiency Metrics (ROI) */}
        <EfficiencyMetricsSection
          efficiency={data.efficiency}
          trendData={data.trend}
          sessionCount={sessionCount}
          commitsCount={data.overview.output.commitsCount}
        />

        {/* Model Comparison Table */}
        <ModelComparison byModel={data.byModel} />

        {/* Learning Curve (re-edit rate over time) */}
        <LearningCurve data={data.learningCurve} />

        {/* Skill Effectiveness Table */}
        <SkillEffectiveness bySkill={data.bySkill} skillInsight={data.skillInsight} />

        {/* Branch Breakdown */}
        <BranchList
          byBranch={data.byBranch}
          onSessionDrillDown={(sessionId) => handleSessionDrillDown(sessionId)}
          projectId={projectId ?? undefined}
          timeRange={range}
          activeBranchFilter={branchFilter}
          onBranchFilter={handleBranchFilter}
        />
      </div>

      {/* Session Drill-Down Modal */}
      {drillDownSessionId && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50">
          <div className="w-full max-w-2xl max-h-[90vh] overflow-y-auto">
            <SessionDrillDown
              sessionId={drillDownSessionId}
              branchName={drillDownBranch}
              onClose={handleCloseDrillDown}
              onOpenFullSession={(sessionId) => {
                navigate(buildSessionUrl(sessionId, searchParams))
              }}
            />
          </div>
        </div>
      )}
    </div>
  )
}

/**
 * Generate trend insight from data.
 */
function generateTrendInsight(data: { linesAdded: number; date: string }[]) {
  if (data.length === 0) return undefined

  const totalAdded = data.reduce((sum, d) => sum + d.linesAdded, 0)
  const avgPerDay = totalAdded / data.length

  // Find peak day
  let peakDay = data[0]
  for (const day of data) {
    if (day.linesAdded > peakDay.linesAdded) {
      peakDay = day
    }
  }

  const peakDate = new Date(peakDay.date).toLocaleDateString('en-US', {
    weekday: 'long',
  })

  return {
    text: `${peakDate} was your most productive day with ${peakDay.linesAdded.toLocaleString()} lines added. Average: ${Math.round(avgPerDay).toLocaleString()} lines/day.`,
    kind: 'info' as const,
  }
}
