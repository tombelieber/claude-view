import { Check, Clipboard, RefreshCw } from 'lucide-react'
import { useCallback, useState } from 'react'
import { cn } from '../lib/utils'

interface OnDeviceAiSetupGuideProps {
  onCheckInstall: () => void
  isChecking: boolean
}

const INSTALL_CMD = 'pip install omlx'

export function OnDeviceAiSetupGuide({ onCheckInstall, isChecking }: OnDeviceAiSetupGuideProps) {
  const [copied, setCopied] = useState(false)

  const handleCopy = useCallback(async () => {
    await navigator.clipboard.writeText(INSTALL_CMD)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }, [])

  return (
    <div className="space-y-4">
      <div>
        <p className="text-sm text-gray-600 dark:text-gray-300 mb-1">
          Classify your coding sessions into phases like building, testing, reviewing — entirely on
          your Mac.
        </p>
        <p className="text-xs text-gray-400 dark:text-gray-500">
          No cloud, no API keys. Powered by{' '}
          <a
            href="https://github.com/nicholascpark/omlx"
            target="_blank"
            rel="noopener noreferrer"
            className="underline decoration-gray-300 dark:decoration-gray-600 underline-offset-2 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
          >
            oMLX
          </a>{' '}
          on Apple Silicon.
        </p>
      </div>

      {/* Step 1: Install oMLX */}
      <div className="rounded-md border border-gray-200 dark:border-gray-700 p-3">
        <div className="flex items-start gap-2.5">
          <span className="shrink-0 mt-0.5 flex items-center justify-center h-5 w-5 rounded-full bg-gray-900 dark:bg-gray-100 text-[11px] font-semibold text-white dark:text-gray-900 tabular-nums">
            1
          </span>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100 mb-2">
              Install oMLX
            </p>
            <div className="flex items-center gap-2">
              <code className="flex-1 block px-3 py-1.5 rounded bg-gray-100 dark:bg-gray-800 text-sm font-mono text-gray-800 dark:text-gray-200 select-all">
                {INSTALL_CMD}
              </code>
              <button
                type="button"
                onClick={handleCopy}
                className={cn(
                  'shrink-0 p-1.5 rounded-md transition-colors cursor-pointer',
                  copied
                    ? 'text-green-600 dark:text-green-400 bg-green-50 dark:bg-green-900/20'
                    : 'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800',
                )}
                title={copied ? 'Copied' : 'Copy command'}
              >
                {copied ? <Check className="w-4 h-4" /> : <Clipboard className="w-4 h-4" />}
              </button>
            </div>
            <p className="text-xs text-gray-400 dark:text-gray-500 mt-1.5">
              Requires Python 3.10+ and Apple Silicon (M1 or later).
            </p>
          </div>
        </div>
      </div>

      {/* Step 2: Enable */}
      <div className="rounded-md border border-dashed border-gray-200 dark:border-gray-700 p-3 opacity-60">
        <div className="flex items-start gap-2.5">
          <span className="shrink-0 mt-0.5 flex items-center justify-center h-5 w-5 rounded-full bg-gray-200 dark:bg-gray-700 text-[11px] font-semibold text-gray-500 dark:text-gray-400 tabular-nums">
            2
          </span>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-gray-500 dark:text-gray-400">
              Click Enable below
            </p>
            <p className="text-xs text-gray-400 dark:text-gray-500 mt-0.5">
              The default model downloads automatically (~2.5 GB).
            </p>
          </div>
        </div>
      </div>

      {/* Check Install button */}
      <button
        type="button"
        onClick={onCheckInstall}
        disabled={isChecking}
        className={cn(
          'inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md cursor-pointer',
          'transition-colors duration-150',
          'border border-gray-200 dark:border-gray-700',
          'text-gray-700 dark:text-gray-300',
          'hover:bg-gray-50 dark:hover:bg-gray-800',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2',
        )}
      >
        <RefreshCw className={cn('w-4 h-4', isChecking && 'animate-spin')} />
        {isChecking ? 'Checking...' : 'Check Installation'}
      </button>

      {/* Requirements footer */}
      <div className="flex items-center gap-1.5 text-[11px] text-gray-400 dark:text-gray-500">
        <span>Apple Silicon</span>
        <span className="text-gray-300 dark:text-gray-600">·</span>
        <span>4 GB RAM</span>
        <span className="text-gray-300 dark:text-gray-600">·</span>
        <span>2.5 GB disk</span>
      </div>
    </div>
  )
}
