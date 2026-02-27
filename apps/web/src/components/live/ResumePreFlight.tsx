import * as Dialog from '@radix-ui/react-dialog'
import { useMutation, useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import type { CostEstimate, ResumeResponse } from '../../types/control'

interface ResumePreFlightProps {
  sessionId: string
  open: boolean
  onOpenChange: (open: boolean) => void
  onResume: (controlId: string, sessionId: string) => void
}

const MODEL_OPTIONS = [
  { value: 'sonnet-4', label: 'Claude Sonnet 4' },
  { value: 'opus-4', label: 'Claude Opus 4' },
  { value: 'haiku-4', label: 'Claude Haiku 4' },
] as const

export function ResumePreFlight({ sessionId, open, onOpenChange, onResume }: ResumePreFlightProps) {
  const [model, setModel] = useState('sonnet-4')

  const estimate = useQuery({
    queryKey: ['cost-estimate', sessionId, model],
    queryFn: async (): Promise<CostEstimate> => {
      const res = await fetch('/api/control/estimate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ session_id: sessionId, model }),
      })
      if (!res.ok) throw new Error('Failed to fetch cost estimate')
      return res.json()
    },
    enabled: open && !!sessionId,
  })

  const resumeMutation = useMutation({
    mutationFn: async (): Promise<ResumeResponse> => {
      const res = await fetch('/api/control/resume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          sessionId,
          model,
          projectPath: estimate.data?.project_name ?? '',
        }),
      })
      if (!res.ok) {
        const err = await res.json().catch(() => ({ error: 'Resume failed' }))
        throw new Error(err.error || 'Resume failed')
      }
      return res.json()
    },
    onSuccess: (data) => {
      onOpenChange(false)
      onResume(data.controlId, sessionId)
    },
  })

  const data = estimate.data

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 dark:bg-black/70" />
        <Dialog.Content className="fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 w-full max-w-md bg-white dark:bg-gray-900 rounded-xl shadow-xl p-6 focus:outline-none">
          <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Resume Session
          </Dialog.Title>
          <Dialog.Description className="text-sm text-gray-500 dark:text-gray-400 mt-1">
            Continue this Claude Code session from the dashboard.
          </Dialog.Description>

          {estimate.isLoading && (
            <div className="mt-4 text-sm text-gray-500 dark:text-gray-400">
              Loading cost estimate...
            </div>
          )}

          {estimate.error && (
            <div className="mt-4 text-sm text-red-600 dark:text-red-400">
              Failed to load estimate. {(estimate.error as Error).message}
            </div>
          )}

          {data && (
            <div className="mt-4 space-y-4">
              {/* Session Info */}
              <div className="space-y-1">
                {data.session_title && (
                  <p className="text-sm font-medium text-gray-900 dark:text-gray-100 line-clamp-2">
                    {data.session_title}
                  </p>
                )}
                {data.project_name && (
                  <p className="text-xs text-gray-500 dark:text-gray-400">{data.project_name}</p>
                )}
                <div className="flex items-center gap-3 text-xs text-gray-500 dark:text-gray-400">
                  <span>{data.turn_count} turns</span>
                  <span>{data.files_edited} files edited</span>
                  <span>{data.history_tokens.toLocaleString()} tokens</span>
                </div>
              </div>

              {/* Cache Status */}
              <div className="flex items-center gap-2">
                <span
                  className={`inline-flex items-center px-2 py-0.5 text-xs font-medium rounded-full ${
                    data.cache_warm
                      ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400'
                      : 'bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400'
                  }`}
                >
                  {data.cache_warm ? 'Cache Warm' : 'Cache Cold'}
                </span>
                {data.last_active_secs_ago > 0 && (
                  <span className="text-xs text-gray-400 dark:text-gray-500">
                    Last active {Math.floor(data.last_active_secs_ago / 60)}m ago
                  </span>
                )}
              </div>

              {/* Cost Breakdown */}
              <div className="rounded-lg bg-gray-50 dark:bg-gray-800/50 p-3 space-y-1">
                <div className="flex justify-between text-sm">
                  <span className="text-gray-600 dark:text-gray-400">First message</span>
                  <span className="font-mono text-gray-900 dark:text-gray-100">
                    ${data.first_message_cost.toFixed(4)}
                  </span>
                </div>
                <div className="flex justify-between text-sm">
                  <span className="text-gray-600 dark:text-gray-400">Per follow-up</span>
                  <span className="font-mono text-gray-900 dark:text-gray-100">
                    ~${data.per_message_cost.toFixed(4)}
                  </span>
                </div>
                <p className="text-xs text-gray-500 dark:text-gray-400 mt-2">{data.explanation}</p>
              </div>

              {/* Model Selector */}
              <div>
                <label
                  htmlFor="model-select"
                  className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
                >
                  Model
                </label>
                <select
                  id="model-select"
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                  className="w-full rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 px-3 py-2 text-sm text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
                >
                  {MODEL_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          )}

          {/* Actions */}
          <div className="mt-6 flex justify-end gap-3">
            <Dialog.Close asChild>
              <button
                type="button"
                className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-800 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700"
              >
                Cancel
              </button>
            </Dialog.Close>
            <button
              type="button"
              onClick={() => resumeMutation.mutate()}
              disabled={!data || resumeMutation.isPending}
              className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {resumeMutation.isPending ? 'Resuming...' : 'Resume in Dashboard'}
            </button>
          </div>

          {resumeMutation.error && (
            <p className="mt-2 text-sm text-red-600 dark:text-red-400">
              {(resumeMutation.error as Error).message}
            </p>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
