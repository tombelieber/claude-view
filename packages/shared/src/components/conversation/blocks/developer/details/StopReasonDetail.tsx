import { Square } from 'lucide-react'

export const RENDERED_KEYS = ['stopReason', 'preventedContinuation', 'hasOutput'] as const

interface StopReasonDetailProps {
  rawJson: Record<string, unknown> | null | undefined
}

export function StopReasonDetail({ rawJson }: StopReasonDetailProps) {
  if (!rawJson || typeof rawJson !== 'object' || Array.isArray(rawJson)) return null
  const stopReason = rawJson.stopReason as string | undefined
  const prevented = rawJson.preventedContinuation as boolean | undefined
  const hasOutput = rawJson.hasOutput as boolean | undefined
  if (stopReason == null && prevented == null && hasOutput == null) return null

  const isPrevented = prevented === true

  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs">
      <Square
        className={`w-3 h-3 ${isPrevented ? 'text-red-500 dark:text-red-400' : 'text-gray-500 dark:text-gray-400'}`}
      />
      {stopReason && (
        <span className="font-mono px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300">
          {stopReason}
        </span>
      )}
      {isPrevented && <span className="font-medium text-red-600 dark:text-red-400">prevented</span>}
      {hasOutput != null && (
        <span className="text-gray-500 dark:text-gray-400">output: {hasOutput ? 'yes' : 'no'}</span>
      )}
    </div>
  )
}
