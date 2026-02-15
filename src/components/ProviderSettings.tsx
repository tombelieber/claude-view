import { useState } from 'react'
import {
  Settings2,
  Terminal,
  Key,
  Server,
  CheckCircle2,
  AlertCircle,
  Loader2,
} from 'lucide-react'
import { cn } from '../lib/utils'
import type { ClaudeCliStatus } from '../types/generated'

type ProviderType = 'claude-cli' | 'anthropic-api' | 'openai-compatible'

interface ProviderOption {
  id: ProviderType
  label: string
  description: string
  icon: React.ReactNode
  available: boolean
}

interface ProviderSettingsProps {
  onClose?: () => void
  cliStatus?: ClaudeCliStatus
}

const PROVIDERS: ProviderOption[] = [
  {
    id: 'claude-cli',
    label: 'Claude CLI',
    description: 'Uses locally installed Claude CLI. No API key needed.',
    icon: <Terminal className="w-4 h-4" />,
    available: true,
  },
  {
    id: 'anthropic-api',
    label: 'Anthropic API',
    description: 'Direct API access with your own key. Faster and more reliable.',
    icon: <Key className="w-4 h-4" />,
    available: false,
  },
  {
    id: 'openai-compatible',
    label: 'OpenAI Compatible',
    description: 'Any OpenAI-compatible endpoint (Ollama, LM Studio, etc.)',
    icon: <Server className="w-4 h-4" />,
    available: false,
  },
]

/**
 * Provider settings form for classification configuration.
 *
 * Currently only supports Claude CLI (default provider).
 * Anthropic API and OpenAI-compatible providers are planned for future releases.
 *
 * Shows:
 * - Provider selection (radio group)
 * - Model selection per provider
 * - API key input for API providers (future)
 * - Endpoint URL for OpenAI-compatible (future)
 * - Test connection button
 */
export function ProviderSettings({ onClose: _onClose, cliStatus }: ProviderSettingsProps) {
  const [selectedProvider, setSelectedProvider] = useState<ProviderType>('claude-cli')
  const [model, setModel] = useState('haiku')
  const [isTesting, setIsTesting] = useState(false)
  const [testResult, setTestResult] = useState<'idle' | 'success' | 'error' | 'not-installed' | 'not-authenticated'>('idle')

  const handleTestConnection = async () => {
    setIsTesting(true)
    setTestResult('idle')

    try {
      if (selectedProvider === 'claude-cli') {
        // Check the full provider chain: CLI installed → authenticated → API reachable
        if (!cliStatus?.path) {
          setTestResult('not-installed')
          return
        }
        if (!cliStatus?.authenticated) {
          setTestResult('not-authenticated')
          return
        }
      }

      const res = await fetch('/api/classify/status')
      if (res.ok) {
        setTestResult('success')
      } else {
        setTestResult('error')
      }
    } catch {
      setTestResult('error')
    } finally {
      setIsTesting(false)
      setTimeout(() => setTestResult('idle'), 5000)
    }
  }

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <Settings2 className="w-4 h-4 text-gray-500 dark:text-gray-400" />
          <h2 className="text-sm font-semibold text-gray-700 dark:text-gray-300 uppercase tracking-wide">
            Classification Provider
          </h2>
        </div>
      </div>

      {/* Body */}
      <div className="p-4">
        <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
          Choose how sessions are classified. The provider determines which LLM service is used for categorization.
        </p>

        {/* Provider selection */}
        <div className="space-y-2 mb-4">
          {PROVIDERS.map((provider) => (
            <label
              key={provider.id}
              className={cn(
                'flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition-colors duration-150',
                selectedProvider === provider.id
                  ? 'border-blue-300 dark:border-blue-600 bg-blue-50/50 dark:bg-blue-900/10'
                  : 'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800',
                !provider.available && 'opacity-50 cursor-not-allowed'
              )}
            >
              <input
                type="radio"
                name="provider"
                value={provider.id}
                checked={selectedProvider === provider.id}
                onChange={() => provider.available && setSelectedProvider(provider.id)}
                disabled={!provider.available}
                className="mt-0.5 w-4 h-4 text-blue-600 focus:ring-blue-500"
              />
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-gray-500 dark:text-gray-400">
                    {provider.icon}
                  </span>
                  <span className="text-sm font-medium text-gray-900 dark:text-gray-100">
                    {provider.label}
                  </span>
                  {!provider.available && (
                    <span className="text-[10px] font-medium px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400">
                      Coming Soon
                    </span>
                  )}
                </div>
                <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                  {provider.description}
                </p>
              </div>
            </label>
          ))}
        </div>

        {/* Model selection (for Claude CLI) */}
        {selectedProvider === 'claude-cli' && (
          <div className="mb-4">
            <label
              htmlFor="model-select"
              className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2 block"
            >
              Model
            </label>
            <select
              id="model-select"
              value={model}
              onChange={(e) => setModel(e.target.value)}
              className="text-sm border border-gray-200 dark:border-gray-700 rounded-md px-3 py-2 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:ring-2 focus:ring-blue-400 focus:outline-none w-full"
            >
              <option value="haiku">Claude Haiku (fastest, cheapest)</option>
              <option value="sonnet">Claude Sonnet (balanced)</option>
              <option value="opus">Claude Opus (most capable)</option>
            </select>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1.5">
              Haiku is recommended for classification. It provides excellent accuracy at the lowest cost.
            </p>
          </div>
        )}

        {/* CLI status (inline when Claude CLI is selected) */}
        {selectedProvider === 'claude-cli' && cliStatus && (
          <div className="mb-4 p-3 rounded-lg bg-gray-50 dark:bg-gray-800/50 space-y-1.5">
            <div className="flex items-center gap-2">
              {cliStatus.path ? (
                <CheckCircle2 className="w-3.5 h-3.5 text-green-500 flex-shrink-0" />
              ) : (
                <AlertCircle className="w-3.5 h-3.5 text-red-500 flex-shrink-0" />
              )}
              <span className="text-xs text-gray-600 dark:text-gray-400">
                {cliStatus.path ? (
                  <>Installed: <code className="font-mono bg-gray-100 dark:bg-gray-700 px-1 rounded">{cliStatus.path}</code></>
                ) : (
                  'CLI not installed'
                )}
              </span>
            </div>
            {cliStatus.path && (
              <div className="flex items-center gap-2">
                {cliStatus.authenticated ? (
                  <CheckCircle2 className="w-3.5 h-3.5 text-green-500 flex-shrink-0" />
                ) : (
                  <AlertCircle className="w-3.5 h-3.5 text-amber-500 flex-shrink-0" />
                )}
                <span className="text-xs text-gray-600 dark:text-gray-400">
                  {cliStatus.authenticated ? (
                    <>
                      Authenticated
                      {cliStatus.subscriptionType && cliStatus.subscriptionType !== 'unknown' && (
                        <> ({cliStatus.subscriptionType.charAt(0).toUpperCase() + cliStatus.subscriptionType.slice(1)})</>
                      )}
                    </>
                  ) : (
                    <>
                      Not authenticated —{' '}
                      run <code className="font-mono bg-gray-100 dark:bg-gray-700 px-1 rounded">claude auth login</code>
                    </>
                  )}
                </span>
              </div>
            )}
          </div>
        )}

        {/* Test connection */}
        <div className="flex items-center gap-3 pt-3 border-t border-gray-100 dark:border-gray-800">
          <button
            type="button"
            onClick={handleTestConnection}
            disabled={isTesting}
            className={cn(
              'inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-md cursor-pointer',
              'border border-gray-300 dark:border-gray-600',
              'text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors duration-150'
            )}
          >
            {isTesting ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Terminal className="w-3.5 h-3.5" />
            )}
            Test Connection
          </button>

          {testResult === 'success' && (
            <div className="flex items-center gap-1 text-green-600 dark:text-green-400">
              <CheckCircle2 className="w-3.5 h-3.5" />
              <span className="text-xs">Ready to classify</span>
            </div>
          )}
          {testResult === 'not-installed' && (
            <div className="flex items-center gap-1 text-red-600 dark:text-red-400">
              <AlertCircle className="w-3.5 h-3.5" />
              <span className="text-xs">Claude CLI not installed</span>
            </div>
          )}
          {testResult === 'not-authenticated' && (
            <div className="flex items-center gap-1 text-amber-600 dark:text-amber-400">
              <AlertCircle className="w-3.5 h-3.5" />
              <span className="text-xs">CLI not authenticated — run <code className="font-mono bg-amber-50 dark:bg-amber-900/30 px-1 rounded">claude auth login</code></span>
            </div>
          )}
          {testResult === 'error' && (
            <div className="flex items-center gap-1 text-red-600 dark:text-red-400">
              <AlertCircle className="w-3.5 h-3.5" />
              <span className="text-xs">Connection failed</span>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
