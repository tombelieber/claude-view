import { useState, useCallback } from 'react'
import { Loader2, Sparkles, AlertCircle } from 'lucide-react'
import { cn } from '../lib/utils'
import { useClassifySingle } from '../hooks/use-classify-single'

interface ClassifyButtonProps {
  sessionId: string
  className?: string
  /** Compact mode for table cells — icon only */
  compact?: boolean
}

/**
 * Small inline button to classify a single session.
 * Shows spinner while classifying, error state on failure,
 * morphs into CategoryBadge on success (via cache update).
 */
export function ClassifyButton({ sessionId, className, compact }: ClassifyButtonProps) {
  const { classifyingId, classifySession } = useClassifySingle()
  const isClassifying = classifyingId === sessionId
  const [lastError, setLastError] = useState<string | null>(null)

  const hasError = lastError !== null

  const handleClick = useCallback(async (e: React.MouseEvent) => {
    // Stop the parent <Link> from navigating
    e.preventDefault()
    e.stopPropagation()
    // Also stop native propagation for react-router
    e.nativeEvent.stopImmediatePropagation()

    console.log('[ClassifyButton] clicked', sessionId)
    setLastError(null)

    const result = await classifySession(sessionId)
    if (!result) {
      // classifySession returns null on error — read the error from the hook's state
      // won't be available synchronously, so use a generic message
      setLastError('Classification failed — check server logs')
      console.error('[ClassifyButton] classify failed for', sessionId)
    } else {
      console.log('[ClassifyButton] classify success', sessionId, result)
    }
  }, [sessionId, classifySession])

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={isClassifying}
      className={cn(
        'inline-flex items-center gap-1 px-1.5 py-0.5 text-xs rounded border transition-colors',
        hasError
          ? 'border-red-300 dark:border-red-800 text-red-500 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950/30'
          : 'border-gray-200 dark:border-gray-700 text-gray-400 dark:text-gray-500 hover:border-blue-300 hover:text-blue-600 hover:bg-blue-50 dark:hover:border-blue-700 dark:hover:text-blue-400 dark:hover:bg-blue-950/30',
        'disabled:opacity-50 disabled:cursor-wait',
        className,
      )}
      title={hasError ? `Failed: ${lastError}. Click to retry.` : 'Classify this session with AI (~5s) — experimental, may be inaccurate'}
    >
      {isClassifying ? (
        <Loader2 className="w-3 h-3 animate-spin" />
      ) : hasError ? (
        <AlertCircle className="w-3 h-3" />
      ) : (
        <Sparkles className="w-3 h-3" />
      )}
      {!compact && (
        <span>
          {isClassifying ? 'Classifying…' : hasError ? 'Retry' : 'Classify'}
        </span>
      )}
    </button>
  )
}
