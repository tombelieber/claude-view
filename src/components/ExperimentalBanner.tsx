import { useState } from 'react'
import { FlaskConical, X } from 'lucide-react'

const DISMISSED_KEY = 'experimental-insights-banner-dismissed'

interface ExperimentalBannerProps {
  /** localStorage key suffix for dismiss state */
  storageKey?: string
}

/**
 * Dismissible amber banner for the top of experimental pages.
 * Warns users that AI-powered insights are early-stage and may be inaccurate.
 */
export function ExperimentalBanner({ storageKey = DISMISSED_KEY }: ExperimentalBannerProps) {
  const [dismissed, setDismissed] = useState(() =>
    localStorage.getItem(storageKey) === 'true'
  )

  if (dismissed) return null

  const handleDismiss = () => {
    setDismissed(true)
    localStorage.setItem(storageKey, 'true')
  }

  return (
    <div className="flex items-start gap-3 px-4 py-3 mb-4 rounded-lg border border-amber-200 dark:border-amber-800/60 bg-amber-50/80 dark:bg-amber-950/30">
      <FlaskConical className="w-4 h-4 mt-0.5 text-amber-600 dark:text-amber-400 flex-shrink-0" />
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-amber-800 dark:text-amber-300">
          Experimental Feature
        </p>
        <p className="text-xs text-amber-700/80 dark:text-amber-400/70 mt-0.5">
          AI-powered insights and session classification are early-stage. Results may be inaccurate, incomplete, or change as the feature matures. Use as a rough guide, not ground truth.
        </p>
      </div>
      <button
        type="button"
        onClick={handleDismiss}
        className="p-0.5 text-amber-400 hover:text-amber-600 dark:text-amber-500 dark:hover:text-amber-300 flex-shrink-0 cursor-pointer"
        aria-label="Dismiss experimental notice"
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  )
}
