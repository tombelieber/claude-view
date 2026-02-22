import { useState, useEffect, useMemo, useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { Wifi, WifiOff, LogOut, Loader, AlertTriangle, Inbox } from 'lucide-react'
import { useMobileRelay } from '../hooks/use-mobile-relay.ts'
import { getItem, removeItem } from '../lib/mobile-storage.ts'
import { MobileSessionCard } from '../components/mobile/MobileSessionCard.tsx'
import { MobileSessionDetail } from '../components/mobile/MobileSessionDetail.tsx'
import type { LiveSession } from '../components/live/use-live-sessions.ts'

/** Shown when phone is paired and connected to relay. */
export function MobileMonitorPageMobile() {
  const navigate = useNavigate()
  const [relayUrl, setRelayUrl] = useState<string | null>(null)
  const [loadingRelay, setLoadingRelay] = useState(true)
  const [selectedSession, setSelectedSession] = useState<LiveSession | null>(null)
  const [detailOpen, setDetailOpen] = useState(false)

  // Load relay URL from IndexedDB on mount
  useEffect(() => {
    let cancelled = false
    async function load() {
      const url = await getItem('relay_url')
      if (!cancelled) {
        setRelayUrl(url)
        setLoadingRelay(false)
        // If no relay URL, redirect to pairing
        if (!url) {
          navigate('/mobile', { replace: true })
        }
      }
    }
    void load()
    return () => { cancelled = true }
  }, [navigate])

  const { sessions, isConnected, error } = useMobileRelay(relayUrl)

  const sessionList = useMemo(
    () => Array.from(sessions.values()).sort((a, b) => b.lastActivityAt - a.lastActivityAt),
    [sessions],
  )

  const handleDisconnect = useCallback(async () => {
    await removeItem('relay_url')
    await removeItem('device_id')
    await removeItem('enc_secret')
    await removeItem('enc_public')
    await removeItem('sign_secret')
    await removeItem('sign_public')
    await removeItem('mac_enc_public')
    navigate('/mobile', { replace: true })
  }, [navigate])

  const handleCardClick = useCallback((session: LiveSession) => {
    setSelectedSession(session)
    setDetailOpen(true)
  }, [])

  const handleDetailClose = useCallback(() => {
    setDetailOpen(false)
  }, [])

  if (loadingRelay) {
    return (
      <div className="flex-1 flex flex-col bg-gray-950">
        <div className="flex-1 flex items-center justify-center">
          <Loader className="w-8 h-8 text-gray-500 animate-spin" />
        </div>
      </div>
    )
  }

  return (
    <div className="flex-1 flex flex-col bg-gray-950">
      {/* Header */}
      <header className="h-14 flex items-center justify-between px-4 border-b border-gray-800 flex-shrink-0">
        <h1 className="text-lg font-semibold text-gray-100">Claude Sessions</h1>
        <div className="flex items-center gap-3">
          {/* Connection indicator */}
          <div className="flex items-center gap-1.5">
            {isConnected ? (
              <>
                <Wifi className="w-4 h-4 text-green-400" />
                <span className="text-xs text-green-400">Live</span>
              </>
            ) : error ? (
              <>
                <WifiOff className="w-4 h-4 text-red-400" />
                <span className="text-xs text-red-400">Error</span>
              </>
            ) : (
              <>
                <Loader className="w-4 h-4 text-gray-400 animate-spin" />
                <span className="text-xs text-gray-400">Connecting</span>
              </>
            )}
          </div>

          {/* Disconnect button */}
          <button
            type="button"
            onClick={() => void handleDisconnect()}
            className="w-11 h-11 flex items-center justify-center rounded-lg hover:bg-gray-800 active:bg-gray-700 cursor-pointer"
            aria-label="Disconnect and unpair"
            title="Disconnect"
          >
            <LogOut className="w-5 h-5 text-gray-400" />
          </button>
        </div>
      </header>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {/* Error banner */}
        {error && (
          <div className="mx-4 mt-4 flex items-center gap-2 bg-red-900/30 border border-red-800 rounded-lg px-3 py-2.5">
            <AlertTriangle className="w-4 h-4 text-red-400 flex-shrink-0" />
            <p className="text-xs text-red-300">{error}</p>
          </div>
        )}

        {/* Session list */}
        {sessionList.length > 0 ? (
          <div className="px-4 py-4 flex flex-col gap-3">
            {sessionList.map((session) => (
              <MobileSessionCard
                key={session.id}
                session={session}
                onClick={() => handleCardClick(session)}
              />
            ))}
          </div>
        ) : isConnected ? (
          /* Empty state — connected but no sessions */
          <div className="flex-1 flex flex-col items-center justify-center py-20">
            <Inbox className="w-12 h-12 text-gray-600 mb-4" />
            <p className="text-gray-500 text-sm">No active sessions</p>
            <p className="text-gray-600 text-xs mt-1">Sessions will appear when Claude is running</p>
          </div>
        ) : !error ? (
          /* Connecting state */
          <div className="flex-1 flex flex-col items-center justify-center py-20">
            <Loader className="w-8 h-8 text-gray-500 animate-spin mb-4" />
            <p className="text-gray-500 text-sm">Connecting to relay...</p>
          </div>
        ) : null}
      </div>

      {/* Session detail bottom sheet */}
      <MobileSessionDetail
        session={selectedSession}
        open={detailOpen}
        onClose={handleDetailClose}
      />
    </div>
  )
}
