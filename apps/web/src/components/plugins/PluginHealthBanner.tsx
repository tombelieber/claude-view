import { AlertTriangle, WifiOff } from 'lucide-react'

interface PluginHealthBannerProps {
  duplicateCount: number
  unusedCount: number
  cliError: string | null
  onCleanup?: () => void
}

export function PluginHealthBanner({
  duplicateCount,
  unusedCount,
  cliError,
  onCleanup,
}: PluginHealthBannerProps) {
  if (cliError) {
    return (
      <div className="mx-6 mb-3 rounded-lg border border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20 px-4 py-2 flex items-center gap-2">
        <WifiOff className="w-4 h-4 text-red-500 dark:text-red-400 flex-shrink-0" />
        <span className="text-sm text-red-700 dark:text-red-300">CLI unavailable: {cliError}</span>
      </div>
    )
  }

  if (duplicateCount === 0 && unusedCount === 0) return null

  const parts: string[] = []
  if (duplicateCount > 0) parts.push(`${duplicateCount} duplicate${duplicateCount > 1 ? 's' : ''}`)
  if (unusedCount > 0) parts.push(`${unusedCount} unused in 30 days`)

  return (
    <div className="mx-6 mb-3 rounded-lg border border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/20 px-4 py-2 flex items-center justify-between">
      <div className="flex items-center gap-2">
        <AlertTriangle className="w-4 h-4 text-amber-500 dark:text-amber-400 flex-shrink-0" />
        <span className="text-sm text-amber-700 dark:text-amber-300">{parts.join(' \u00b7 ')}</span>
      </div>
      {onCleanup && (
        <button
          type="button"
          onClick={onCleanup}
          className="text-xs font-medium text-amber-700 dark:text-amber-300 hover:underline"
        >
          Cleanup &rarr;
        </button>
      )}
    </div>
  )
}
