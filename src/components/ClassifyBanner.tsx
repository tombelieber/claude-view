import { useState, useEffect } from 'react'
import { Sparkles, X, Loader2 } from 'lucide-react'
import { useClassification } from '../hooks/use-classification'

const CLASSIFY_COUNT_KEY = 'classify-single-count'
const BANNER_DISMISSED_KEY = 'classify-banner-dismissed'
const SHOW_AFTER_COUNT = 3

interface ClassifyBannerProps {
  unclassifiedCount: number
  estimatedCostCents: number
}

/**
 * Inline banner that appears after the user has classified 3+ sessions individually.
 * Prompts them to classify all remaining sessions with a clear cost estimate.
 */
export function ClassifyBanner({ unclassifiedCount, estimatedCostCents }: ClassifyBannerProps) {
  const [dismissed, setDismissed] = useState(() =>
    localStorage.getItem(BANNER_DISMISSED_KEY) === 'true'
  )
  const [singleCount, setSingleCount] = useState(() =>
    parseInt(localStorage.getItem(CLASSIFY_COUNT_KEY) || '0', 10)
  )
  const { startClassification, isLoading } = useClassification()
  const [isStarting, setIsStarting] = useState(false)

  // Listen for classify count changes via CustomEvent (instant, same-tab)
  // and StorageEvent (cross-tab fallback)
  useEffect(() => {
    const handleCustom = (e: Event) => {
      const count = (e as CustomEvent<number>).detail
      setSingleCount(count)
    }
    const handleStorage = (e: StorageEvent) => {
      if (e.key === CLASSIFY_COUNT_KEY && e.newValue) {
        setSingleCount(parseInt(e.newValue, 10))
      }
    }
    window.addEventListener('classify-single-done', handleCustom)
    window.addEventListener('storage', handleStorage)
    return () => {
      window.removeEventListener('classify-single-done', handleCustom)
      window.removeEventListener('storage', handleStorage)
    }
  }, [])

  // Don't show if: dismissed, not enough single classifies, no unclassified sessions
  if (dismissed || singleCount < SHOW_AFTER_COUNT || unclassifiedCount === 0) {
    return null
  }

  const costDisplay = estimatedCostCents < 1
    ? '<$0.01'
    : `~$${(estimatedCostCents / 100).toFixed(2)}`

  const handleClassifyAll = async () => {
    setIsStarting(true)
    await startClassification('unclassified')
    setIsStarting(false)
  }

  const handleDismiss = () => {
    setDismissed(true)
    localStorage.setItem(BANNER_DISMISSED_KEY, 'true')
  }

  return (
    <div className="flex items-center justify-between gap-3 px-4 py-2.5 bg-blue-50 dark:bg-blue-950/30 border border-blue-200 dark:border-blue-800 rounded-lg text-sm">
      <div className="flex items-center gap-2 text-blue-700 dark:text-blue-300">
        <Sparkles className="w-4 h-4 flex-shrink-0" />
        <span>
          <strong>{unclassifiedCount}</strong> sessions unclassified.
          Classify all ({costDisplay}, ~{Math.ceil(unclassifiedCount * 0.4)}s)
        </span>
      </div>
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={handleClassifyAll}
          disabled={isStarting || isLoading}
          className="px-3 py-1 text-xs font-medium text-white bg-blue-600 hover:bg-blue-700 disabled:opacity-50 rounded-md transition-colors"
        >
          {isStarting ? <Loader2 className="w-3 h-3 animate-spin" /> : 'Classify All'}
        </button>
        <button
          type="button"
          onClick={handleDismiss}
          className="p-0.5 text-blue-400 hover:text-blue-600 dark:text-blue-500 dark:hover:text-blue-300"
          aria-label="Dismiss"
        >
          <X className="w-4 h-4" />
        </button>
      </div>
    </div>
  )
}
