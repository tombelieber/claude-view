import { useState } from 'react'
import { Brain, ChevronRight } from 'lucide-react'
import { cn } from '../lib/utils'

interface ThinkingBlockProps {
  thinking: string
}

export function ThinkingBlock({ thinking }: ThinkingBlockProps) {
  const [isExpanded, setIsExpanded] = useState(false)

  // Generate a short preview: first ~80 chars, cut at word boundary
  const preview = thinking.length > 80
    ? thinking.slice(0, 80).replace(/\s+\S*$/, '') + '...'
    : thinking

  return (
    <div className="mb-3">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className={cn(
          'w-full text-left rounded-lg border transition-colors',
          'border-indigo-100 bg-indigo-50/50 hover:bg-indigo-50',
          isExpanded && 'rounded-b-none'
        )}
      >
        <div className="flex items-center gap-2 px-3 py-2">
          <Brain className="w-3.5 h-3.5 text-indigo-400 flex-shrink-0" />
          <span className="text-xs font-medium text-indigo-500">Thinking</span>
          <ChevronRight
            className={cn(
              'w-3 h-3 text-indigo-300 transition-transform',
              isExpanded && 'rotate-90'
            )}
          />
          {!isExpanded && (
            <span className="text-xs text-indigo-300 italic truncate">
              {preview}
            </span>
          )}
        </div>
      </button>
      {isExpanded && (
        <div className="px-4 py-3 text-sm text-indigo-700/70 italic leading-relaxed border border-t-0 border-indigo-100 rounded-b-lg bg-indigo-50/30 whitespace-pre-wrap">
          {thinking}
        </div>
      )}
    </div>
  )
}
