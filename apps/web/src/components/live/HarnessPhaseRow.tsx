import { cn } from '../../lib/utils'
import type { PhaseColumn } from './harness-phase-groups'
import { SessionCard } from './SessionCard'
import type { LiveSession } from './use-live-sessions'

export function HarnessPhaseColumnHeaders({
  phases,
  counts,
}: {
  phases: readonly PhaseColumn[]
  counts: Record<string, number>
}) {
  return (
    <div className="flex gap-3 px-4 py-2">
      {phases.map(({ phase, label, emoji, stripe }) => (
        <div key={phase} className="flex-1 min-w-0">
          <div className="relative bg-gray-50/50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-800">
            <div className={cn('h-0.5 rounded-t-lg', stripe)} />
            <div className="px-3 py-1.5 flex items-center gap-1.5">
              <span className="text-xs">{emoji}</span>
              <span className="text-xs font-medium text-gray-600 dark:text-gray-400">{label}</span>
              <span className="text-xs text-gray-400 dark:text-gray-500">
                ({counts[phase] ?? 0})
              </span>
            </div>
          </div>
        </div>
      ))}
    </div>
  )
}

export function HarnessPhaseRow({
  phases,
  byPhase,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
  hideProjectBranch,
}: {
  phases: readonly PhaseColumn[]
  byPhase: Record<string, LiveSession[]>
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
  hideProjectBranch: boolean
}) {
  return (
    <div className="flex gap-3 px-3 py-2">
      {phases.map(({ phase }) => (
        <div key={phase} className="flex-1 min-w-0">
          <div className="space-y-2">
            {(byPhase[phase]?.length ?? 0) === 0 ? (
              <div className="text-center text-gray-300 dark:text-gray-700 py-4 text-xs">
                {'\u2014'}
              </div>
            ) : (
              byPhase[phase].map((session) => (
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
                    hideProjectBranch={hideProjectBranch}
                  />
                </div>
              ))
            )}
          </div>
        </div>
      ))}
    </div>
  )
}
