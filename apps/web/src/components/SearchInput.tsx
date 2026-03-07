import { Loader2, Search, X } from 'lucide-react'
import { forwardRef, useEffect, useRef } from 'react'
import type { IndexingPhase } from '../hooks/use-indexing-progress'

interface SearchInputProps {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  autoFocus?: boolean
  shortcutHint?: string
  matchInfo?: { current: number; total: number }
  onPrev?: () => void
  onNext?: () => void
  onClose?: () => void
  onKeyDown?: (e: React.KeyboardEvent) => void
  className?: string
  /** When set, shows indexing-aware placeholder and spinner icon. */
  indexingPhase?: IndexingPhase
  /** Percentage (0-100) shown in placeholder during deep-indexing. */
  indexingPercent?: number
}

function getIndexingPlaceholder(phase: IndexingPhase, percent: number): string {
  switch (phase) {
    case 'idle':
    case 'reading-indexes':
      return 'Preparing search...'
    case 'ready':
    case 'deep-indexing':
      return `Search (indexing ${percent}%)...`
    case 'finalizing':
      return 'Building search index...'
    default:
      return 'Search conversations...'
  }
}

export const SearchInput = forwardRef<HTMLInputElement, SearchInputProps>(function SearchInput(
  {
    value,
    onChange,
    placeholder = 'Search conversations...',
    autoFocus = false,
    shortcutHint,
    matchInfo,
    onPrev,
    onNext,
    onClose,
    onKeyDown,
    className = '',
    indexingPhase,
    indexingPercent = 0,
  },
  ref,
) {
  const internalRef = useRef<HTMLInputElement>(null)
  const inputRef = (ref as React.RefObject<HTMLInputElement>) || internalRef
  const isIndexing =
    indexingPhase &&
    indexingPhase !== 'done' &&
    indexingPhase !== 'error' &&
    indexingPhase !== 'idle'
  const resolvedPlaceholder = isIndexing
    ? getIndexingPlaceholder(indexingPhase, indexingPercent)
    : placeholder

  useEffect(() => {
    if (autoFocus) {
      inputRef.current?.focus()
    }
  }, [autoFocus, inputRef])

  return (
    <div className={`flex items-center gap-2 ${className}`}>
      {isIndexing ? (
        <Loader2 className="w-4 h-4 text-blue-400 dark:text-blue-500 animate-spin motion-reduce:animate-none flex-shrink-0" />
      ) : (
        <Search className="w-4 h-4 text-slate-400 dark:text-slate-500 flex-shrink-0" />
      )}
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={resolvedPlaceholder}
        aria-label={
          isIndexing
            ? 'Search conversations — indexing in progress, results may be limited'
            : undefined
        }
        onKeyDown={onKeyDown}
        className="flex-1 bg-transparent text-sm text-slate-900 dark:text-slate-100 placeholder:text-slate-400 dark:placeholder:text-slate-500 outline-none"
      />
      {matchInfo && matchInfo.total > 0 && (
        <div className="flex items-center gap-1 text-xs text-slate-500 dark:text-slate-400 flex-shrink-0">
          <span>
            {matchInfo.current} of {matchInfo.total}
          </span>
          {onPrev && (
            <button
              type="button"
              onClick={onPrev}
              className="p-0.5 hover:text-slate-700 dark:hover:text-slate-200"
              title="Previous match (Shift+Enter)"
            >
              &#9650;
            </button>
          )}
          {onNext && (
            <button
              type="button"
              onClick={onNext}
              className="p-0.5 hover:text-slate-700 dark:hover:text-slate-200"
              title="Next match (Enter)"
            >
              &#9660;
            </button>
          )}
        </div>
      )}
      {shortcutHint && !value && (
        <kbd className="text-xs text-slate-400 dark:text-slate-500 bg-slate-100 dark:bg-white/[0.06] px-1.5 py-0.5 rounded flex-shrink-0">
          {shortcutHint}
        </kbd>
      )}
      {value && onClose && (
        <button
          type="button"
          onClick={() => {
            onChange('')
            onClose?.()
          }}
          className="p-0.5 text-slate-400 hover:text-slate-600 dark:hover:text-slate-300 flex-shrink-0"
        >
          <X className="w-4 h-4" />
        </button>
      )}
    </div>
  )
})
