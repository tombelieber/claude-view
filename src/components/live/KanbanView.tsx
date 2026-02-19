import { useMemo } from 'react'
import type { LiveSession } from './use-live-sessions'
import type { AgentStateGroup } from './types'
import { KanbanColumn } from './KanbanColumn'

interface KanbanViewProps {
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
}

const COLUMNS: {
  title: string
  group: AgentStateGroup
  accentColor: string
  emptyMessage: string
}[] = [
  {
    title: 'Needs You',
    group: 'needs_you',
    accentColor: 'bg-amber-500',
    emptyMessage: 'No sessions need attention',
  },
  {
    title: 'Running',
    group: 'autonomous',
    accentColor: 'bg-green-500',
    emptyMessage: 'No autonomous sessions',
  },
]

/** Sort key for needs_you sessions: urgency ordering */
export function needsYouSortKey(session: LiveSession): number {
  switch (session.agentState.state) {
    case 'needs_permission': return 0
    case 'awaiting_input': return 1
    case 'interrupted': return 2
    case 'error': return 3
    case 'awaiting_approval': return 4
    case 'idle': return 5
    default: return 6
  }
}

export function KanbanView({ sessions, selectedId, onSelect, stalledSessions, currentTime, onCardClick }: KanbanViewProps) {
  const grouped = useMemo(() => {
    const groups: Record<AgentStateGroup, LiveSession[]> = {
      needs_you: [],
      autonomous: [],
    }
    for (const s of sessions) {
      groups[s.agentState.group].push(s)
    }
    // Sort by last user input timestamp (stack order: most recent user prompt on top).
    // This avoids re-sorting on every agent activity update — only user messages move cards.
    groups.autonomous.sort((a, b) => {
      const aTime = a.currentTurnStartedAt ?? a.lastActivityAt
      const bTime = b.currentTurnStartedAt ?? b.lastActivityAt
      return bTime - aTime
    })
    groups.needs_you.sort((a, b) => {
      // Warm sessions first; 'unknown' sorts between warm and cold
      const cacheRank = (s: LiveSession) =>
        s.cacheStatus === 'warm' ? 0 : s.cacheStatus === 'unknown' ? 1 : 2
      const cacheDiff = cacheRank(a) - cacheRank(b)
      if (cacheDiff !== 0) return cacheDiff

      // Within same cache tier: sort by urgency then recency
      const keyDiff = needsYouSortKey(a) - needsYouSortKey(b)
      if (keyDiff !== 0) return keyDiff
      return b.lastActivityAt - a.lastActivityAt
    })
    return groups
  }, [sessions])

  if (sessions.length === 0) {
    return null
  }

  return (
    <div className="flex gap-4 pb-4 h-full min-h-0">
      {COLUMNS.map((col) => (
        <KanbanColumn
          key={col.group}
          title={col.title}
          group={col.group}
          sessions={grouped[col.group]}
          accentColor={col.accentColor}
          selectedId={selectedId}
          onSelect={onSelect}
          emptyMessage={col.emptyMessage}
          stalledSessions={stalledSessions}
          currentTime={currentTime}
          onCardClick={onCardClick}
        />
      ))}
    </div>
  )
}
