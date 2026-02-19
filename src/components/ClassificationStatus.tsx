import { useState } from 'react'
import {
  Brain,
  Loader2,
  Settings2,
  CheckCircle2,
  AlertCircle,
  XCircle,
  FlaskConical,
} from 'lucide-react'
import { useClassification } from '../hooks/use-classification'
import { ClassificationProgress } from './ClassificationProgress'
import { cn } from '../lib/utils'
import { formatNumber } from '../lib/format-utils'

interface ClassificationStatusProps {
  onConfigure?: () => void
}

/**
 * Classification status card for the Settings/System page.
 *
 * Shows:
 * - Classification progress bar (sessions classified / total)
 * - Last run info (date, duration, cost, errors)
 * - Provider settings link
 * - Read-only hint to classify from Sessions list
 */
export function ClassificationStatus({ onConfigure }: ClassificationStatusProps) {
  const {
    status,
    error,
    sseProgress,
  } = useClassification()

  const [showProgress, setShowProgress] = useState(false)

  const isRunning = status?.status === 'running'

  const totalSessions = status?.totalSessions ?? 0
  const classifiedSessions = status?.classifiedSessions ?? 0
  const unclassifiedSessions = status?.unclassifiedSessions ?? 0
  const percentage = totalSessions > 0 ? (classifiedSessions / totalSessions) * 100 : 0

  // Use SSE progress when streaming, otherwise use status
  const currentProgress = sseProgress ?? status?.progress
  const displayPercentage = isRunning && currentProgress ? currentProgress.percentage : percentage
  const displayClassified = isRunning && currentProgress ? currentProgress.classified : classifiedSessions
  const displayTotal = isRunning && currentProgress ? currentProgress.total : totalSessions

  return (
    <>
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
          <div className="flex items-center gap-2">
            <Brain className="w-4 h-4 text-gray-500 dark:text-gray-400" />
            <h2 className="text-sm font-semibold text-gray-700 dark:text-gray-300 uppercase tracking-wide">
              Classification
            </h2>
            <span className="inline-flex items-center gap-0.5 px-1.5 py-0 text-[10px] font-medium rounded-full border border-amber-300 dark:border-amber-700 text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/40">
              <FlaskConical className="w-2.5 h-2.5" />
              Experimental
            </span>
          </div>
          {unclassifiedSessions > 0 && (
            <p className="text-sm text-gray-500 dark:text-gray-400">
              Classify sessions from the Sessions list.
            </p>
          )}
        </div>

        {/* Body */}
        <div className="p-4">
          {/* Status line */}
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2">
              {isRunning ? (
                <>
                  <Loader2 className="w-4 h-4 text-blue-500 animate-spin" />
                  <span className="text-sm font-medium text-blue-600 dark:text-blue-400">Classifying...</span>
                </>
              ) : status?.status === 'failed' ? (
                <>
                  <XCircle className="w-4 h-4 text-red-500" />
                  <span className="text-sm font-medium text-red-600 dark:text-red-400">Failed</span>
                </>
              ) : classifiedSessions === totalSessions && totalSessions > 0 ? (
                <>
                  <CheckCircle2 className="w-4 h-4 text-green-500" />
                  <span className="text-sm font-medium text-green-600 dark:text-green-400">All classified</span>
                </>
              ) : (
                <span className="text-sm text-gray-500 dark:text-gray-400">Ready</span>
              )}
            </div>
            {isRunning && currentProgress && (
              <span className="text-xs text-gray-500 dark:text-gray-400">
                ETA: {currentProgress.eta}
              </span>
            )}
          </div>

          {/* Progress bar */}
          <div className="mb-3">
            <div className="flex items-center justify-between mb-1">
              <span className="text-xs text-gray-500 dark:text-gray-400">
                Sessions classified
              </span>
              <span className="text-xs font-medium text-gray-700 dark:text-gray-300 tabular-nums">
                {formatNumber(displayClassified)} / {formatNumber(displayTotal)} ({displayPercentage.toFixed(1)}%)
              </span>
            </div>
            <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
              <div
                className={cn(
                  'h-2 rounded-full transition-all duration-300',
                  isRunning ? 'bg-blue-500' : 'bg-green-500'
                )}
                style={{ width: `${Math.min(displayPercentage, 100)}%` }}
              />
            </div>
          </div>

          {/* Error message */}
          {(error || status?.error) && (
            <div className="flex items-center gap-2 text-red-600 dark:text-red-400 mb-3 text-xs">
              <AlertCircle className="w-3.5 h-3.5 flex-shrink-0" />
              <span>{error || status?.error?.message}</span>
            </div>
          )}

          {/* Last run info */}
          {status?.lastRun && (
            <div className="text-xs text-gray-500 dark:text-gray-400 space-y-0.5">
              <div className="flex items-center justify-between">
                <span>Last run</span>
                <span className="tabular-nums">
                  {status.lastRun.completedAt &&
                  !status.lastRun.completedAt.startsWith('1970')
                    ? new Date(status.lastRun.completedAt).toLocaleDateString(undefined, {
                        month: 'short',
                        day: 'numeric',
                        hour: '2-digit',
                        minute: '2-digit',
                      })
                    : '--'}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span>Sessions classified</span>
                <span className="tabular-nums">{formatNumber(status.lastRun.sessionsClassified)}</span>
              </div>
              {status.lastRun.errorCount > 0 && (
                <div className="flex items-center justify-between text-amber-600 dark:text-amber-400">
                  <span>Errors</span>
                  <span className="tabular-nums">{status.lastRun.errorCount}</span>
                </div>
              )}
            </div>
          )}

          {/* Provider info */}
          <div className="flex items-center justify-between mt-3 pt-3 border-t border-gray-100 dark:border-gray-800">
            <span className="text-xs text-gray-500 dark:text-gray-400">
              Provider: Claude CLI (haiku)
            </span>
            {onConfigure && (
              <button
                type="button"
                onClick={onConfigure}
                className="inline-flex items-center gap-1 text-xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 cursor-pointer"
              >
                <Settings2 className="w-3 h-3" />
                Settings
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Progress modal */}
      {showProgress && (
        <ClassificationProgress
          isOpen={showProgress}
          onClose={() => setShowProgress(false)}
          onRunInBackground={() => setShowProgress(false)}
        />
      )}
    </>
  )
}
