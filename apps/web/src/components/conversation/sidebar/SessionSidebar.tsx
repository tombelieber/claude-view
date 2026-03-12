import type { ActiveSession, AvailableSession } from '@claude-view/shared'
import { PenSquare, Search } from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { toast } from 'sonner'
import { TOAST_DURATION } from '../../../lib/notify'
import { SessionListItem } from './SessionListItem'

function groupByTime(sessions: AvailableSession[], now: number) {
  const today = new Date(now * 1000)
  today.setHours(0, 0, 0, 0)
  const yesterday = new Date(today)
  yesterday.setDate(yesterday.getDate() - 1)
  const lastWeek = new Date(today)
  lastWeek.setDate(lastWeek.getDate() - 7)

  const groups: { label: string; sessions: AvailableSession[] }[] = [
    { label: 'Today', sessions: [] },
    { label: 'Yesterday', sessions: [] },
    { label: 'Last 7 days', sessions: [] },
    { label: 'Older', sessions: [] },
  ]

  for (const s of sessions) {
    const ts = new Date(s.lastModified * 1000)
    if (ts >= today) groups[0].sessions.push(s)
    else if (ts >= yesterday) groups[1].sessions.push(s)
    else if (ts >= lastWeek) groups[2].sessions.push(s)
    else groups[3].sessions.push(s)
  }

  return groups.filter((g) => g.sessions.length > 0)
}

export function SessionSidebar() {
  const navigate = useNavigate()
  const { sessionId: currentSessionId } = useParams<{ sessionId?: string }>()

  const [activeSessions, setActiveSessions] = useState<ActiveSession[]>([])
  const [historySessions, setHistorySessions] = useState<AvailableSession[]>([])
  const [searchQuery, setSearchQuery] = useState('')
  const [loading, setLoading] = useState(true)

  // Fetch active and available sessions
  useEffect(() => {
    let cancelled = false
    async function fetchSessions() {
      try {
        const [activeRes, historyRes] = await Promise.all([
          fetch('/api/control/sessions'),
          fetch('/api/control/available-sessions'),
        ])
        if (cancelled) return
        if (activeRes.ok) setActiveSessions(await activeRes.json())
        if (historyRes.ok) setHistorySessions(await historyRes.json())
      } catch {
        // Network error — silently fail, show empty state
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    fetchSessions()
    const interval = setInterval(fetchSessions, 10_000) // poll every 10s
    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [])

  const activeSessionIds = useMemo(
    () => new Set(activeSessions.map((s) => s.sessionId)),
    [activeSessions],
  )

  // Merge: mark history sessions that are also active
  const enrichedHistory = useMemo(() => {
    return historySessions.map((s) => ({
      ...s,
      isActive: activeSessionIds.has(s.sessionId),
      activeInfo: activeSessions.find((a) => a.sessionId === s.sessionId),
    }))
  }, [historySessions, activeSessionIds, activeSessions])

  // Separate active-pinned from rest
  const pinnedSessions = enrichedHistory.filter((s) => s.isActive)
  const restSessions = enrichedHistory.filter((s) => !s.isActive)

  // Client-side text search
  const filteredRest = useMemo(() => {
    if (!searchQuery.trim()) return restSessions
    const q = searchQuery.toLowerCase()
    return restSessions.filter(
      (s) => s.customTitle?.toLowerCase().includes(q) || s.firstPrompt?.toLowerCase().includes(q),
    )
  }, [restSessions, searchQuery])

  const now = Math.floor(Date.now() / 1000)
  const timeGroups = useMemo(() => groupByTime(filteredRest, now), [filteredRest, now])

  const handleSelect = useCallback((id: string) => navigate(`/chat/${id}`), [navigate])
  const handleNewChat = useCallback(() => navigate('/chat'), [navigate])

  const handleResume = useCallback(
    async (sessionId: string) => {
      navigate(`/chat/${sessionId}`)
    },
    [navigate],
  )

  const handleFork = useCallback(
    async (sessionId: string) => {
      try {
        const res = await fetch('/api/control/sessions/fork', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ sessionId }),
        })
        const data = await res.json()
        if (data.sessionId) {
          toast.success('Session forked', { duration: TOAST_DURATION.micro })
          navigate(`/chat/${data.sessionId}`)
        } else {
          toast.error('Fork failed', { duration: TOAST_DURATION.extended })
        }
      } catch {
        toast.error('Failed to fork session', { duration: TOAST_DURATION.extended })
      }
    },
    [navigate],
  )

  return (
    <div className="flex flex-col h-full w-64 border-r border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-950">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-3 border-b border-gray-200 dark:border-gray-800">
        <span className="text-sm font-semibold text-gray-700 dark:text-gray-200">Chats</span>
        <button
          onClick={handleNewChat}
          className="p-1.5 rounded-md hover:bg-gray-200 dark:hover:bg-gray-800 text-gray-500 dark:text-gray-400 transition-colors"
          title="New chat"
        >
          <PenSquare size={16} />
        </button>
      </div>

      {/* Search */}
      <div className="px-3 py-2">
        <div className="flex items-center gap-2 px-2 py-1.5 rounded-md bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700">
          <Search size={13} className="text-gray-400 flex-shrink-0" />
          <input
            type="text"
            placeholder="Search chats..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="flex-1 text-sm bg-transparent outline-none text-gray-700 dark:text-gray-300 placeholder:text-gray-400"
          />
        </div>
      </div>

      {/* Session list */}
      <div className="flex-1 overflow-y-auto px-2 pb-2">
        {loading ? (
          <div className="px-3 py-8 text-center text-sm text-gray-400">Loading...</div>
        ) : (
          <>
            {/* Active sessions — pinned at top */}
            {pinnedSessions.length > 0 && (
              <div className="mb-2">
                <p className="px-3 py-1 text-xs font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wide">
                  Active
                </p>
                {pinnedSessions.map((s) => (
                  <SessionListItem
                    key={s.sessionId}
                    session={s}
                    isSelected={s.sessionId === currentSessionId}
                    onSelect={handleSelect}
                    onResume={handleResume}
                    onFork={handleFork}
                  />
                ))}
              </div>
            )}

            {/* Time-grouped history */}
            {timeGroups.map((group) => (
              <div key={group.label} className="mb-2">
                <p className="px-3 py-1 text-xs font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wide">
                  {group.label}
                </p>
                {group.sessions.map((s) => (
                  <SessionListItem
                    key={s.sessionId}
                    session={s}
                    isSelected={s.sessionId === currentSessionId}
                    onSelect={handleSelect}
                    onResume={handleResume}
                    onFork={handleFork}
                  />
                ))}
              </div>
            ))}

            {!loading && enrichedHistory.length === 0 && (
              <div className="px-3 py-8 text-center text-sm text-gray-400">
                No sessions yet. Start a new chat!
              </div>
            )}
          </>
        )}
      </div>
    </div>
  )
}
