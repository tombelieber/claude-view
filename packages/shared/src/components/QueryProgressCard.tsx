import { Database } from 'lucide-react'
import { useCompactCodeBlock } from '../contexts/CodeRenderContext'

/**
 * QueryProgressCard — info-rich, SQL code block.
 *
 * Schema: query
 * Design: icon + header, query in SQL code block.
 */

interface QueryProgressCardProps {
  query: string
  blockId?: string
}

export function QueryProgressCard({ query, blockId }: QueryProgressCardProps) {
  const CompactCodeBlock = useCompactCodeBlock()

  return (
    <div className="py-0.5 border-l-2 border-l-teal-400 pl-1 my-1" aria-label="Query progress">
      <div className="flex items-center gap-1.5 mb-0.5">
        <Database className="w-3 h-3 text-teal-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400">Query</span>
      </div>
      <CompactCodeBlock
        code={query}
        language="sql"
        blockId={blockId ? `${blockId}-query` : undefined}
      />
    </div>
  )
}
