import { AlertCircle, FileCode2, Sparkles } from 'lucide-react'
import {
  type TimeRangeParams,
  formatTokens,
  useAIGenerationStats,
} from '../hooks/use-ai-generation'
import { useIsMobile } from '../hooks/use-media-query'
import { formatModelName } from '../lib/format-model'
import { CostBreakdownCard } from './CostBreakdownCard'
import { TokenBreakdown } from './TokenBreakdown'
import { ProgressBar } from './ui'

type ScopeValue = 'primary_sessions_only' | 'primary_plus_subagent_work'

type ScopeMeta = {
  dataScope?: {
    sessions?: ScopeValue
    workload?: ScopeValue
  }
  sessionBreakdown?: {
    primarySessions?: number
    sidechainSessions?: number
    otherSessions?: number
    totalObservedSessions?: number
  }
}

function scopeLabel(scope: ScopeValue | undefined): string {
  return scope === 'primary_plus_subagent_work'
    ? 'primary + subagent work'
    : 'primary sessions only'
}

function resolveSessionBreakdown(meta: ScopeMeta | undefined) {
  const primarySessions = meta?.sessionBreakdown?.primarySessions ?? 0
  const sidechainSessions = meta?.sessionBreakdown?.sidechainSessions ?? 0
  const otherSessions = meta?.sessionBreakdown?.otherSessions ?? 0
  const totalObservedSessions =
    meta?.sessionBreakdown?.totalObservedSessions ??
    primarySessions + sidechainSessions + otherSessions

  return {
    primarySessions,
    sidechainSessions,
    otherSessions,
    totalObservedSessions,
  }
}

interface AIGenerationStatsProps {
  /** Optional time range filter */
  timeRange?: TimeRangeParams | null
  /** Optional project filter */
  project?: string
  /** Optional branch filter */
  branch?: string
}

/**
 * AI Generation Breakdown component for the dashboard.
 *
 * Displays:
 * 1. Metric cards: Lines Generated, Files Edited, Tokens Used
 * 2. Token usage by model (progress bars)
 * 3. Top projects by token usage (progress bars)
 */
export function AIGenerationStats({ timeRange, project, branch }: AIGenerationStatsProps) {
  const {
    data: stats,
    isLoading,
    error,
    refetch,
  } = useAIGenerationStats(timeRange, project, branch)
  const isMobile = useIsMobile()

  if (isLoading) {
    return <AIGenerationStatsSkeleton />
  }

  if (error) {
    return (
      <div className="bg-white dark:bg-gray-900 rounded-xl border border-red-200 dark:border-red-800 p-4">
        <div className="flex items-center gap-2 text-red-500 text-sm">
          <AlertCircle className="w-4 h-4" />
          <span>Failed to load AI generation stats</span>
          <button onClick={() => refetch()} className="underline ml-2">
            Retry
          </button>
        </div>
      </div>
    )
  }

  if (!stats) {
    return null
  }

  // Calculate totals for progress bars
  const totalModelTokens = stats.tokensByModel.reduce(
    (sum, m) => sum + m.inputTokens + m.outputTokens,
    0,
  )
  const totalProjectTokens = stats.tokensByProject.reduce(
    (sum, p) => sum + p.inputTokens + p.outputTokens,
    0,
  )

  // Check if we have any meaningful data
  const hasTokenData = stats.totalInputTokens > 0 || stats.totalOutputTokens > 0
  const hasFileData = stats.filesCreated > 0
  const scopeMeta = stats.meta as ScopeMeta | undefined
  const sessionsScope = scopeLabel(scopeMeta?.dataScope?.sessions)
  const workloadScope = scopeLabel(scopeMeta?.dataScope?.workload)
  const sessionBreakdown = resolveSessionBreakdown(scopeMeta)

  // If no data at all, don't show the component
  if (!hasTokenData && !hasFileData) {
    return null
  }

  return (
    <div className="space-y-4 sm:space-y-6">
      <p className="text-xs text-gray-500 dark:text-gray-400">
        Session counts show {sessionsScope}. Workload metrics include {workloadScope}. Observed
        sessions: {sessionBreakdown.primarySessions.toLocaleString()} primary,{' '}
        {sessionBreakdown.sidechainSessions.toLocaleString()} sidechain,{' '}
        {sessionBreakdown.otherSessions.toLocaleString()} other,{' '}
        {sessionBreakdown.totalObservedSessions.toLocaleString()} total.
      </p>

      {/* Token Usage Breakdowns */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 sm:gap-6">
        {/* Token Usage by Model */}
        {stats.tokensByModel.length > 0 && (
          <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-4 sm:p-6">
            <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-3 sm:mb-4 flex items-center gap-1.5">
              <Sparkles className="w-4 h-4" />
              Token Usage by Model
            </h2>
            <div className={isMobile ? 'space-y-2' : 'space-y-1'}>
              {stats.tokensByModel.map((model) => {
                const modelTotal = model.inputTokens + model.outputTokens
                return (
                  <ProgressBar
                    key={model.model}
                    label={formatModelName(model.model)}
                    value={modelTotal}
                    max={totalModelTokens}
                    suffix={formatTokens(modelTotal)}
                    stacked={isMobile}
                  />
                )
              })}
            </div>
          </div>
        )}

        {/* Top Projects by Token Usage */}
        {stats.tokensByProject.length > 0 && (
          <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-4 sm:p-6">
            <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-3 sm:mb-4 flex items-center gap-1.5">
              <FileCode2 className="w-4 h-4" />
              Top Projects by Token Usage
            </h2>
            <div className={isMobile ? 'space-y-2' : 'space-y-1'}>
              {stats.tokensByProject.map((project) => {
                const projectTotal = project.inputTokens + project.outputTokens
                return (
                  <ProgressBar
                    key={project.project}
                    label={project.project}
                    value={projectTotal}
                    max={totalProjectTokens}
                    suffix={formatTokens(projectTotal)}
                    stacked={isMobile}
                  />
                )
              })}
            </div>
          </div>
        )}
      </div>

      {/* Token Breakdown (stacked bar + 4 detail cards) */}
      {hasTokenData && (
        <TokenBreakdown
          totalInputTokens={stats.totalInputTokens}
          totalOutputTokens={stats.totalOutputTokens}
          cacheReadTokens={stats.cacheReadTokens}
          cacheCreationTokens={stats.cacheCreationTokens}
        />
      )}

      {/* Cost Breakdown */}
      {stats.cost && stats.cost.totalCostUsd > 0 && <CostBreakdownCard cost={stats.cost} />}
    </div>
  )
}

/**
 * Skeleton loading state for AI Generation Stats.
 */
function AIGenerationStatsSkeleton() {
  return (
    <div className="space-y-6 animate-pulse">
      {/* Metric Cards Skeleton */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {[1, 2, 3].map((i) => (
          <div
            key={i}
            className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-4"
          >
            <div className="h-3 w-20 bg-gray-200 dark:bg-gray-700 rounded mb-3" />
            <div className="h-8 w-24 bg-gray-200 dark:bg-gray-700 rounded mb-2" />
            <div className="h-4 w-32 bg-gray-100 dark:bg-gray-800 rounded" />
          </div>
        ))}
      </div>

      {/* Progress Bars Skeleton */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {[1, 2].map((i) => (
          <div
            key={i}
            className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6"
          >
            <div className="h-4 w-40 bg-gray-200 dark:bg-gray-700 rounded mb-4" />
            <div className="space-y-3">
              {[1, 2, 3].map((j) => (
                <div key={j}>
                  <div className="flex justify-between mb-1">
                    <div className="h-4 w-24 bg-gray-200 dark:bg-gray-700 rounded" />
                    <div className="h-4 w-16 bg-gray-200 dark:bg-gray-700 rounded" />
                  </div>
                  <div className="h-2 w-full bg-gray-200 dark:bg-gray-700 rounded-full" />
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
