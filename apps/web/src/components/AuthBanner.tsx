import { Check, Copy } from 'lucide-react'
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { useSystem } from '../hooks/use-system'
import { Banner } from './ui/Banner'

const COMMAND = 'claude auth login'

export function AuthBanner() {
  const { data, isLoading } = useSystem()
  const [copied, setCopied] = useState(false)

  if (isLoading) return null

  const cli = data?.claudeCli
  const notInstalled = !cli?.path
  const notAuthenticated = cli?.path && !cli.authenticated

  if (!notInstalled && !notAuthenticated) return null

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(COMMAND)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {
      // Clipboard API may fail in insecure contexts
    }
  }

  const message = notInstalled
    ? 'Claude CLI is not installed. Classification and provider features require it.'
    : 'Claude CLI is not signed in. Run the command below to authenticate.'

  return (
    <Banner variant={notInstalled ? 'error' : 'warning'} layout="bar">
      <div className="flex items-center gap-3 flex-wrap">
        <p className="flex-1 min-w-0">{message}</p>

        {!notInstalled && (
          <div className="flex items-center gap-2">
            <code className="text-xs font-mono px-2 py-1 rounded bg-amber-100 dark:bg-amber-900/40 text-amber-800 dark:text-amber-200">
              {COMMAND}
            </code>
            <button
              type="button"
              onClick={handleCopy}
              className="p-1 rounded transition-colors cursor-pointer text-amber-600 hover:text-amber-800 dark:text-amber-400 dark:hover:text-amber-200"
              aria-label={copied ? 'Copied' : 'Copy command'}
            >
              {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
            </button>
          </div>
        )}

        <Link
          to="/settings?provider=show"
          className={`text-xs font-medium whitespace-nowrap underline underline-offset-2 ${
            notInstalled
              ? 'text-red-600 hover:text-red-800 dark:text-red-400 dark:hover:text-red-200'
              : 'text-amber-600 hover:text-amber-800 dark:text-amber-400 dark:hover:text-amber-200'
          }`}
        >
          Settings
        </Link>
      </div>
    </Banner>
  )
}
