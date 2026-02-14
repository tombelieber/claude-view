import { Link } from 'react-router-dom'
import { CheckCircle2, AlertCircle } from 'lucide-react'
import { useSystem } from '../hooks/use-system'

export function AuthPill() {
  const { data, isLoading } = useSystem()

  if (isLoading) {
    return (
      <span className="inline-block w-14 h-5 rounded-full bg-gray-200 dark:bg-gray-700 animate-pulse" />
    )
  }

  const cli = data?.claudeCli

  // CLI not installed
  if (!cli?.path) {
    return (
      <Link
        to="/settings?provider=show"
        className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400 hover:opacity-80 transition-opacity"
        title="Claude CLI not found"
      >
        <AlertCircle className="w-3 h-3" />
        CLI Missing
      </Link>
    )
  }

  // Not authenticated
  if (!cli.authenticated) {
    return (
      <Link
        to="/settings?provider=show"
        className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400 hover:opacity-80 transition-opacity"
        title="Claude CLI not signed in"
      >
        <AlertCircle className="w-3 h-3" />
        Not Signed In
      </Link>
    )
  }

  // Authenticated — show tier or fallback
  const tier = cli.subscriptionType
  const label = tier
    ? tier.charAt(0).toUpperCase() + tier.slice(1).toLowerCase()
    : 'CLI \u2713'

  return (
    <Link
      to="/settings?provider=show"
      className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400 hover:opacity-80 transition-opacity"
      title={`Claude CLI: ${tier ?? 'authenticated'}`}
    >
      <CheckCircle2 className="w-3 h-3" />
      {label}
    </Link>
  )
}
