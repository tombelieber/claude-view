import { Brain } from 'lucide-react'

export const RENDERED_KEYS = ['thinkingMetadata'] as const

interface ThinkingMetadataDetailProps {
  rawJson: Record<string, unknown> | null | undefined
}

export function ThinkingMetadataDetail({ rawJson }: ThinkingMetadataDetailProps) {
  if (!rawJson || typeof rawJson !== 'object' || Array.isArray(rawJson)) return null
  const meta = rawJson.thinkingMetadata
  if (!meta) return null

  const obj = typeof meta === 'object' && meta !== null ? (meta as Record<string, unknown>) : null
  if (!obj) return null

  return (
    <div className="rounded border border-indigo-200/50 dark:border-indigo-700/50 overflow-hidden">
      <div className="flex items-center gap-2 px-2.5 py-1.5 bg-indigo-50 dark:bg-indigo-900/20 border-b border-indigo-200/50 dark:border-indigo-700/50">
        <Brain className="w-3 h-3 text-indigo-500 dark:text-indigo-400" />
        <span className="text-[10px] font-medium text-indigo-600 dark:text-indigo-300">
          Thinking Metadata
        </span>
      </div>
      <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 px-2.5 py-1.5 text-[10px]">
        {Object.entries(obj).map(([key, val]) => (
          <div key={key} className="contents">
            <div className="text-gray-500 dark:text-gray-400">{key}</div>
            <div className="font-mono text-gray-700 dark:text-gray-300 truncate">
              {typeof val === 'object' ? JSON.stringify(val) : String(val)}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
