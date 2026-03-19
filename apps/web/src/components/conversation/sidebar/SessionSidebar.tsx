import type { AvailableSession } from '@claude-view/shared'
import type { LiveSession } from '@claude-view/shared/types/generated'
import { PenSquare, Search } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { toast } from 'sonner'
import { TOAST_DURATION } from '../../../lib/notify'
import { SessionListItem } from './SessionListItem'

type EnrichedSession = AvailableSession & {
  isActive?: boolean
  liveData?: LiveSession | null
  isSidecarManaged?: boolean
}

interface SessionSidebarProps {
  liveSessions: LiveSession[]
  /** Session IDs actively managed by the sidecar (from /control/sessions). */
  sidecarSessionIds?: Set<string>
}

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

export function SessionSidebar({ liveSessions, sidecarSessionIds }: SessionSidebarProps) {
  const navigate = useNavigate()
  const { sessionId: currentSessionId } = useParams<{ sessionId?: string }>()

  const VISIBLE_BATCH = 30

  const [historySessions, setHistorySessions] = useState<AvailableSession[]>([])
  const [searchQuery, setSearchQuery] = useState('')
  const [loading, setLoading] = useState(true)
  const [visibleCount, setVisibleCount] = useState(VISIBLE_BATCH)

  // Active sessions: all live sessions that are working, paused, or SDK-controlled
  const activeSessions = useMemo(
    () =>
      liveSessions.filter(
        (s) => s.status === 'working' || s.status === 'paused' || s.control !== null,
      ),
    [liveSessions],
  )

  const activeSessionIds = useMemo(() => new Set(activeSessions.map((s) => s.id)), [activeSessions])

  // Fetch history sessions on mount
  useEffect(() => {
    let cancelled = false
    async function fetchHistory() {
      try {
        const res = await fetch('/api/sessions')
        if (cancelled) return
        if (res.ok) setHistorySessions(await res.json())
      } catch {
        // Network error — silently fail, show empty state
      } finally {
        if (!cancelled) setLoading(false)
      }
    }
    fetchHistory()
    return () => {
      cancelled = true
    }
  }, [])

  // Refetch history when liveSessions count changes (session created/closed)
  const prevLiveCount = useRef(liveSessions.length)
  useEffect(() => {
    if (prevLiveCount.current === liveSessions.length) return
    prevLiveCount.current = liveSessions.length
    fetch('/api/sessions')
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => {
        if (data) setHistorySessions(data)
      })
      .catch(() => {})
  }, [liveSessions.length])

  // Merge: mark history sessions that are also active
  const enrichedHistory = useMemo(() => {
    return historySessions.map((s) => ({
      ...s,
      isActive: activeSessionIds.has(s.sessionId),
      liveData: activeSessions.find((a) => a.id === s.sessionId) ?? null,
      isSidecarManaged: sidecarSessionIds?.has(s.sessionId) ?? false,
    }))
  }, [historySessions, activeSessionIds, activeSessions, sidecarSessionIds])

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

  // Reset visibleCount when search query changes.
  // searchQuery is intentionally listed as the dependency to trigger on change,
  // even though it is not read inside the effect body.
  // biome-ignore lint/correctness/useExhaustiveDependencies: searchQuery triggers the reset
  useEffect(() => {
    setVisibleCount(VISIBLE_BATCH)
  }, [searchQuery])

  const now = Math.floor(Date.now() / 1000)
  const hasMore = visibleCount < filteredRest.length
  const visibleTimeGroups = useMemo(
    () => groupByTime(filteredRest.slice(0, visibleCount), now),
    [filteredRest, visibleCount, now],
  )

  const loadMoreRef = useRef<HTMLDivElement>(null)
  const scrollContainerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const sentinel = loadMoreRef.current
    const container = scrollContainerRef.current
    if (!sentinel || !container || !hasMore) return

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setVisibleCount((prev) => prev + VISIBLE_BATCH)
        }
      },
      { root: container, threshold: 0.1 },
    )
    observer.observe(sentinel)
    return () => observer.disconnect()
  }, [hasMore])

  // Flatten all visible sessions into a single ordered list for keyboard nav
  const flatSessions = useMemo(() => {
    const list: EnrichedSession[] = [...pinnedSessions]
    for (const group of visibleTimeGroups) {
      for (const s of group.sessions) {
        const enriched = enrichedHistory.find((e) => e.sessionId === s.sessionId)
        if (enriched) list.push(enriched)
      }
    }
    return list
  }, [pinnedSessions, visibleTimeGroups, enrichedHistory])

  const [activeNavIndex, setActiveNavIndex] = useState(-1)
  const itemRefs = useRef<Map<string, HTMLDivElement>>(new Map())

  // Reset nav index only when search query changes (user intent changed).
  // Do NOT reset on flatSessions.length change — that happens during
  // infinite scroll load-more which must preserve the current position.
  // biome-ignore lint/correctness/useExhaustiveDependencies: searchQuery triggers the reset
  useEffect(() => {
    setActiveNavIndex(-1)
  }, [searchQuery])

  // Debounced navigation: highlight moves instantly, but the expensive
  // navigate() (which triggers JSONL fetch + WS connect + rich data fetch)
  // only fires after 200ms of no arrow presses — same pattern as VS Code
  // file explorer and Slack channel list.
  const navTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined)
  const debouncedNavigate = useCallback(
    (sessionId: string) => {
      clearTimeout(navTimerRef.current)
      navTimerRef.current = setTimeout(() => {
        navigate(`/chat/${sessionId}`)
      }, 200)
    },
    [navigate],
  )
  // Cleanup on unmount
  useEffect(() => () => clearTimeout(navTimerRef.current), [])

  // Keyboard handler for arrow nav — no wrapping (clamped at top/bottom)
  // Highlight moves instantly; navigation is debounced to avoid flooding fetches
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (flatSessions.length === 0) return
      const tag = (e.target as HTMLElement)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA') return

      if (e.key === 'ArrowDown' || e.key === 'j') {
        e.preventDefault()
        setActiveNavIndex((prev) => {
          const next = Math.min(prev + 1, flatSessions.length - 1)
          const session = flatSessions[next]
          if (session) {
            debouncedNavigate(session.sessionId)
            itemRefs.current.get(session.sessionId)?.scrollIntoView({ block: 'nearest' })
          }
          // Near the bottom — proactively load more sessions
          if (next >= flatSessions.length - 3 && hasMore) {
            setVisibleCount((v) => v + VISIBLE_BATCH)
          }
          return next
        })
      } else if (e.key === 'ArrowUp' || e.key === 'k') {
        e.preventDefault()
        setActiveNavIndex((prev) => {
          if (prev <= 0) return 0
          const next = prev - 1
          const session = flatSessions[next]
          if (session) {
            debouncedNavigate(session.sessionId)
            itemRefs.current.get(session.sessionId)?.scrollIntoView({ block: 'nearest' })
          }
          return next
        })
      }
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [flatSessions, debouncedNavigate, hasMore])

  // Callback to register item refs
  const setItemRef = useCallback((sessionId: string, el: HTMLDivElement | null) => {
    if (el) itemRefs.current.set(sessionId, el)
    else itemRefs.current.delete(sessionId)
  }, [])

  const handleSelect = useCallback(
    (id: string) => {
      // Update nav index to match clicked item
      const idx = flatSessions.findIndex((s) => s.sessionId === id)
      setActiveNavIndex(idx)
      navigate(`/chat/${id}`)
    },
    [navigate, flatSessions],
  )
  const handleNewChat = useCallback(() => navigate('/chat'), [navigate])

  const handleResume = useCallback(
    async (sessionId: string) => {
      navigate(`/chat/${sessionId}`, { state: { resumed: true } })
    },
    [navigate],
  )

  const handleFork = useCallback(
    async (sessionId: string) => {
      try {
        const session = enrichedHistory.find((s) => s.sessionId === sessionId)
        const res = await fetch(`/api/sessions/${sessionId}/fork`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ projectPath: session?.cwd }),
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
    [enrichedHistory, navigate],
  )

  return (
    <nav
      aria-label="Chat history"
      className="flex flex-col h-full w-64 border-r border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-950"
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-3 border-b border-gray-200 dark:border-gray-800">
        <span className="text-sm font-semibold text-gray-700 dark:text-gray-200">Chats</span>
        <button
          type="button"
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
      <div ref={scrollContainerRef} className="flex-1 overflow-y-auto px-2 pb-2">
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
                {pinnedSessions.map((s) => {
                  const idx = flatSessions.findIndex((f) => f.sessionId === s.sessionId)
                  return (
                    <SessionListItem
                      key={s.sessionId}
                      ref={(el) => setItemRef(s.sessionId, el)}
                      session={s}
                      isSelected={s.sessionId === currentSessionId}
                      isKeyboardActive={idx === activeNavIndex}
                      onSelect={handleSelect}
                      onResume={handleResume}
                      onFork={handleFork}
                    />
                  )
                })}
              </div>
            )}

            {/* Time-grouped history */}
            {visibleTimeGroups.map((group) => (
              <div key={group.label} className="mb-2">
                <p className="px-3 py-1 text-xs font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wide">
                  {group.label}
                </p>
                {group.sessions.map((s) => {
                  const enriched = enrichedHistory.find((e) => e.sessionId === s.sessionId) ?? {
                    ...s,
                    isActive: false,
                  }
                  const idx = flatSessions.findIndex((f) => f.sessionId === s.sessionId)
                  return (
                    <SessionListItem
                      key={s.sessionId}
                      ref={(el) => setItemRef(s.sessionId, el)}
                      session={enriched}
                      isSelected={s.sessionId === currentSessionId}
                      isKeyboardActive={idx === activeNavIndex}
                      onSelect={handleSelect}
                      onResume={handleResume}
                      onFork={handleFork}
                    />
                  )
                })}
              </div>
            ))}

            {/* Load-more sentinel */}
            {hasMore && (
              <div ref={loadMoreRef} className="flex justify-center py-3">
                <div className="h-4 w-4 animate-spin rounded-full border-2 border-gray-300 border-t-blue-400" />
              </div>
            )}

            {!loading && enrichedHistory.length === 0 && (
              <div className="px-3 py-8 text-center text-sm text-gray-400">
                No sessions yet. Start a new chat!
              </div>
            )}
          </>
        )}
      </div>
    </nav>
  )
}
