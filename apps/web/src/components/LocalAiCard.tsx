import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Check, Copy, Cpu, Loader2, Plug, PlugZap, Wifi } from 'lucide-react'
import { useCallback, useState } from 'react'
import { cn } from '../lib/utils'

// ---------------------------------------------------------------------------
// Types — match the Rust StatusSnapshot exactly
// ---------------------------------------------------------------------------

type Provider = 'omlx' | 'ollama' | 'lm_studio' | 'custom'
type ServerState = 'unknown' | 'scanning' | 'connected' | 'disconnected'

interface ServiceStatus {
  enabled: boolean
  state: ServerState
  provider: Provider | null
  url: string | null
  models: string[]
  active_model: string | null
  classify_mode: 'Efficient' | 'Balanced' | 'Aggressive'
  omlx_installed: boolean
  omlx_running: boolean
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const PROVIDER_LABELS: Record<Provider, string> = {
  omlx: 'oMLX',
  ollama: 'Ollama',
  lm_studio: 'LM Studio',
  custom: 'Custom',
}

/** Toggle switch — matches TelemetrySection dimensions with refined shadow and easing. */
function Toggle({
  checked,
  onChange,
  disabled,
  loading,
}: {
  checked: boolean
  onChange: (v: boolean) => void
  disabled?: boolean
  loading?: boolean
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        'relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent',
        'transition-colors duration-300 ease-[cubic-bezier(0.25,0.1,0.25,1)]',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2',
        'disabled:cursor-not-allowed disabled:opacity-50',
        checked ? 'bg-green-500 dark:bg-green-600' : 'bg-gray-300 dark:bg-gray-600',
      )}
    >
      <span
        className={cn(
          'pointer-events-none inline-flex h-5 w-5 transform items-center justify-center rounded-full bg-white ring-0',
          'transition-transform duration-300 ease-[cubic-bezier(0.25,0.1,0.25,1)]',
          checked ? 'translate-x-5' : 'translate-x-0',
        )}
        style={{
          boxShadow: '0 2px 4px rgba(0,0,0,0.15), 0 1px 2px rgba(0,0,0,0.06)',
        }}
      >
        {loading ? (
          <Loader2 className="h-3 w-3 animate-spin text-gray-400" />
        ) : (
          checked && <Check className="h-3 w-3 text-green-600" />
        )}
      </span>
    </button>
  )
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }, [text])

  return (
    <button
      type="button"
      onClick={handleCopy}
      className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors cursor-pointer"
      title="Copy to clipboard"
    >
      {copied ? <Check className="w-3 h-3 text-green-500" /> : <Copy className="w-3 h-3" />}
    </button>
  )
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function LocalAiCard() {
  const queryClient = useQueryClient()
  const [customUrl, setCustomUrl] = useState('')

  const { data: status } = useQuery<ServiceStatus>({
    queryKey: ['local-llm-status'],
    queryFn: async () => {
      const res = await fetch('/api/local-llm/status')
      if (!res.ok) throw new Error('status fetch failed')
      return res.json()
    },
    refetchInterval: 5_000,
    staleTime: 3_000,
    retry: 1,
  })

  const toggleMutation = useMutation({
    mutationFn: async (enabled: boolean) => {
      const res = await fetch('/api/local-llm/toggle', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled }),
      })
      if (!res.ok) throw new Error('toggle failed')
    },
    onMutate: async (enabled) => {
      await queryClient.cancelQueries({ queryKey: ['local-llm-status'] })
      const previous = queryClient.getQueryData<ServiceStatus>(['local-llm-status'])
      queryClient.setQueryData<ServiceStatus>(['local-llm-status'], (old) =>
        old ? { ...old, enabled } : old,
      )
      return { previous }
    },
    onError: (_err, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData(['local-llm-status'], context.previous)
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['local-llm-status'] })
    },
  })

  const connectMutation = useMutation({
    mutationFn: async (url: string) => {
      const res = await fetch('/api/local-llm/connect', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ url }),
      })
      if (!res.ok) throw new Error('connect failed')
    },
    onMutate: async () => {
      await queryClient.cancelQueries({ queryKey: ['local-llm-status'] })
      const previous = queryClient.getQueryData<ServiceStatus>(['local-llm-status'])
      queryClient.setQueryData<ServiceStatus>(['local-llm-status'], (old) =>
        old ? { ...old, enabled: true, state: 'scanning' as ServerState } : old,
      )
      return { previous }
    },
    onError: (_err, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData(['local-llm-status'], context.previous)
      }
    },
    onSettled: () => {
      setCustomUrl('')
      queryClient.invalidateQueries({ queryKey: ['local-llm-status'] })
    },
  })

  if (!status) {
    return (
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700">
        <div className="px-5 py-4">
          <div className="flex items-center gap-2 text-sm text-gray-400">
            <Loader2 className="w-4 h-4 animate-spin" />
            Loading local AI status…
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700">
      {/* Header — matches SettingsSection pattern */}
      <div className="flex items-center justify-between px-5 pt-4 pb-1.5">
        <div className="flex items-center gap-2">
          <Cpu className="w-4 h-4 text-gray-400 dark:text-gray-500" />
          <h2 className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider">
            On-Device AI
          </h2>
          {status.enabled && status.state === 'connected' && status.provider && (
            <span className="inline-flex items-center px-1.5 py-0.5 rounded-full text-xs font-medium bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400">
              {PROVIDER_LABELS[status.provider]}
            </span>
          )}
        </div>
        <Toggle
          checked={status.enabled}
          onChange={(v) => toggleMutation.mutate(v)}
          disabled={toggleMutation.isPending}
          loading={toggleMutation.isPending}
        />
      </div>

      {/* Body — one of three clean states */}
      {status.enabled && (
        <div className="px-5 pb-5 pt-1 text-sm">
          {toggleMutation.isPending || status.state === 'scanning' || status.state === 'unknown' ? (
            <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400">
              <Loader2 className="w-4 h-4 animate-spin" />
              <span>Scanning for local servers…</span>
            </div>
          ) : status.state === 'connected' ? (
            <ConnectedInfo status={status} />
          ) : (
            <NotFoundState
              status={status}
              customUrl={customUrl}
              setCustomUrl={setCustomUrl}
              onConnect={(url) => connectMutation.mutate(url)}
              isConnecting={connectMutation.isPending}
            />
          )}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Connected — clean status display, no actions (toggle OFF to disconnect)
// ---------------------------------------------------------------------------

function ConnectedInfo({ status }: { status: ServiceStatus }) {
  const hasModelDetails = !!status.active_model

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2 text-green-600 dark:text-green-400">
        <Wifi className="w-4 h-4" />
        <span className="font-medium">Connected</span>
        {status.url && (
          <span className="text-xs text-gray-400 dark:text-gray-500 font-mono truncate max-w-48">
            {status.url}
          </span>
        )}
      </div>

      {hasModelDetails ? (
        <>
          <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
            <span className="font-medium">Model:</span>
            <code className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 font-mono">
              {status.active_model}
            </code>
          </div>
          {status.models.length > 1 && (
            <div className="text-xs text-gray-400 dark:text-gray-500">
              {status.models.length} models available
            </div>
          )}
          {status.provider === 'omlx' && (
            <div className="inline-flex items-center gap-1.5 text-xs px-2 py-1 rounded-md bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300">
              <PlugZap className="w-3 h-3" />
              Apple Silicon optimized
            </div>
          )}
        </>
      ) : (
        <div className="space-y-2.5" role="status" aria-label="Loading model details">
          <div className="flex items-center gap-2">
            <div className="h-4 w-12 rounded bg-gray-200 dark:bg-gray-700 animate-pulse" />
            <div className="h-4 w-36 rounded bg-gray-100 dark:bg-gray-800 animate-pulse" />
          </div>
          <div className="h-[22px] w-40 rounded-md bg-gray-100 dark:bg-gray-800 animate-pulse" />
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Not found — one guidance block + custom URL fallback
// ---------------------------------------------------------------------------

function NotFoundState({
  status,
  customUrl,
  setCustomUrl,
  onConnect,
  isConnecting,
}: {
  status: ServiceStatus
  customUrl: string
  setCustomUrl: (url: string) => void
  onConnect: (url: string) => void
  isConnecting: boolean
}) {
  return (
    <div className="space-y-3">
      <p className="text-gray-500 dark:text-gray-400">No server detected</p>

      <SetupGuidance status={status} />

      <div className="space-y-1.5">
        <label htmlFor="local-llm-custom-url" className="text-xs text-gray-400 dark:text-gray-500">
          Or connect to a running server
        </label>
        <div className="flex gap-2">
          <input
            id="local-llm-custom-url"
            type="text"
            value={customUrl}
            onChange={(e) => setCustomUrl(e.target.value)}
            placeholder="http://localhost:8080"
            disabled={isConnecting}
            className="flex-1 text-xs px-2.5 py-1.5 rounded-md border border-gray-200 dark:border-gray-600 bg-white dark:bg-gray-800 focus:outline-none focus:ring-1 focus:ring-blue-400 disabled:opacity-50"
          />
          <button
            type="button"
            onClick={() => customUrl.trim() && onConnect(customUrl.trim())}
            disabled={!customUrl.trim() || isConnecting}
            className="inline-flex items-center gap-1 text-xs px-2.5 py-1.5 rounded-md bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 hover:bg-gray-800 dark:hover:bg-gray-200 disabled:opacity-40 disabled:cursor-not-allowed transition-colors cursor-pointer"
          >
            {isConnecting ? (
              <Loader2 className="w-3 h-3 animate-spin" />
            ) : (
              <Plug className="w-3 h-3" />
            )}
            Connect
          </button>
        </div>
      </div>

      <p className="text-xs text-gray-400 dark:text-gray-500">
        Works with{' '}
        <a
          href="https://github.com/nicholasgasior/omlx"
          target="_blank"
          rel="noopener noreferrer"
          className="underline decoration-gray-300 dark:decoration-gray-600 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
        >
          oMLX
        </a>
        ,{' '}
        <a
          href="https://ollama.com"
          target="_blank"
          rel="noopener noreferrer"
          className="underline decoration-gray-300 dark:decoration-gray-600 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
        >
          Ollama
        </a>
        , and{' '}
        <a
          href="https://lmstudio.ai"
          target="_blank"
          rel="noopener noreferrer"
          className="underline decoration-gray-300 dark:decoration-gray-600 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
        >
          LM Studio
        </a>
      </p>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Setup guidance — one contextual command based on oMLX state
// ---------------------------------------------------------------------------

function SetupGuidance({ status }: { status: ServiceStatus }) {
  let command: string
  let hint: string

  if (!status.omlx_installed) {
    command = 'pip install omlx && omlx serve'
    hint = 'Get started with oMLX for Apple Silicon'
  } else if (!status.omlx_running) {
    command = 'omlx serve'
    hint = 'Start the oMLX server'
  } else {
    command = 'omlx pull mlx-community/Qwen3-4B-4bit'
    hint = 'Load a model to get started'
  }

  return (
    <div className="rounded-md bg-gray-50 dark:bg-gray-800/50 p-3 space-y-1.5">
      <p className="text-xs font-medium text-gray-600 dark:text-gray-300">{hint}</p>
      <div className="flex items-center gap-2">
        <code className="text-xs font-mono px-2 py-1 rounded bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-200">
          {command}
        </code>
        <CopyButton text={command} />
      </div>
    </div>
  )
}
