import { Loader2, X, Minimize2 } from 'lucide-react'
import { useClassification } from '../hooks/use-classification'
import { cn } from '../lib/utils'
import { formatNumber } from '../lib/format-utils'

interface ClassificationProgressProps {
  isOpen: boolean
  onClose: () => void
  onRunInBackground: () => void
}

/**
 * Modal overlay showing detailed classification progress.
 *
 * Shows:
 * - Spinner animation
 * - Progress bar with percentage
 * - Time elapsed / remaining
 * - Cost info
 * - Current batch info
 * - Run in Background / Cancel buttons
 */
export function ClassificationProgress({
  isOpen,
  onClose,
  onRunInBackground,
}: ClassificationProgressProps) {
  const {
    status,
    sseProgress,
    cancelClassification,
  } = useClassification()

  if (!isOpen) return null

  const isRunning = status?.status === 'running'
  const isDone = status?.status === 'completed' || status?.status === 'cancelled' || status?.status === 'failed'

  const progress = sseProgress ?? status?.progress
  const classified = progress?.classified ?? 0
  const total = progress?.total ?? status?.unclassifiedSessions ?? 0
  const percentage = progress?.percentage ?? 0
  const eta = progress?.eta ?? 'calculating...'

  const handleCancel = async () => {
    await cancelClassification()
    onClose()
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/50"
        onClick={onRunInBackground}
      />

      {/* Modal */}
      <div className="relative bg-white dark:bg-gray-900 rounded-xl shadow-xl border border-gray-200 dark:border-gray-700 w-full max-w-md mx-4">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-100 dark:border-gray-800">
          <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
            Classification {isDone ? (status?.status === 'completed' ? 'Complete' : status?.status === 'cancelled' ? 'Cancelled' : 'Failed') : 'in Progress'}
          </h3>
          <button
            type="button"
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body */}
        <div className="p-5">
          {/* Spinner / Status */}
          <div className="flex flex-col items-center mb-5">
            {isRunning ? (
              <Loader2 className="w-8 h-8 text-blue-500 animate-spin mb-2" />
            ) : isDone && status?.status === 'completed' ? (
              <div className="w-8 h-8 rounded-full bg-green-100 dark:bg-green-900/30 flex items-center justify-center mb-2">
                <span className="text-green-600 dark:text-green-400 text-lg">&#10003;</span>
              </div>
            ) : isDone && status?.status === 'failed' ? (
              <div className="w-8 h-8 rounded-full bg-red-100 dark:bg-red-900/30 flex items-center justify-center mb-2">
                <span className="text-red-600 dark:text-red-400 text-lg">&#10007;</span>
              </div>
            ) : null}
            <p className="text-sm text-gray-600 dark:text-gray-300 text-center">
              {isRunning
                ? `Classifying ${formatNumber(total)} sessions with Claude haiku`
                : isDone && status?.status === 'completed'
                ? `Successfully classified ${formatNumber(classified)} sessions`
                : isDone && status?.status === 'cancelled'
                ? `Cancelled after classifying ${formatNumber(classified)} sessions`
                : isDone && status?.status === 'failed'
                ? `Classification failed: ${status?.error?.message ?? 'Unknown error'}`
                : 'Preparing classification...'}
            </p>
          </div>

          {/* Progress bar */}
          <div className="mb-4">
            <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-3">
              <div
                className={cn(
                  'h-3 rounded-full transition-all duration-300',
                  status?.status === 'failed'
                    ? 'bg-red-500'
                    : status?.status === 'cancelled'
                    ? 'bg-amber-500'
                    : 'bg-blue-500'
                )}
                style={{ width: `${Math.min(percentage, 100)}%` }}
              />
            </div>
            <div className="flex items-center justify-between mt-1.5">
              <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums">
                {formatNumber(classified)} / {formatNumber(total)}
              </span>
              <span className="text-xs font-medium text-gray-700 dark:text-gray-300 tabular-nums">
                {percentage.toFixed(1)}%
              </span>
            </div>
          </div>

          {/* Stats */}
          {isRunning && (
            <div className="grid grid-cols-2 gap-3 mb-4 text-xs">
              <div className="bg-gray-50 dark:bg-gray-800 rounded-lg p-3">
                <span className="text-gray-500 dark:text-gray-400 block mb-0.5">Estimated remaining</span>
                <span className="text-gray-900 dark:text-gray-100 font-medium tabular-nums">{eta}</span>
              </div>
              <div className="bg-gray-50 dark:bg-gray-800 rounded-lg p-3">
                <span className="text-gray-500 dark:text-gray-400 block mb-0.5">Current batch</span>
                <span className="text-gray-900 dark:text-gray-100 font-medium">
                  {progress?.currentBatch ?? 'Processing...'}
                </span>
              </div>
            </div>
          )}

          {/* Error details */}
          {status?.status === 'failed' && status?.error && (
            <div className="bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg p-3 mb-4">
              <p className="text-xs text-red-700 dark:text-red-300">{status.error.message}</p>
              {status.error.retryable && (
                <p className="text-xs text-red-500 dark:text-red-400 mt-1">
                  This error is retryable. You can try again.
                </p>
              )}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-gray-100 dark:border-gray-800">
          {isRunning && (
            <>
              <button
                type="button"
                onClick={onRunInBackground}
                className={cn(
                  'inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md cursor-pointer',
                  'border border-gray-300 dark:border-gray-600',
                  'text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800',
                  'transition-colors duration-150'
                )}
              >
                <Minimize2 className="w-3.5 h-3.5" />
                Run in Background
              </button>
              <button
                type="button"
                onClick={handleCancel}
                className={cn(
                  'inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md cursor-pointer',
                  'bg-red-600 text-white hover:bg-red-700',
                  'transition-colors duration-150'
                )}
              >
                Cancel
              </button>
            </>
          )}
          {isDone && (
            <button
              type="button"
              onClick={onClose}
              className={cn(
                'inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md cursor-pointer',
                'bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900',
                'hover:bg-gray-800 dark:hover:bg-gray-200',
                'transition-colors duration-150'
              )}
            >
              Close
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
