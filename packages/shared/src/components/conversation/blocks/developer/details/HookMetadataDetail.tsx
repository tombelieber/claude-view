import { GitBranch } from 'lucide-react'

export const RENDERED_KEYS = ['hookInfos', 'hookErrors', 'hookCount'] as const

interface HookMetadataDetailProps {
  rawJson: Record<string, unknown> | null | undefined
}

export function HookMetadataDetail({ rawJson }: HookMetadataDetailProps) {
  if (!rawJson || typeof rawJson !== 'object' || Array.isArray(rawJson)) return null
  const hookInfos = rawJson.hookInfos as unknown[] | undefined
  const hookErrors = rawJson.hookErrors as unknown[] | undefined
  const hookCount = rawJson.hookCount as number | undefined
  if (!hookInfos && !hookErrors && hookCount == null) return null

  return (
    <div className="rounded border border-gray-200/50 dark:border-gray-700/50 overflow-hidden">
      <div className="flex items-center gap-2 px-2.5 py-1.5 bg-gray-50 dark:bg-gray-800/40 border-b border-gray-200/50 dark:border-gray-700/50">
        <GitBranch className="w-3 h-3 text-gray-500 dark:text-gray-400" />
        <span className="text-xs font-medium text-gray-600 dark:text-gray-300">
          Hooks ({hookCount ?? hookInfos?.length ?? 0})
        </span>
        {hookErrors && hookErrors.length > 0 && (
          <span className="text-xs font-medium text-red-500 dark:text-red-400">
            {hookErrors.length} error(s)
          </span>
        )}
      </div>
      <pre className="px-2.5 py-1.5 font-mono text-xs text-gray-600 dark:text-gray-400 whitespace-pre-wrap max-h-32 overflow-y-auto">
        {JSON.stringify({ hookInfos, hookErrors }, null, 2)}
      </pre>
    </div>
  )
}
