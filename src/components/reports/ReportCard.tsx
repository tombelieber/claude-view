import { Loader2, Sparkles, RefreshCw } from 'lucide-react'
import { cn } from '../../lib/utils'
import { formatCostUsd } from '../../lib/format-utils'
import { useReportPreview } from '../../hooks/use-report-preview'
import { useReportGenerate } from '../../hooks/use-report-generate'
import { ReportContent } from './ReportContent'
import { ReportDetails } from './ReportDetails'
import type { ReportRow } from '../../types/generated/ReportRow'

interface ReportCardProps {
  label: string
  dateStart: string
  dateEnd: string
  type: 'daily' | 'weekly'
  startTs: number
  endTs: number
  suggested?: boolean
  existingReport?: ReportRow
}

function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  return `${m}m`
}

function formatCost(cents: number): string {
  return formatCostUsd(cents / 100)
}

export function ReportCard({ label, dateStart, dateEnd, type, startTs, endTs, suggested, existingReport }: ReportCardProps) {
  const { data: preview, isLoading: previewLoading } = useReportPreview(startTs, endTs)
  const { generate, isGenerating, streamedText, contextDigest: completedContextDigest, generationModel, error, reset } = useReportGenerate()
  const handleGenerate = () => {
    generate({ reportType: type, dateStart, dateEnd, startTs, endTs })
  }

  const handleRedo = () => {
    reset()
    handleGenerate()
  }

  const dateLabel = dateStart === dateEnd
    ? dateStart
    : `${dateStart} \u2014 ${dateEnd}`

  // STREAMING state
  if (isGenerating && streamedText) {
    return (
      <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-5">
        <div className="flex items-center justify-between mb-3">
          <div>
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">{label}</h3>
            <p className="text-xs text-gray-500 dark:text-gray-400">{dateLabel}</p>
          </div>
        </div>
        <ReportContent markdown={streamedText} streaming />
        <button
          type="button"
          disabled
          className="mt-4 w-full flex items-center justify-center gap-2 px-4 py-2 text-sm font-medium rounded-md bg-gray-100 dark:bg-gray-800 text-gray-400 dark:text-gray-500 cursor-not-allowed"
        >
          <Loader2 className="w-4 h-4 animate-spin" />
          Generating...
        </button>
      </div>
    )
  }

  // COMPLETE state (just finished streaming, or showing existing)
  if (streamedText && !isGenerating) {
    return (
      <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-5">
        <div className="flex items-center justify-between mb-3">
          <div>
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">{label}</h3>
            <p className="text-xs text-gray-500 dark:text-gray-400">{dateLabel}</p>
          </div>
          <button
            type="button"
            onClick={handleRedo}
            className="p-1.5 rounded-md text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            title="Regenerate"
          >
            <RefreshCw className="w-4 h-4" />
          </button>
        </div>
        <ReportContent markdown={streamedText} />
        <ReportDetails
          contextDigestJson={completedContextDigest ?? existingReport?.contextDigest ?? null}
          totalCostCents={existingReport?.totalCostCents ?? 0}
          generationModel={generationModel ?? existingReport?.generationModel}
          generationInputTokens={existingReport?.generationInputTokens}
          generationOutputTokens={existingReport?.generationOutputTokens}
        />
      </div>
    )
  }

  // ERROR state (must precede EXISTING check — errors from regeneration
  // would otherwise be hidden behind the existing report)
  if (error) {
    return (
      <div className="rounded-lg border border-red-200 dark:border-red-800 bg-white dark:bg-gray-900 p-5">
        <div className="flex items-center justify-between mb-3">
          <div>
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">{label}</h3>
            <p className="text-xs text-gray-500 dark:text-gray-400">{dateLabel}</p>
          </div>
        </div>
        <p className="text-sm text-red-600 dark:text-red-400 mb-3">{error}</p>
        <button
          type="button"
          onClick={handleGenerate}
          className="w-full flex items-center justify-center gap-2 px-4 py-2 text-sm font-medium rounded-md bg-blue-500 text-white hover:bg-blue-600 transition-colors"
        >
          Try Again
        </button>
      </div>
    )
  }

  // Showing existing report (derived — no stale state)
  if (existingReport) {
    return (
      <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-5">
        <div className="flex items-center justify-between mb-3">
          <div>
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">{label}</h3>
            <p className="text-xs text-gray-500 dark:text-gray-400">{dateLabel}</p>
          </div>
          <button
            type="button"
            onClick={handleRedo}
            className="p-1.5 rounded-md text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            title="Regenerate"
          >
            <RefreshCw className="w-4 h-4" />
          </button>
        </div>
        <ReportContent markdown={existingReport.contentMd} />
        <ReportDetails
          contextDigestJson={existingReport.contextDigest ?? null}
          totalCostCents={existingReport.totalCostCents}
          generationModel={existingReport.generationModel}
          generationInputTokens={existingReport.generationInputTokens}
          generationOutputTokens={existingReport.generationOutputTokens}
        />
      </div>
    )
  }

  // PREVIEW state (default)
  const isEmpty = preview && preview.sessionCount === 0

  return (
    <div className={cn(
      'rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-5',
      suggested && 'border-l-2 border-l-blue-500 dark:border-l-blue-400'
    )}>
      <div className="mb-3">
        <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">{label}</h3>
        <p className="text-xs text-gray-500 dark:text-gray-400">{dateLabel}</p>
      </div>

      {previewLoading ? (
        <div className="h-6 bg-gray-100 dark:bg-gray-800 rounded animate-pulse mb-4" />
      ) : isEmpty ? (
        <p className="text-sm text-gray-400 dark:text-gray-500 mb-4">No sessions in this range</p>
      ) : preview ? (
        <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
          {preview.sessionCount} sessions &middot; {preview.projectCount} projects &middot; {formatDuration(preview.totalDurationSecs)}
          {preview.totalCostCents > 0 && (
            <span title="Estimated API cost for sessions in this period">
              {' \u00B7 ~'}{formatCost(preview.totalCostCents)}{' API usage'}
            </span>
          )}
        </p>
      ) : null}

      <button
        type="button"
        onClick={handleGenerate}
        disabled={isGenerating || isEmpty || previewLoading}
        className={cn(
          'w-full flex items-center justify-center gap-2 px-4 py-2 text-sm font-medium rounded-md transition-colors',
          isGenerating || isEmpty || previewLoading
            ? 'bg-gray-100 dark:bg-gray-800 text-gray-400 dark:text-gray-500 cursor-not-allowed'
            : 'bg-blue-500 text-white hover:bg-blue-600'
        )}
      >
        {isGenerating ? (
          <>
            <Loader2 className="w-4 h-4 animate-spin" />
            Generating...
          </>
        ) : (
          <>
            <Sparkles className="w-4 h-4" />
            Generate Report
          </>
        )}
      </button>
    </div>
  )
}
