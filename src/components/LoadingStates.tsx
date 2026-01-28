import { AlertCircle, FolderOpen, Search, RefreshCw } from 'lucide-react'

/**
 * Skeleton loading component with animate-pulse and proper accessibility.
 * Shows a pulsing placeholder while content loads.
 */
interface SkeletonProps {
  /** Description of what is loading (for screen readers) */
  label: string
  /** Number of placeholder rows to show */
  rows?: number
  /** Whether to show a header skeleton */
  withHeader?: boolean
}

export function Skeleton({ label, rows = 3, withHeader = true }: SkeletonProps) {
  return (
    <div
      className="animate-pulse"
      role="status"
      aria-busy="true"
      aria-label={`Loading ${label}`}
    >
      <span className="sr-only">Loading {label}...</span>

      {withHeader && (
        <div className="mb-4">
          <div className="h-6 w-48 bg-gray-200 rounded" />
          <div className="h-4 w-32 bg-gray-100 rounded mt-2" />
        </div>
      )}

      <div className="space-y-3">
        {Array.from({ length: rows }).map((_, i) => (
          <div
            key={i}
            className="p-4 bg-white border border-gray-200 rounded-lg"
          >
            <div className="flex items-center justify-between mb-2">
              <div className="h-4 w-24 bg-gray-200 rounded" />
              <div className="h-4 w-16 bg-gray-100 rounded" />
            </div>
            <div className="h-4 w-full bg-gray-100 rounded mb-2" />
            <div className="h-4 w-3/4 bg-gray-50 rounded" />
          </div>
        ))}
      </div>
    </div>
  )
}

/**
 * Dashboard skeleton with metrics grid placeholder.
 */
export function DashboardSkeleton() {
  return (
    <div
      className="h-full overflow-y-auto p-6"
      role="status"
      aria-busy="true"
      aria-label="Loading dashboard"
    >
      <span className="sr-only">Loading dashboard...</span>
      <div className="max-w-4xl mx-auto space-y-6 animate-pulse">
        {/* Header card */}
        <div className="bg-white rounded-xl border border-gray-200 p-6">
          <div className="h-5 w-40 bg-gray-200 rounded mb-4" />
          <div className="flex items-center gap-6">
            <div className="h-8 w-24 bg-gray-100 rounded" />
            <div className="w-px h-8 bg-gray-200" />
            <div className="h-8 w-24 bg-gray-100 rounded" />
          </div>
        </div>

        {/* Stats grid */}
        <div className="grid md:grid-cols-2 gap-6">
          {[1, 2].map(i => (
            <div key={i} className="bg-white rounded-xl border border-gray-200 p-6">
              <div className="h-4 w-24 bg-gray-200 rounded mb-4" />
              <div className="space-y-3">
                {[1, 2, 3, 4, 5].map(j => (
                  <div key={j}>
                    <div className="flex justify-between mb-1">
                      <div className="h-4 w-20 bg-gray-100 rounded" />
                      <div className="h-4 w-8 bg-gray-50 rounded" />
                    </div>
                    <div className="h-2 w-full bg-gray-100 rounded-full" />
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>

        {/* Activity heatmap */}
        <div className="bg-white rounded-xl border border-gray-200 p-6">
          <div className="h-4 w-32 bg-gray-200 rounded mb-4" />
          <div className="flex gap-1">
            {Array.from({ length: 5 }).map((_, i) => (
              <div key={i} className="flex flex-col gap-1">
                {Array.from({ length: 7 }).map((_, j) => (
                  <div key={j} className="w-3 h-3 bg-gray-100 rounded-sm" />
                ))}
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}

/**
 * Empty state component with descriptive text and optional illustration.
 */
interface EmptyStateProps {
  /** Icon to display */
  icon?: React.ReactNode
  /** Main message */
  title: string
  /** Secondary description */
  description: string
  /** Optional action button */
  action?: {
    label: string
    onClick: () => void
  }
}

export function EmptyState({ icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-16 px-4 text-center">
      {icon && (
        <div className="inline-flex items-center justify-center w-14 h-14 rounded-full bg-gray-100 mb-4">
          {icon}
        </div>
      )}
      <h2 className="text-base font-semibold text-gray-900 mb-1">{title}</h2>
      <p className="text-sm text-gray-500 max-w-sm">{description}</p>
      {action && (
        <button
          type="button"
          onClick={action.onClick}
          className="mt-4 px-4 py-2 text-sm font-medium text-blue-600 hover:text-blue-700 hover:bg-blue-50 rounded-lg transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 cursor-pointer"
        >
          {action.label}
        </button>
      )}
    </div>
  )
}

/**
 * Error state component with retry functionality.
 */
interface ErrorStateProps {
  /** Error message to display */
  message: string
  /** Callback to retry the failed action */
  onRetry?: () => void
  /** Optional back navigation */
  onBack?: () => void
}

export function ErrorState({ message, onRetry, onBack }: ErrorStateProps) {
  return (
    <div
      className="flex flex-col items-center justify-center py-16 px-4 text-center"
      role="alert"
      aria-live="assertive"
    >
      <div className="inline-flex items-center justify-center w-14 h-14 rounded-full bg-red-100 mb-4">
        <AlertCircle className="w-6 h-6 text-red-600" aria-hidden="true" />
      </div>
      <h2 className="text-base font-semibold text-gray-900 mb-1">
        Something went wrong
      </h2>
      <p className="text-sm text-red-600 max-w-sm mb-4">{message}</p>
      <div className="flex items-center gap-3">
        {onBack && (
          <button
            type="button"
            onClick={onBack}
            className="px-4 py-2 text-sm font-medium text-gray-600 hover:text-gray-800 hover:bg-gray-100 rounded-lg transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-gray-400 cursor-pointer"
          >
            Go back
          </button>
        )}
        {onRetry && (
          <button
            type="button"
            onClick={onRetry}
            className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2 cursor-pointer"
          >
            <RefreshCw className="w-4 h-4" aria-hidden="true" />
            Try again
          </button>
        )}
      </div>
    </div>
  )
}

/**
 * Sessions empty state - specific for when no sessions are found.
 */
export function SessionsEmptyState({ isFiltered, onClearFilters }: {
  isFiltered: boolean
  onClearFilters?: () => void
}) {
  if (isFiltered) {
    return (
      <EmptyState
        icon={<Search className="w-6 h-6 text-gray-400" />}
        title="No sessions found"
        description="Try adjusting your filters or search terms to find what you're looking for."
        action={onClearFilters ? { label: 'Clear filters', onClick: onClearFilters } : undefined}
      />
    )
  }

  return (
    <EmptyState
      icon={<FolderOpen className="w-6 h-6 text-gray-400" />}
      title="No sessions yet"
      description="Start using Claude Code to see your session history here. Sessions will appear after your first conversation."
    />
  )
}

/**
 * Loading spinner with accessible label.
 */
export function LoadingSpinner({ label = 'Loading' }: { label?: string }) {
  return (
    <div
      className="flex items-center justify-center py-12"
      role="status"
      aria-busy="true"
      aria-label={label}
    >
      <span className="sr-only">{label}...</span>
      <svg
        className="animate-spin h-5 w-5 text-gray-400"
        xmlns="http://www.w3.org/2000/svg"
        fill="none"
        viewBox="0 0 24 24"
        aria-hidden="true"
      >
        <circle
          className="opacity-25"
          cx="12"
          cy="12"
          r="10"
          stroke="currentColor"
          strokeWidth="4"
        />
        <path
          className="opacity-75"
          fill="currentColor"
          d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
        />
      </svg>
    </div>
  )
}
