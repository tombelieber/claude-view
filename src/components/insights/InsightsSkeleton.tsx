/**
 * Full-page loading skeleton for the Insights page.
 * Matches the layout of the populated state to prevent layout shift.
 */
export function InsightsSkeleton() {
  return (
    <div className="animate-pulse space-y-6">
      {/* Hero skeleton */}
      <div className="bg-gradient-to-r from-blue-50 to-blue-100 dark:from-blue-900/20 dark:to-blue-800/20 rounded-xl border border-blue-200 dark:border-blue-800 p-6">
        <div className="h-3 w-24 bg-blue-200 dark:bg-blue-700 rounded mb-3" />
        <div className="h-6 w-3/4 bg-blue-200 dark:bg-blue-700 rounded mb-2" />
        <div className="h-4 w-full bg-blue-200 dark:bg-blue-700 rounded mb-1" />
        <div className="h-4 w-2/3 bg-blue-200 dark:bg-blue-700 rounded mb-4" />
        <div className="h-3 w-32 bg-blue-200 dark:bg-blue-700 rounded" />
      </div>

      {/* Stats row skeleton */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {[0, 1, 2].map((i) => (
          <div
            key={i}
            className="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-700 p-5"
          >
            <div className="h-3 w-20 bg-gray-200 dark:bg-gray-700 rounded mb-4" />
            <div className="h-8 w-16 bg-gray-200 dark:bg-gray-700 rounded mb-2" />
            <div className="h-3 w-24 bg-gray-200 dark:bg-gray-700 rounded mb-3" />
            <div className="h-3 w-32 bg-gray-200 dark:bg-gray-700 rounded" />
          </div>
        ))}
      </div>

      {/* Tab bar skeleton */}
      <div className="flex items-center gap-1 border-b border-gray-200 dark:border-gray-700 pb-0">
        <div className="h-4 w-16 bg-gray-200 dark:bg-gray-700 rounded mx-4 mb-2" />
        <div className="h-4 w-16 bg-gray-200 dark:bg-gray-700 rounded mx-4 mb-2" />
        <div className="h-4 w-20 bg-gray-200 dark:bg-gray-700 rounded mx-4 mb-2" />
        <div className="h-4 w-24 bg-gray-200 dark:bg-gray-700 rounded mx-4 mb-2" />
      </div>

      {/* Pattern cards skeleton */}
      <div className="space-y-3">
        {[0, 1, 2].map((i) => (
          <div
            key={i}
            className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-4"
          >
            <div className="flex items-center justify-between mb-2">
              <div className="h-3 w-20 bg-gray-200 dark:bg-gray-700 rounded" />
              <div className="h-2 w-24 bg-gray-200 dark:bg-gray-700 rounded" />
            </div>
            <div className="h-4 w-2/3 bg-gray-200 dark:bg-gray-700 rounded mb-2" />
            <div className="h-3 w-full bg-gray-200 dark:bg-gray-700 rounded mb-1" />
            <div className="h-3 w-3/4 bg-gray-200 dark:bg-gray-700 rounded" />
          </div>
        ))}
      </div>
    </div>
  )
}
