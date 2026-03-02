import { SessionInfoPanel, ViewModeToggle } from '@claude-view/shared'
import type { SharePayload } from '@claude-view/shared/types/message'
import * as Sentry from '@sentry/react'
import { PanelRight } from 'lucide-react'
import posthog from 'posthog-js'
import { useEffect, useState } from 'react'
import { SharedConversationView } from './SharedConversationView'
import { decryptShareBlob } from './crypto'

const WORKER_URL = import.meta.env.VITE_WORKER_URL || 'https://api-share.claudeview.ai'

Sentry.init({
  dsn: import.meta.env.VITE_SENTRY_DSN,
  enabled: import.meta.env.PROD,
})

if (import.meta.env.PROD && import.meta.env.VITE_POSTHOG_KEY) {
  posthog.init(import.meta.env.VITE_POSTHOG_KEY, {
    api_host: 'https://us.i.posthog.com',
  })
}

export default function App() {
  const token = window.location.pathname.split('/s/')[1]?.split('#')[0]
  const hash = window.location.hash.slice(1)
  const keyBase64url = new URLSearchParams(hash).get('k')

  const [session, setSession] = useState<SharePayload | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [verboseMode, setVerboseMode] = useState(false)
  const [panelOpen, setPanelOpen] = useState(false)

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
        setSession(data as SharePayload)
        posthog.capture('share_decrypt_success', {
          duration_ms: Date.now() - start,
        })
      })
      .catch((err: unknown) => {
        Sentry.captureException(err)
        setError(err instanceof Error ? err.message : 'Failed to decrypt share.')
      })
      .finally(() => setLoading(false))
  }, [token, keyBase64url])

  // Show panel by default on desktop when shareMetadata exists
  useEffect(() => {
    if (session?.shareMetadata && window.innerWidth >= 1024) {
      setPanelOpen(true)
    }
  }, [session?.shareMetadata])

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
      {/* Sticky header with backdrop-blur */}
      <header className="sticky top-0 z-10 border-b border-gray-200 dark:border-gray-800 bg-white/80 dark:bg-gray-950/80 backdrop-blur-sm px-4 py-2.5 flex items-center justify-between">
        {/* Left: branding */}
        <div className="flex items-center gap-2 text-sm">
          <span className="font-semibold text-gray-900 dark:text-white">claude-view</span>
          <span className="text-gray-400 dark:text-gray-500">Shared conversation</span>
        </div>

        {/* Center: view mode toggle */}
        <ViewModeToggle
          verboseMode={verboseMode}
          onToggleVerbose={() => setVerboseMode((v) => !v)}
        />

        {/* Right: panel toggle + CTA */}
        <div className="flex items-center gap-2">
          {session?.shareMetadata && (
            <button
              type="button"
              onClick={() => setPanelOpen((v) => !v)}
              className="p-1.5 rounded-md text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors cursor-pointer"
              title={panelOpen ? 'Hide session info' : 'Show session info'}
            >
              <PanelRight className="w-4 h-4" />
            </button>
          )}
          <a
            href="https://claudeview.ai"
            target="_blank"
            rel="noopener noreferrer"
            className="text-xs font-medium px-3 py-1.5 rounded-md bg-gray-900 dark:bg-white text-white dark:text-gray-900 hover:bg-gray-700 dark:hover:bg-gray-200 transition-colors"
          >
            Get claude-view &rarr;
          </a>
        </div>
      </header>

      {/* Main content: two-column on desktop */}
      <div className="flex">
        <main className="flex-1 min-w-0 py-8 px-4">
          {session && (
            <SharedConversationView messages={session.messages} verboseMode={verboseMode} />
          )}
        </main>

        {/* Collapsible side panel */}
        {panelOpen && session?.shareMetadata && (
          <aside className="hidden lg:block w-72 shrink-0 border-l border-gray-200 dark:border-gray-800 p-4 sticky top-[49px] h-[calc(100vh-49px)] overflow-y-auto">
            <SessionInfoPanel metadata={session.shareMetadata} />
          </aside>
        )}
      </div>
    </div>
  )
}
