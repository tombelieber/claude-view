import { ChevronDown, ChevronRight } from 'lucide-react'
import { useState } from 'react'
import type { JsonTreeProps } from '../../../../contexts/DeveloperToolsContext'
import { SimpleJsonView } from '../developer/SimpleJsonView'

interface CollapsibleJsonProps {
  data: unknown
  label?: string
  defaultOpen?: boolean
}

export function CollapsibleJson({
  data,
  label = 'JSON',
  defaultOpen = false,
}: CollapsibleJsonProps) {
  const [open, setOpen] = useState(defaultOpen)

  return (
    <div className="text-xs font-mono">
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        className="inline-flex items-center gap-0.5 text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 transition-colors cursor-pointer"
      >
        {open ? (
          <ChevronDown className="w-3 h-3 flex-shrink-0" />
        ) : (
          <ChevronRight className="w-3 h-3 flex-shrink-0" />
        )}
        <span>{label}</span>
      </button>

      {open && (
        <div className="mt-1 max-h-64 overflow-y-auto rounded border border-gray-200/60 dark:border-gray-700/50 p-1.5 bg-gray-50/50 dark:bg-gray-800/30">
          <SimpleJsonView data={data} />
        </div>
      )}
    </div>
  )
}
