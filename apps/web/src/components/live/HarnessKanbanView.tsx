import { useMemo } from 'react'
import { cn } from '../../lib/utils'
import type { SessionPhase } from '../../types/generated/SessionPhase'
import { SessionCard } from './SessionCard'
import type { LiveSession } from './use-live-sessions'

/** Displayable phases -- excludes 'working' which is the unclassified fallback */
type DisplayPhase = Exclude<SessionPhase, 'working'>

const PHASE_COLUMNS: { phase: DisplayPhase; label: string; emoji: string; stripe: string }[] = [
  { phase: 'thinking', label: 'Thinking', emoji: '\u{1F4AD}', stripe: 'bg-purple-500' },
  { phase: 'planning', label: 'Planning', emoji: '\u{1F4CB}', stripe: 'bg-blue-500' },
  { phase: 'building', label: 'Building', emoji: '\u{1F528}', stripe: 'bg-orange-500' },
  { phase: 'testing', label: 'Testing', emoji: '\u{1F9EA}', stripe: 'bg-green-500' },
  { phase: 'reviewing', label: 'Reviewing', emoji: '\u{1F50D}', stripe: 'bg-cyan-500' },
  { phase: 'shipping', label: 'Shipping', emoji: '\u{1F680}', stripe: 'bg-red-500' },
]

interface HarnessKanbanViewProps {
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
}

function getSessionPhase(session: LiveSession): DisplayPhase {
  const phase = session.phase?.current?.phase
  if (!phase || phase === 'working') return 'building'
  return phase
}

export function HarnessKanbanView({
  sessions,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
}: HarnessKanbanViewProps) {
  const columns = useMemo(() => {
    const grouped: Record<DisplayPhase, LiveSession[]> = {
      thinking: [],
      planning: [],
      building: [],
      testing: [],
      reviewing: [],
      shipping: [],
    }
    for (const s of sessions) {
      const phase = getSessionPhase(s)
      grouped[phase].push(s)
    }
    // Sort: needs_you first, then by last activity descending
    for (const key of Object.keys(grouped) as DisplayPhase[]) {
      grouped[key].sort((a, b) => {
        const aGroup = a.agentState.group === 'needs_you' ? 0 : 1
        const bGroup = b.agentState.group === 'needs_you' ? 0 : 1
        if (aGroup !== bGroup) return aGroup - bGroup
        return b.lastActivityAt - a.lastActivityAt
      })
    }
    return grouped
  }, [sessions])

  return (
    <div className="flex flex-col h-full min-h-0 pb-4">
      {/* Column headers */}
      <div className="flex gap-3 shrink-0 mb-3">
        {PHASE_COLUMNS.map(({ phase, label, emoji, stripe }) => (
          <div key={phase} className="flex-1 min-w-0">
            <div className="relative bg-gray-50/50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-800">
              <div className={cn('h-0.5 rounded-t-lg', stripe)} />
              <div className="px-3 py-2 flex items-center gap-1.5">
                <span className="text-sm">{emoji}</span>
                <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                  {label}
                </span>
                <span className="text-xs text-gray-400 dark:text-gray-500">
                  ({columns[phase].length})
                </span>
              </div>
            </div>
          </div>
        ))}
      </div>

      {/* Scrollable columns */}
      <div className="flex-1 min-h-0 flex gap-3 overflow-hidden">
        {PHASE_COLUMNS.map(({ phase }) => (
          <div key={phase} className="flex-1 min-w-0 overflow-y-auto pr-1">
            <div className="flex flex-col gap-3">
              {columns[phase].length === 0 ? (
                <div className="text-center text-gray-300 dark:text-gray-700 py-8 text-xs">
                  {'\u2014'}
                </div>
              ) : (
                columns[phase].map((session) => (
                  <div
                    key={session.id}
                    role="button"
                    tabIndex={0}
                    data-session-id={session.id}
                    onClick={() => onSelect(session.id)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault()
                        onSelect(session.id)
                      }
                    }}
                    className={cn(
                      'cursor-pointer rounded-lg transition-opacity',
                      session.id === selectedId && 'ring-2 ring-indigo-500 rounded-lg',
                    )}
                  >
                    <SessionCard
                      session={session}
                      stalledSessions={stalledSessions}
                      currentTime={currentTime}
                      onClickOverride={onCardClick ? () => onCardClick(session.id) : undefined}
                      showStateBadge
                    />
                  </div>
                ))
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
