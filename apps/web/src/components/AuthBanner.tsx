import { useState } from 'react'
import { Link } from 'react-router-dom'
import { AlertCircle, Copy, Check } from 'lucide-react'
import { useSystem } from '../hooks/use-system'

const COMMAND = 'claude auth login'

export function AuthBanner() {
  const { data, isLoading } = useSystem()
  const [copied, setCopied] = useState(false)

  if (isLoading) return null

  const cli = data?.claudeCli
  const notInstalled = !cli?.path
  const notAuthenticated = cli?.path && !cli.authenticated

  // Only show when auth is broken
  if (!notInstalled && !notAuthenticated) return null

  const isRed = notInstalled
  const bgClass = isRed
    ? 'bg-red-50 dark:bg-red-950/40 border-red-200 dark:border-red-800'
    : 'bg-amber-50 dark:bg-amber-950/40 border-amber-200 dark:border-amber-800'
  const textClass = isRed
    ? 'text-red-700 dark:text-red-300'
    : 'text-amber-700 dark:text-amber-300'
  const iconClass = isRed
    ? 'text-red-500 dark:text-red-400'
    : 'text-amber-500 dark:text-amber-400'
  const codeClass = isRed
    ? 'bg-red-100 dark:bg-red-900/40 text-red-800 dark:text-red-200'
    : 'bg-amber-100 dark:bg-amber-900/40 text-amber-800 dark:text-amber-200'
  const btnClass = isRed
    ? 'text-red-600 hover:text-red-800 dark:text-red-400 dark:hover:text-red-200'
    : 'text-amber-600 hover:text-amber-800 dark:text-amber-400 dark:hover:text-amber-200'
  const linkClass = isRed
    ? 'text-red-600 hover:text-red-800 dark:text-red-400 dark:hover:text-red-200 underline underline-offset-2'
    : 'text-amber-600 hover:text-amber-800 dark:text-amber-400 dark:hover:text-amber-200 underline underline-offset-2'

  const message = notInstalled
    ? 'Claude CLI is not installed. Classification and provider features require it.'
    : 'Claude CLI is not signed in. Run the command below to authenticate.'

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(COMMAND)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {
      // Clipboard API may fail in insecure contexts — no-op
    }
  }

  return (
    <div
      className={`border-b px-4 py-2.5 transition-all ease-out duration-300 ${bgClass}`}
      role="alert"
    >
      <div className="max-w-5xl mx-auto flex items-center gap-3 flex-wrap">
        <AlertCircle className={`w-4 h-4 flex-shrink-0 ${iconClass}`} aria-hidden="true" />

        <p className={`text-sm flex-1 min-w-0 ${textClass}`}>
          {message}
        </p>

        {!notInstalled && (
          <div className="flex items-center gap-2">
            <code className={`text-xs font-mono px-2 py-1 rounded ${codeClass}`}>
              {COMMAND}
            </code>
            <button
              type="button"
              onClick={handleCopy}
              className={`p-1 rounded transition-colors cursor-pointer ${btnClass}`}
              aria-label={copied ? 'Copied' : 'Copy command'}
            >
              {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
            </button>
          </div>
        )}

        <Link
          to="/settings?provider=show"
          className={`text-xs font-medium whitespace-nowrap ${linkClass}`}
        >
          Settings
        </Link>
      </div>
    </div>
  )
}
