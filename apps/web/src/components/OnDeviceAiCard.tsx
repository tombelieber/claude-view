import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Cpu, Loader2, Power, PowerOff } from 'lucide-react'
import { useCallback, useRef, useState } from 'react'
import { cn } from '../lib/utils'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ServiceStatus {
  enabled: boolean
  llm: {
    ready: boolean
    port: number
    pid: number | null
    state: 'unknown' | 'running' | 'unavailable'
  }
  model_exists: boolean
  model_size_bytes: number | null
}

interface DownloadProgress {
  bytes_downloaded: number
  total_bytes: number | null
  percent: number | null
  done: boolean
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Binary 1024-based byte formatting. */
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB']
  const k = 1024
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  const value = bytes / k ** i
  return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function OnDeviceAiCard() {
  const queryClient = useQueryClient()
  const [isEnabling, setIsEnabling] = useState(false)
  const [isDisabling, setIsDisabling] = useState(false)
  const [download, setDownload] = useState<DownloadProgress | null>(null)
  const abortRef = useRef<AbortController | null>(null)

  const { data: status } = useQuery<ServiceStatus>({
    queryKey: ['local-llm-status'],
    queryFn: async () => {
      const res = await fetch('/api/local-llm/status')
      if (!res.ok) throw new Error('Failed to fetch local LLM status')
      return res.json()
    },
    refetchInterval: 5000,
    retry: 1,
    staleTime: 3000,
  })

  const handleEnable = useCallback(async () => {
    setIsEnabling(true)
    setDownload(null)
    abortRef.current = new AbortController()

    try {
      const res = await fetch('/api/local-llm/enable', {
        method: 'POST',
        signal: abortRef.current.signal,
      })

      if (!res.ok) throw new Error('Failed to enable local LLM')

      const contentType = res.headers.get('content-type') ?? ''
      if (contentType.includes('text/event-stream') && res.body) {
        const reader = res.body.getReader()
        const decoder = new TextDecoder()
        let buf = ''

        while (true) {
          const { done, value } = await reader.read()
          if (done) break
          buf += decoder.decode(value, { stream: true })

          const lines = buf.split('\n')
          buf = lines.pop() ?? ''

          for (const line of lines) {
            if (line.startsWith('data: ')) {
              try {
                const progress = JSON.parse(line.slice(6)) as DownloadProgress
                setDownload(progress)
                if (progress.done) {
                  setDownload(null)
                }
              } catch {
                // skip malformed SSE lines
              }
            }
          }
        }
      }

      queryClient.invalidateQueries({ queryKey: ['local-llm-status'] })
    } catch (err) {
      if (err instanceof DOMException && err.name === 'AbortError') return
      // silently fail — status poll will reflect reality
    } finally {
      setIsEnabling(false)
      abortRef.current = null
    }
  }, [queryClient])

  const handleDisable = useCallback(async () => {
    setIsDisabling(true)
    try {
      const res = await fetch('/api/local-llm/disable', { method: 'POST' })
      if (!res.ok) throw new Error('Failed to disable local LLM')
      queryClient.invalidateQueries({ queryKey: ['local-llm-status'] })
    } catch {
      // silently fail — status poll will reflect reality
    } finally {
      setIsDisabling(false)
    }
  }, [queryClient])

  const isRunning = status?.llm.state === 'running'
  const isStarting = status?.enabled && !isRunning && !download
  const busy = isEnabling || isDisabling

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-4 py-3 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
        <Cpu className="w-4 h-4 text-gray-500 dark:text-gray-400" />
        <h2 className="text-sm font-semibold text-gray-700 dark:text-gray-300 uppercase tracking-wide">
          On-Device AI
        </h2>
        <span className="ml-auto inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300">
          Experimental
        </span>
      </div>

      {/* Body */}
      <div className="p-4">
        <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
          Run a local LLM on your machine for offline classification and enrichment.
        </p>

        {/* Status row */}
        {status && (
          <div className="space-y-1 mb-4">
            <div className="flex items-center justify-between py-1.5">
              <span className="text-sm text-gray-500 dark:text-gray-400">Status</span>
              <span
                className={cn(
                  'text-sm font-medium',
                  isRunning
                    ? 'text-green-600 dark:text-green-400'
                    : status.enabled
                      ? 'text-amber-600 dark:text-amber-400'
                      : 'text-gray-500 dark:text-gray-400',
                )}
              >
                {isRunning
                  ? 'Running'
                  : isStarting
                    ? 'Starting...'
                    : status.enabled
                      ? 'Enabled'
                      : 'Disabled'}
              </span>
            </div>

            {isRunning && status.llm.port > 0 && (
              <div className="flex items-center justify-between py-1.5">
                <span className="text-sm text-gray-500 dark:text-gray-400">Port</span>
                <span className="text-sm font-medium text-gray-900 dark:text-gray-100 tabular-nums">
                  {status.llm.port}
                </span>
              </div>
            )}

            {status.model_size_bytes != null && status.model_size_bytes > 0 && (
              <div className="flex items-center justify-between py-1.5">
                <span className="text-sm text-gray-500 dark:text-gray-400">Model size</span>
                <span className="text-sm font-medium text-gray-900 dark:text-gray-100 tabular-nums">
                  {formatBytes(status.model_size_bytes)}
                </span>
              </div>
            )}
          </div>
        )}

        {/* Starting indicator */}
        {isStarting && (
          <div className="flex items-center gap-2 text-amber-600 dark:text-amber-400 mb-4 text-sm">
            <Loader2 className="w-4 h-4 animate-spin" />
            <span>Starting local AI server...</span>
          </div>
        )}

        {/* Download progress */}
        {download && !download.done && (
          <div className="mb-4">
            <div className="flex items-center justify-between text-sm text-gray-600 dark:text-gray-300 mb-1">
              <span>Downloading model...</span>
              <span className="tabular-nums">
                {download.percent != null
                  ? `${download.percent.toFixed(1)}%`
                  : formatBytes(download.bytes_downloaded)}
              </span>
            </div>
            <div className="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
              <div
                className="h-full bg-blue-500 dark:bg-blue-400 rounded-full transition-all duration-300"
                style={{
                  width: download.percent != null ? `${download.percent}%` : '0%',
                }}
              />
            </div>
            {download.total_bytes != null && (
              <div className="text-xs text-gray-400 dark:text-gray-500 mt-1 tabular-nums">
                {formatBytes(download.bytes_downloaded)} / {formatBytes(download.total_bytes)}
              </div>
            )}
          </div>
        )}

        {/* Action buttons */}
        {!status?.enabled ? (
          <button
            type="button"
            onClick={handleEnable}
            disabled={busy}
            className={cn(
              'inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md cursor-pointer',
              'transition-colors duration-150',
              'bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 hover:bg-gray-800 dark:hover:bg-gray-200',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2',
            )}
          >
            {isEnabling ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                Enabling...
              </>
            ) : (
              <>
                <Power className="w-4 h-4" />
                Enable Local AI
              </>
            )}
          </button>
        ) : (
          <button
            type="button"
            onClick={handleDisable}
            disabled={busy}
            className={cn(
              'inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md cursor-pointer',
              'transition-colors duration-150',
              'text-red-600 dark:text-red-400 border border-red-200 dark:border-red-800',
              'hover:bg-red-50 dark:hover:bg-red-900/20',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'focus-visible:ring-2 focus-visible:ring-red-400 focus-visible:ring-offset-2',
            )}
          >
            {isDisabling ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                Disabling...
              </>
            ) : (
              <>
                <PowerOff className="w-4 h-4" />
                Disable Local AI
              </>
            )}
          </button>
        )}
      </div>
    </div>
  )
}
