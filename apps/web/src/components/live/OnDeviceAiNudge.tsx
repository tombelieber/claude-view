import { useQuery } from '@tanstack/react-query'
import { X } from 'lucide-react'
import { useCallback, useState } from 'react'
import { Link } from 'react-router-dom'

const DISMISS_KEY = 'claude-view:on-device-ai-nudge-dismissed'

interface NudgeStatus {
  enabled: boolean
  omlx_available: boolean
}

export function OnDeviceAiNudge() {
  const [dismissed, setDismissed] = useState(() => {
    try {
      return localStorage.getItem(DISMISS_KEY) === '1'
    } catch {
      return false
    }
  })

  const { data: status } = useQuery<NudgeStatus>({
    queryKey: ['local-llm-status'],
    queryFn: async () => {
      const res = await fetch('/api/local-llm/status')
      if (!res.ok) throw new Error('status fetch failed')
      return res.json()
    },
    refetchInterval: 30_000,
    staleTime: 15_000,
    retry: 1,
  })

  const handleDismiss = useCallback(() => {
    setDismissed(true)
    try {
      localStorage.setItem(DISMISS_KEY, '1')
    } catch {
      // localStorage unavailable
    }
  }, [])

  if (dismissed || !status || status.enabled) return null

  return (
    <div className="flex items-center gap-3 px-3 py-2 rounded-md bg-gray-50 dark:bg-gray-800/50 text-xs text-gray-500 dark:text-gray-400">
      <span className="flex-1">
        Phase detection available — classify sessions automatically with{' '}
        <Link
          to="/settings"
          className="font-medium text-gray-700 dark:text-gray-200 underline decoration-gray-300 dark:decoration-gray-600 underline-offset-2 hover:text-gray-900 dark:hover:text-gray-100 transition-colors"
        >
          On-Device AI
        </Link>
      </span>
      <button
        type="button"
        onClick={handleDismiss}
        className="shrink-0 p-0.5 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors cursor-pointer"
        title="Dismiss"
      >
        <X className="w-3.5 h-3.5" />
      </button>
    </div>
  )
}
