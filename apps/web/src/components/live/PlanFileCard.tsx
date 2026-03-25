import { ChevronDown, ChevronRight, FileText } from 'lucide-react'
import { useState } from 'react'
import Markdown from 'react-markdown'
import rehypeRaw from 'rehype-raw'
import remarkGfm from 'remark-gfm'
import { markdownComponents } from '../../lib/markdown-components'
import { cn } from '../../lib/utils'
import type { PlanDocument } from '../../types/generated/PlanDocument'

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  return `${(bytes / 1024).toFixed(1)} KB`
}

interface PlanFileCardProps {
  plan: PlanDocument
  defaultExpanded?: boolean
}

export function PlanFileCard({ plan, defaultExpanded = false }: PlanFileCardProps) {
  const [expanded, setExpanded] = useState(defaultExpanded)

  const Icon = expanded ? ChevronDown : ChevronRight

  return (
    <div
      className={cn(
        'rounded-lg overflow-hidden transition-colors duration-150',
        expanded ? 'border border-gray-200 dark:border-gray-800' : '',
      )}
    >
      {/* Clickable header */}
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
        className={cn(
          'flex items-center gap-2 w-full px-3 py-2 text-left transition-colors cursor-pointer',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1',
          expanded
            ? 'bg-gray-50 dark:bg-gray-900/50 border-b border-gray-200 dark:border-gray-800'
            : 'hover:bg-gray-50 dark:hover:bg-gray-800/50 rounded-lg',
        )}
      >
        <Icon className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 flex-shrink-0" />
        <FileText className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 flex-shrink-0" />
        <span className="text-xs font-mono truncate flex-1 min-w-0 text-gray-800 dark:text-gray-200 font-medium">
          {plan.filename}
        </span>
        {plan.variant && (
          <span className="text-xs font-mono px-1.5 py-0.5 rounded bg-amber-50 dark:bg-amber-900/30 text-amber-600 dark:text-amber-400 flex-shrink-0">
            {plan.variant}
          </span>
        )}
        <span className="text-xs font-mono text-gray-400 dark:text-gray-500 flex-shrink-0">
          {formatBytes(plan.sizeBytes)}
        </span>
      </button>

      {/* Expanded content — rendered markdown */}
      {expanded && (
        <div className="p-4 overflow-y-auto max-h-[60vh]">
          <div className="prose prose-sm dark:prose-invert max-w-none">
            <Markdown
              remarkPlugins={[remarkGfm]}
              rehypePlugins={[rehypeRaw]}
              components={markdownComponents}
            >
              {plan.content}
            </Markdown>
          </div>
        </div>
      )}
    </div>
  )
}
