import { Brain, ChevronDown, ChevronRight } from 'lucide-react'
import { useState } from 'react'

interface ThinkingBlockProps {
  content: string
  defaultExpanded?: boolean
}

export function ThinkingBlock({ content, defaultExpanded = false }: ThinkingBlockProps) {
  const [expanded, setExpanded] = useState(defaultExpanded)
  const preview = content.slice(0, 120).replace(/\n/g, ' ')

  return (
    <div className="border-l-2 border-purple-300 dark:border-purple-700 pl-3 py-1.5">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 text-xs text-purple-600 dark:text-purple-400 hover:text-purple-700 dark:hover:text-purple-300 cursor-pointer"
      >
        <Brain className="w-3.5 h-3.5" />
        <span className="font-medium">Thinking</span>
        {expanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
      </button>
      {expanded ? (
        <p className="mt-1.5 text-xs text-gray-600 dark:text-gray-400 italic whitespace-pre-wrap">
          {content}
        </p>
      ) : (
        <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-500 truncate">{preview}...</p>
      )}
    </div>
  )
}
