import { ChevronRight, Network } from 'lucide-react'
import { useState } from 'react'

export const RENDERED_KEYS = [
  'parentUuid',
  'logicalParentUuid',
  'isSidechain',
  'agentId',
  'sourceToolUseID',
  'sourceToolAssistantUUID',
  'messageId',
  'sessionId',
  'uuid',
] as const

interface MessageLineageDetailProps {
  rawJson: Record<string, unknown> | null | undefined
}

function truncUuid(val: unknown): string {
  if (typeof val !== 'string') return String(val ?? '')
  return val.length > 8 ? `${val.slice(0, 8)}…` : val
}

export function MessageLineageDetail({ rawJson }: MessageLineageDetailProps) {
  const [expanded, setExpanded] = useState(false)
  if (!rawJson || typeof rawJson !== 'object' || Array.isArray(rawJson)) return null

  const entries = RENDERED_KEYS.filter((key) => rawJson[key] != null).map(
    (key) => [key, rawJson[key]] as const,
  )
  if (entries.length === 0) return null

  return (
    <div className="mt-1">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1 text-[10px] text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300"
      >
        <ChevronRight className={`w-3 h-3 transition-transform ${expanded ? 'rotate-90' : ''}`} />
        <Network className="w-3 h-3" />
        <span>Message lineage ({entries.length})</span>
      </button>
      {expanded && (
        <div className="mt-1 grid grid-cols-2 gap-x-4 gap-y-0.5 px-2.5 py-1.5 text-[10px] rounded border border-gray-200/50 dark:border-gray-700/50 bg-gray-50 dark:bg-gray-800/40">
          {entries.map(([key, val]) => (
            <div key={key} className="contents">
              <div className="text-gray-500 dark:text-gray-400">{key}</div>
              <div
                className="font-mono text-gray-700 dark:text-gray-300 truncate"
                title={String(val)}
              >
                {typeof val === 'boolean' ? String(val) : truncUuid(val)}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
