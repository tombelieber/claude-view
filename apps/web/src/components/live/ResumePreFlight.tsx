import * as Dialog from '@radix-ui/react-dialog'
import { useQuery } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import type { CostEstimate } from '../../types/control'
import { DialogContent, DialogOverlay } from '../ui/CenteredDialog'

interface ResumePreFlightProps {
  sessionId: string
  open: boolean
  onOpenChange: (open: boolean) => void
  onResume: (sessionId: string) => void
}

/** Current-gen models offered in the selector. Users can always switch model on resume. */
const MODEL_OPTIONS: { value: string; label: string }[] = [
  { value: 'claude-sonnet-4-6', label: 'Sonnet 4.6' },
  { value: 'claude-opus-4-6', label: 'Opus 4.6' },
  { value: 'claude-haiku-4-5-20251001', label: 'Haiku 4.5' },
  { value: 'claude-sonnet-4-5-20250929', label: 'Sonnet 4.5' },
  { value: 'claude-opus-4-5-20251101', label: 'Opus 4.5' },
  { value: 'claude-sonnet-4-20250514', label: 'Sonnet 4' },
  { value: 'claude-opus-4-20250514', label: 'Opus 4' },
]

/** Turn a raw model ID into a display label (e.g. "claude-opus-4-6" → "Opus 4.6"). */
function modelLabel(id: string): string {
  const found = MODEL_OPTIONS.find((o) => o.value === id)
  if (found) return found.label
  // Fallback: strip "claude-" prefix and date suffix, title-case the rest
  return id
    .replace(/^claude-/, '')
    .replace(/-\d{8}$/, '')
    .replace(/-/g, ' ')
    .replace(/\b\w/g, (c) => c.toUpperCase())
}

export function ResumePreFlight({ sessionId, open, onOpenChange, onResume }: ResumePreFlightProps) {
  // null = not yet loaded, let backend pick the session's original model
  const [model, setModel] = useState<string | null>(null)

  // Reset model when switching sessions so stale selection doesn't carry over
  // biome-ignore lint/correctness/useExhaustiveDependencies: sessionId is an intentional trigger
  useEffect(() => {
    setModel(null)
  }, [sessionId])

  // Use model for the query key only after it's been synced from the backend.
  // While model is null, we omit it so the initial fetch and the post-sync state
  // share the same cache entry — avoiding a redundant double-fetch.
  const queryModel = model !== null ? model : undefined
  const estimate = useQuery({
    queryKey: ['cost-estimate', sessionId, queryModel],
    queryFn: async (): Promise<CostEstimate> => {
      const body: Record<string, string> = { session_id: sessionId }
      if (model) body.model = model
      const res = await fetch('/api/control/estimate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      })
      if (!res.ok) throw new Error('Failed to fetch cost estimate')
      return res.json()
    },
    enabled: open && !!sessionId,
  })

  // Sync model selector from the first estimate response (session's original model)
  useEffect(() => {
    if (estimate.data && model === null) {
      setModel(estimate.data.model)
    }
  }, [estimate.data, model])

  // Build options: static list + session's model if not already present
  const options = MODEL_OPTIONS.some((o) => o.value === (model ?? ''))
    ? MODEL_OPTIONS
    : model
      ? [{ value: model, label: `${modelLabel(model)} (session)` }, ...MODEL_OPTIONS]
      : MODEL_OPTIONS

  const data = estimate.data
  const selectValue = model ?? ''

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <DialogOverlay className="bg-black/50 dark:bg-black/70" />
        <DialogContent className="max-w-md bg-white dark:bg-gray-900 rounded-xl shadow-xl p-6">
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
                {data.has_pricing ? (
                  <>
                    <div className="flex justify-between text-sm">
                      <span className="text-gray-600 dark:text-gray-400">First message</span>
                      <span className="font-mono text-gray-900 dark:text-gray-100">
                        ${data.first_message_cost?.toFixed(4) ?? '--'}
                      </span>
                    </div>
                    <div className="flex justify-between text-sm">
                      <span className="text-gray-600 dark:text-gray-400">
                        Per follow-up estimate
                      </span>
                      <span className="font-mono text-gray-900 dark:text-gray-100">
                        ${data.per_message_cost?.toFixed(4) ?? '--'}
                      </span>
                    </div>
                  </>
                ) : (
                  <div className="text-sm text-amber-700 dark:text-amber-300">
                    Cost estimate unavailable for this model (pricing data missing).
                  </div>
                )}
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
                  value={selectValue}
                  onChange={(e) => setModel(e.target.value)}
                  className="w-full rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 px-3 py-2 text-sm text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
                >
                  {options.map((opt) => (
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
              onClick={() => {
                onOpenChange(false)
                onResume(sessionId)
              }}
              disabled={!data}
              className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Resume in Dashboard
            </button>
          </div>
        </DialogContent>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
