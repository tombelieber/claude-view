import { ChevronRight } from 'lucide-react'
import { useState } from 'react'

interface RawEnvelopeDetailProps {
  rawJson: Record<string, unknown> | null | undefined
  renderedKeys?: readonly string[]
}

export function RawEnvelopeDetail({ rawJson, renderedKeys = [] }: RawEnvelopeDetailProps) {
  const [expanded, setExpanded] = useState(false)
  if (!rawJson || typeof rawJson !== 'object' || Array.isArray(rawJson)) return null

  const filtered = Object.fromEntries(
    Object.entries(rawJson).filter(([key]) => !renderedKeys.includes(key)),
  )
  const count = Object.keys(filtered).length
  if (count === 0) return null

  return (
    <div className="mt-1">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1 text-[10px] text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300"
      >
        <ChevronRight className={`w-3 h-3 transition-transform ${expanded ? 'rotate-90' : ''}`} />
        <span>Raw envelope ({count} fields)</span>
      </button>
      {expanded && (
        <pre className="mt-1 font-mono text-[10px] text-gray-600 dark:text-gray-400 whitespace-pre-wrap max-h-48 overflow-y-auto rounded border border-gray-200/50 dark:border-gray-700/50 p-2 bg-gray-50 dark:bg-gray-800/40">
          {JSON.stringify(filtered, null, 2)}
        </pre>
      )}
    </div>
  )
}
