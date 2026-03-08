import { useQuery } from '@tanstack/react-query'
import { ChevronDown, ChevronRight, Copy } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import type { PromptTemplateInfo } from '../../types/generated/PromptTemplateInfo'

const INITIAL_SHOW = 5

function highlightSlots(pattern: string) {
  const parts = pattern.split(/(<[^>]+>)/)
  return parts.map((part, i) =>
    part.startsWith('<') && part.endsWith('>') ? (
      <span key={i} className="text-blue-500 dark:text-blue-400 font-medium">
        {part}
      </span>
    ) : (
      part
    ),
  )
}

export function PromptTemplates() {
  const [collapsed, setCollapsed] = useState(false)
  const [showAll, setShowAll] = useState(false)

  const { data: templates, isLoading } = useQuery({
    queryKey: ['prompt-templates'],
    queryFn: () =>
      fetch('/api/prompts/templates').then((r) => r.json()) as Promise<Array<PromptTemplateInfo>>,
  })

  const displayTemplates =
    templates && !showAll ? templates.slice(0, INITIAL_SHOW) : (templates ?? [])
  const hasMore = (templates?.length ?? 0) > INITIAL_SHOW

  function handleCopy(pattern: string) {
    navigator.clipboard.writeText(pattern).then(() => {
      toast.success('Copied!')
    })
  }

  return (
    <div>
      {/* Section header */}
      <button
        type="button"
        onClick={() => setCollapsed((c) => !c)}
        className="flex items-center gap-1.5 w-full text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 transition-colors cursor-pointer"
      >
        {collapsed ? (
          <ChevronRight className="w-3.5 h-3.5 shrink-0" />
        ) : (
          <ChevronDown className="w-3.5 h-3.5 shrink-0" />
        )}
        <span>Prompt Templates</span>
        {templates && !collapsed && (
          <span className="ml-1 text-gray-400 dark:text-gray-500 font-normal normal-case tracking-normal">
            ({templates.length})
          </span>
        )}
      </button>

      {collapsed ? null : (
        <div className="mt-3">
          {isLoading || !templates ? (
            <p className="text-xs text-gray-400 dark:text-gray-500">Loading templates...</p>
          ) : templates.length === 0 ? (
            <p className="text-xs text-gray-400 dark:text-gray-500">No templates detected yet.</p>
          ) : (
            <div className="space-y-2">
              {displayTemplates.map((tpl, idx) => (
                <div
                  key={tpl.pattern}
                  className="flex items-start gap-2 p-2 rounded-lg border border-gray-200 dark:border-gray-700 text-sm"
                >
                  {/* Number */}
                  <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums mt-0.5 shrink-0">
                    {idx + 1}.
                  </span>

                  {/* Pattern + frequency */}
                  <div className="flex-1 min-w-0">
                    <p className="text-gray-800 dark:text-gray-200">
                      {highlightSlots(tpl.pattern)}
                    </p>
                    <span className="inline-block mt-1 px-1.5 py-0.5 rounded text-xs bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 tabular-nums">
                      {tpl.frequency}x
                    </span>
                  </div>

                  {/* Copy button */}
                  <button
                    type="button"
                    onClick={() => handleCopy(tpl.pattern)}
                    className="p-1 rounded text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors shrink-0"
                    title="Copy template"
                  >
                    <Copy className="w-3.5 h-3.5" />
                  </button>
                </div>
              ))}

              {/* Show all / collapse toggle */}
              {hasMore && (
                <button
                  type="button"
                  onClick={() => setShowAll((s) => !s)}
                  className="text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
                >
                  {showAll ? 'Show less' : `Show all ${templates.length}`}
                </button>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
