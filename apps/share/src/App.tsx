import * as Sentry from '@sentry/react'
import { useEffect, useState } from 'react'
import { decryptShareBlob } from './crypto'

const WORKER_URL = import.meta.env.VITE_WORKER_URL || 'https://claude-view-share.workers.dev'

Sentry.init({
  dsn: import.meta.env.VITE_SENTRY_DSN,
  enabled: import.meta.env.PROD,
})

export default function App() {
  const token = window.location.pathname.split('/s/')[1]?.split('#')[0]
  const hash = window.location.hash.slice(1)
  const keyBase64url = new URLSearchParams(hash).get('k')

  const [session, setSession] = useState<unknown>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    if (!token) {
      setError('No share token in URL')
      setLoading(false)
      return
    }
    if (!keyBase64url) {
      setError('No decryption key in URL fragment. Was the link truncated?')
      setLoading(false)
      return
    }

    const start = Date.now()

    fetch(`${WORKER_URL}/api/share/${token}`)
      .then(async (res) => {
        if (!res.ok)
          throw new Error(
            res.status === 404 ? 'Share not found or has been revoked.' : 'Failed to load share.',
          )
        return res.arrayBuffer()
      })
      .then((blob) => decryptShareBlob(blob, keyBase64url))
      .then((data) => {
        setSession(data)
        // eslint-disable-next-line @typescript-eslint/no-explicit-any -- PostHog snippet loaded externally
        const win = window as unknown as Record<string, any>
        if (typeof win.posthog?.capture === 'function') {
          win.posthog.capture('share_decrypt_success', {
            duration_ms: Date.now() - start,
          })
        }
      })
      .catch((err: unknown) => {
        Sentry.captureException(err)
        setError(err instanceof Error ? err.message : 'Failed to decrypt share.')
      })
      .finally(() => setLoading(false))
  }, [token, keyBase64url])

  if (loading) {
    return (
      <div className="min-h-screen bg-white dark:bg-gray-950 flex items-center justify-center">
        <div className="text-gray-500 dark:text-gray-400 text-sm">Decrypting conversation...</div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="min-h-screen bg-white dark:bg-gray-950 flex items-center justify-center">
        <div className="text-center">
          <p className="text-red-600 dark:text-red-400 text-sm mb-2">{error}</p>
          <a
            href="https://claudeview.ai"
            className="text-gray-500 dark:text-gray-400 text-xs hover:text-gray-700 dark:hover:text-gray-300"
          >
            What is claude-view?
          </a>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-white dark:bg-gray-950 text-gray-900 dark:text-gray-100">
      <header className="border-b border-gray-200 dark:border-gray-800 px-6 py-3 flex items-center justify-between">
        <div className="text-sm text-gray-500 dark:text-gray-400">
          Shared via{' '}
          <a
            href="https://claudeview.ai"
            className="text-gray-900 dark:text-white font-medium hover:underline"
          >
            claude-view
          </a>
        </div>
        <a
          href="https://claudeview.ai"
          className="text-sm text-blue-600 dark:text-blue-400 hover:text-blue-500 dark:hover:text-blue-300"
        >
          Get claude-view
        </a>
      </header>
      <main className="max-w-4xl mx-auto py-8 px-4">
        {/* Phase 4 MVP: raw JSON preview. Follow-up task: render session.messages
            using extracted components from @web (shared via @claude-view/shared). */}
        <pre className="text-xs text-gray-600 dark:text-gray-400 overflow-auto">
          {JSON.stringify(session, null, 2).slice(0, 2000)}
        </pre>
      </main>
    </div>
  )
}
