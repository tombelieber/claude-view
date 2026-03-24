import { RefreshCw } from 'lucide-react'

export const RENDERED_KEYS = ['retryInMs', 'retryAttempt', 'maxRetries'] as const

interface RetryDetailProps {
  rawJson: Record<string, unknown> | null | undefined
}

export function RetryDetail({ rawJson }: RetryDetailProps) {
  if (!rawJson || typeof rawJson !== 'object' || Array.isArray(rawJson)) return null
  const retryInMs = rawJson.retryInMs as number | undefined
  const retryAttempt = rawJson.retryAttempt as number | undefined
  const maxRetries = rawJson.maxRetries as number | undefined
  if (retryInMs == null && retryAttempt == null) return null

  return (
    <div className="flex items-center gap-2 px-3 py-1 text-[10px] rounded-lg bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800/40">
      <RefreshCw className="w-3 h-3 text-yellow-600 dark:text-yellow-400 flex-shrink-0" />
      <span className="text-yellow-700 dark:text-yellow-300">
        Retry{retryAttempt != null && maxRetries != null ? ` ${retryAttempt}/${maxRetries}` : ''}
        {retryInMs != null ? ` in ${retryInMs}ms` : ''}
      </span>
    </div>
  )
}
