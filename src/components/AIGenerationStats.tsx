import { Sparkles, FileCode2, Coins } from 'lucide-react'
import { useAIGenerationStats, formatTokens, formatLineCount, type TimeRangeParams } from '../hooks/use-ai-generation'
import { MetricCard } from './ui/MetricCard'
import { ProgressBar } from './ui/ProgressBar'

interface AIGenerationStatsProps {
  /** Optional time range filter */
  timeRange?: TimeRangeParams | null
}

/**
 * AI Generation Breakdown component for the dashboard.
 *
 * Displays:
 * 1. Metric cards: Lines Generated, Files Created, Tokens Used
 * 2. Token usage by model (progress bars)
 * 3. Top projects by token usage (progress bars)
 */
export function AIGenerationStats({ timeRange }: AIGenerationStatsProps) {
  const { data: stats, isLoading, error } = useAIGenerationStats(timeRange)

  if (isLoading) {
    return <AIGenerationStatsSkeleton />
  }

  if (error || !stats) {
    return null // Silently hide if error or no data
  }

  // Calculate totals for progress bars
  const totalModelTokens = stats.tokensByModel.reduce(
    (sum, m) => sum + m.inputTokens + m.outputTokens,
    0
  )
  const totalProjectTokens = stats.tokensByProject.reduce(
    (sum, p) => sum + p.inputTokens + p.outputTokens,
    0
  )

  // Calculate net lines
  const netLines = stats.linesAdded - stats.linesRemoved

  // Check if we have any meaningful data
  const hasTokenData = stats.totalInputTokens > 0 || stats.totalOutputTokens > 0
  const hasFileData = stats.filesCreated > 0

  // If no data at all, don't show the component
  if (!hasTokenData && !hasFileData) {
    return null
  }

  return (
    <div className="space-y-6">
      {/* Metric Cards Row */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {/* Lines Generated Card */}
        <MetricCard
          label="Lines Generated"
          value={formatLineCount(stats.linesAdded)}
          subValue={stats.linesRemoved > 0 ? `${formatLineCount(-stats.linesRemoved, false)} removed` : undefined}
          footer={netLines !== stats.linesAdded ? `net: ${formatLineCount(netLines)}` : undefined}
        />

        {/* Files Created Card */}
        <MetricCard
          label="Files Created"
          value={stats.filesCreated.toLocaleString()}
          subValue="written by AI"
        />

        {/* Tokens Used Card */}
        <MetricCard
          label="Tokens Used"
          value={formatTokens(stats.totalInputTokens + stats.totalOutputTokens)}
          subValue={`input: ${formatTokens(stats.totalInputTokens)}`}
          footer={`output: ${formatTokens(stats.totalOutputTokens)}`}
        />
      </div>

      {/* Token Usage Breakdowns */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Token Usage by Model */}
        {stats.tokensByModel.length > 0 && (
          <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
            <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-1.5">
              <Sparkles className="w-4 h-4" />
              Token Usage by Model
            </h2>
            <div className="space-y-1">
              {stats.tokensByModel.map((model) => {
                const modelTotal = model.inputTokens + model.outputTokens
                return (
                  <ProgressBar
                    key={model.model}
                    label={formatModelName(model.model)}
                    value={modelTotal}
                    max={totalModelTokens}
                    suffix={formatTokens(modelTotal)}
                  />
                )
              })}
            </div>
          </div>
        )}

        {/* Top Projects by Token Usage */}
        {stats.tokensByProject.length > 0 && (
          <div className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
            <h2 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-1.5">
              <FileCode2 className="w-4 h-4" />
              Top Projects by Token Usage
            </h2>
            <div className="space-y-1">
              {stats.tokensByProject.map((project) => {
                const projectTotal = project.inputTokens + project.outputTokens
                return (
                  <ProgressBar
                    key={project.project}
                    label={project.project}
                    value={projectTotal}
                    max={totalProjectTokens}
                    suffix={formatTokens(projectTotal)}
                  />
                )
              })}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

/**
 * Format model name to be more readable.
 * e.g., "claude-3-5-sonnet-20241022" -> "Claude 3.5 Sonnet"
 */
function formatModelName(modelId: string): string {
  // Map common model IDs to friendly names
  const modelMap: Record<string, string> = {
    'claude-opus-4-5-20251101': 'Claude Opus 4.5',
    'claude-opus-4-20250514': 'Claude Opus 4',
    'claude-sonnet-4-20250514': 'Claude Sonnet 4',
    'claude-3-5-sonnet-20241022': 'Claude 3.5 Sonnet',
    'claude-3-5-haiku-20241022': 'Claude 3.5 Haiku',
    'claude-3-opus-20240229': 'Claude 3 Opus',
    'claude-3-sonnet-20240229': 'Claude 3 Sonnet',
    'claude-3-haiku-20240307': 'Claude 3 Haiku',
  }

  if (modelMap[modelId]) {
    return modelMap[modelId]
  }

  // Try to parse model name from ID
  // e.g., "claude-3-5-sonnet-20241022" -> "claude-3-5-sonnet"
  const parts = modelId.split('-')
  if (parts.length >= 3 && parts[0] === 'claude') {
    // Remove date suffix if present (8 digits)
    if (parts[parts.length - 1].match(/^\d{8}$/)) {
      parts.pop()
    }
    // Capitalize and format
    return parts
      .map((p, i) => {
        if (i === 0) return 'Claude'
        if (p === '3' || p === '4' || p === '5') return p
        return p.charAt(0).toUpperCase() + p.slice(1)
      })
      .join(' ')
      .replace(' 3 5 ', ' 3.5 ')
      .replace(' 4 5 ', ' 4.5 ')
  }

  return modelId
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
