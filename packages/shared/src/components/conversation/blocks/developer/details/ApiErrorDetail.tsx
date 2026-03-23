import { AlertCircle } from 'lucide-react'

export const RENDERED_KEYS = ['apiError'] as const

interface ApiErrorDetailProps {
  rawJson: Record<string, unknown> | null | undefined
}

export function ApiErrorDetail({ rawJson }: ApiErrorDetailProps) {
  if (!rawJson || typeof rawJson !== 'object' || Array.isArray(rawJson)) return null
  const apiError = rawJson.apiError
  if (!apiError) return null

  const errorObj =
    typeof apiError === 'object' && apiError !== null ? (apiError as Record<string, unknown>) : null
  const message = errorObj?.message ?? errorObj?.error ?? String(apiError)

  return (
    <div className="flex items-start gap-2 px-3 py-2 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800/40">
      <AlertCircle className="w-4 h-4 text-red-500 dark:text-red-400 flex-shrink-0 mt-0.5" />
      <div>
        <div className="text-xs font-medium text-red-700 dark:text-red-300">API Error</div>
        <div className="text-[10px] text-red-600 dark:text-red-400 mt-0.5">
          {typeof message === 'string' ? message : JSON.stringify(message)}
        </div>
        {errorObj?.status != null && (
          <div className="text-[10px] font-mono text-red-500 dark:text-red-400 mt-0.5">
            Status: {String(errorObj.status)}
          </div>
        )}
      </div>
    </div>
  )
}
