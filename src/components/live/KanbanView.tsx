import { useMemo } from 'react'
import { Columns3 } from 'lucide-react'
import type { LiveSession } from '../../hooks/use-live-sessions'
import type { LiveDisplayStatus } from '../../types/live'
import { toDisplayStatus } from '../../types/live'
import { KanbanColumn } from './KanbanColumn'

interface KanbanViewProps {
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
}

const COLUMNS: {
  title: string
  status: LiveDisplayStatus
  accentColor: string
  emptyMessage: string
}[] = [
  {
    title: 'Working',
    status: 'working',
    accentColor: 'bg-green-500',
    emptyMessage: 'No active sessions',
  },
  {
    title: 'Waiting',
    status: 'waiting',
    accentColor: 'bg-amber-500',
    emptyMessage: 'No sessions waiting',
  },
  {
    title: 'Idle',
    status: 'idle',
    accentColor: 'bg-gray-500',
    emptyMessage: 'All sessions active',
  },
  {
    title: 'Done',
    status: 'done',
    accentColor: 'bg-blue-500',
    emptyMessage: 'No completed sessions',
  },
]

export function KanbanView({ sessions, selectedId, onSelect }: KanbanViewProps) {
  const grouped = useMemo(() => {
    const groups: Record<LiveDisplayStatus, LiveSession[]> = {
      working: [],
      waiting: [],
      idle: [],
      done: [],
    }
    for (const s of sessions) {
      groups[toDisplayStatus(s.status)].push(s)
    }
    // Sort within each group by lastActivityAt descending
    for (const arr of Object.values(groups)) {
      arr.sort((a, b) => b.lastActivityAt - a.lastActivityAt)
    }
    return groups
  }, [sessions])

  if (sessions.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-gray-400 dark:text-gray-500">
        <Columns3 className="h-10 w-10 mb-3 text-gray-300 dark:text-gray-600" />
        <p className="text-sm font-medium text-gray-500 dark:text-gray-400">
          No active sessions detected
        </p>
        <p className="text-xs mt-1">
          Start a Claude Code session in your terminal
        </p>
      </div>
    )
  }

  return (
    <div className="flex gap-4 overflow-x-auto pb-4">
      {COLUMNS.map((col) => (
        <KanbanColumn
          key={col.status}
          title={col.title}
          status={col.status}
          sessions={grouped[col.status]}
          accentColor={col.accentColor}
          selectedId={selectedId}
          onSelect={onSelect}
          emptyMessage={col.emptyMessage}
        />
      ))}
    </div>
  )
}
