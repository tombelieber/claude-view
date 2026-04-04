import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  Check,
  ChevronDown,
  ChevronRight,
  Copy,
  Cpu,
  ExternalLink,
  Loader2,
  Plug,
  PlugZap,
  Unplug,
  Wifi,
  WifiOff,
} from 'lucide-react'
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
  const [showAlternatives, setShowAlternatives] = useState(false)

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
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['local-llm-status'] }),
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
    onSuccess: () => {
      setCustomUrl('')
      queryClient.invalidateQueries({ queryKey: ['local-llm-status'] })
    },
  })

  const disconnectMutation = useMutation({
    mutationFn: async () => {
      const res = await fetch('/api/local-llm/disconnect', { method: 'POST' })
      if (!res.ok) throw new Error('disconnect failed')
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['local-llm-status'] }),
  })

  if (!status) {
    return (
      <div className="rounded-lg border border-gray-200 dark:border-gray-700 p-4">
        <div className="flex items-center gap-2 text-sm text-gray-400">
          <Loader2 className="w-4 h-4 animate-spin" />
          Loading local AI status…
        </div>
      </div>
    )
  }

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
      {/* Header with toggle */}
      <div className="flex items-center justify-between px-4 py-3 bg-gray-50 dark:bg-gray-800/50">
        <div className="flex items-center gap-2">
          <Cpu className="w-4 h-4 text-gray-500" />
          <span className="text-sm font-medium">On-Device AI</span>
          {status.state === 'connected' && status.provider && (
            <span className="text-xs px-1.5 py-0.5 rounded-full bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400">
              {PROVIDER_LABELS[status.provider]}
            </span>
          )}
        </div>
        <button
          type="button"
          onClick={() => toggleMutation.mutate(!status.enabled)}
          disabled={toggleMutation.isPending}
          className={cn(
            'relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none',
            status.enabled ? 'bg-blue-500' : 'bg-gray-300 dark:bg-gray-600',
          )}
        >
          <span
            className={cn(
              'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out',
              status.enabled ? 'translate-x-4' : 'translate-x-0',
            )}
          />
        </button>
      </div>

      {/* Body — rendered based on state */}
      {status.enabled && (
        <div className="px-4 py-3 space-y-3 text-sm">
          {status.state === 'scanning' && <ScanningState />}
          {status.state === 'connected' && (
            <ConnectedState status={status} onDisconnect={() => disconnectMutation.mutate()} />
          )}
          {status.state === 'disconnected' && (
            <DisconnectedState
              status={status}
              customUrl={customUrl}
              setCustomUrl={setCustomUrl}
              onConnect={(url) => connectMutation.mutate(url)}
              showAlternatives={showAlternatives}
              setShowAlternatives={setShowAlternatives}
            />
          )}
          {status.state === 'unknown' && <div className="text-gray-400 text-xs">Initializing…</div>}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// State components
// ---------------------------------------------------------------------------

function ScanningState() {
  return (
    <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400">
      <Loader2 className="w-4 h-4 animate-spin" />
      <span>Scanning for local LLM servers…</span>
    </div>
  )
}

function ConnectedState({
  status,
  onDisconnect,
}: {
  status: ServiceStatus
  onDisconnect: () => void
}) {
  return (
    <div className="space-y-2">
      {/* Connection info */}
      <div className="flex items-center gap-2 text-green-600 dark:text-green-400">
        <Wifi className="w-4 h-4" />
        <span className="font-medium">Connected</span>
        {status.url && (
          <span className="text-xs text-gray-400 font-mono truncate max-w-48">{status.url}</span>
        )}
      </div>

      {/* Active model */}
      {status.active_model && (
        <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
          <span className="font-medium">Model:</span>
          <code className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 font-mono">
            {status.active_model}
          </code>
        </div>
      )}

      {/* Model list (if 2+) */}
      {status.models.length > 1 && (
        <div className="text-xs text-gray-400">{status.models.length} models available</div>
      )}

      {/* oMLX Apple Silicon badge */}
      {status.provider === 'omlx' && (
        <div className="inline-flex items-center gap-1.5 text-xs px-2 py-1 rounded-md bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300">
          <PlugZap className="w-3 h-3" />
          Apple Silicon optimized
        </div>
      )}

      {/* Disconnect button */}
      <div className="pt-1">
        <button
          type="button"
          onClick={onDisconnect}
          className="inline-flex items-center gap-1.5 text-xs px-2.5 py-1.5 rounded-md border border-gray-200 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors cursor-pointer text-gray-500"
        >
          <Unplug className="w-3 h-3" />
          Disconnect
        </button>
      </div>
    </div>
  )
}

function DisconnectedState({
  status,
  customUrl,
  setCustomUrl,
  onConnect,
  showAlternatives,
  setShowAlternatives,
}: {
  status: ServiceStatus
  customUrl: string
  setCustomUrl: (url: string) => void
  onConnect: (url: string) => void
  showAlternatives: boolean
  setShowAlternatives: (v: boolean) => void
}) {
  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400">
        <WifiOff className="w-4 h-4" />
        <span>No local LLM server detected</span>
      </div>

      {/* oMLX-specific guidance */}
      <OmlxGuidance status={status} />

      {/* Alternatives (collapsed by default) */}
      <button
        type="button"
        onClick={() => setShowAlternatives(!showAlternatives)}
        className="flex items-center gap-1 text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors cursor-pointer"
      >
        {showAlternatives ? (
          <ChevronDown className="w-3 h-3" />
        ) : (
          <ChevronRight className="w-3 h-3" />
        )}
        Other providers
      </button>

      {showAlternatives && (
        <div className="space-y-2 pl-4 text-xs text-gray-500 dark:text-gray-400">
          <a
            href="https://ollama.com"
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
          >
            Ollama <ExternalLink className="w-3 h-3" />
          </a>
          <a
            href="https://lmstudio.ai"
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
          >
            LM Studio <ExternalLink className="w-3 h-3" />
          </a>
        </div>
      )}

      {/* Custom URL */}
      <div className="pt-1 space-y-1.5">
        <label htmlFor="local-llm-custom-url" className="text-xs text-gray-400">
          Custom URL
        </label>
        <div className="flex gap-2">
          <input
            id="local-llm-custom-url"
            type="text"
            value={customUrl}
            onChange={(e) => setCustomUrl(e.target.value)}
            placeholder="http://localhost:8080"
            className="flex-1 text-xs px-2.5 py-1.5 rounded-md border border-gray-200 dark:border-gray-600 bg-white dark:bg-gray-800 focus:outline-none focus:ring-1 focus:ring-blue-400"
          />
          <button
            type="button"
            onClick={() => customUrl.trim() && onConnect(customUrl.trim())}
            disabled={!customUrl.trim()}
            className="inline-flex items-center gap-1 text-xs px-2.5 py-1.5 rounded-md bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-40 disabled:cursor-not-allowed transition-colors cursor-pointer"
          >
            <Plug className="w-3 h-3" />
            Connect
          </button>
        </div>
      </div>
    </div>
  )
}

function OmlxGuidance({ status }: { status: ServiceStatus }) {
  // Four sub-states based on omlx_installed and omlx_running
  if (!status.omlx_installed) {
    return (
      <div className="rounded-md bg-blue-50 dark:bg-blue-900/20 p-3 space-y-1.5">
        <div className="text-xs font-medium text-blue-700 dark:text-blue-300">
          Recommended: Install oMLX for Apple Silicon
        </div>
        <div className="flex items-center gap-2">
          <code className="text-xs font-mono px-2 py-1 rounded bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-200">
            pip install omlx
          </code>
          <CopyButton text="pip install omlx" />
        </div>
      </div>
    )
  }

  if (status.omlx_installed && !status.omlx_running) {
    return (
      <div className="rounded-md bg-amber-50 dark:bg-amber-900/20 p-3 space-y-1.5">
        <div className="text-xs font-medium text-amber-700 dark:text-amber-300">
          oMLX is installed but not running
        </div>
        <div className="flex items-center gap-2">
          <code className="text-xs font-mono px-2 py-1 rounded bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-200">
            omlx serve
          </code>
          <CopyButton text="omlx serve" />
        </div>
      </div>
    )
  }

  if (status.omlx_installed && status.omlx_running) {
    return (
      <div className="rounded-md bg-amber-50 dark:bg-amber-900/20 p-3 space-y-1.5">
        <div className="text-xs font-medium text-amber-700 dark:text-amber-300">
          oMLX is running but no models loaded
        </div>
        <div className="flex items-center gap-2">
          <code className="text-xs font-mono px-2 py-1 rounded bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-200">
            omlx pull mlx-community/Qwen3-4B-4bit
          </code>
          <CopyButton text="omlx pull mlx-community/Qwen3-4B-4bit" />
        </div>
      </div>
    )
  }

  return null
}
