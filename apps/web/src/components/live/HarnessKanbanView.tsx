import { useMemo } from 'react'
import { HarnessGroupedContent } from './HarnessGroupedContent'
import { HarnessPhaseColumnHeaders, HarnessPhaseRow } from './HarnessPhaseRow'
import {
  PHASE_GROUPS,
  getSessionPhase,
  isDesignPhase,
  splitByPhase,
  type PhaseColumn,
} from './harness-phase-groups'
import type { KanbanGroupBy } from './types'
import type { ProjectGroup } from './use-kanban-grouping'
import type { LiveSession } from './use-live-sessions'

interface HarnessKanbanViewProps {
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
  groupBy: KanbanGroupBy
  projectGroups: ProjectGroup[]
  isCollapsed: (key: string) => boolean
  toggleCollapse: (key: string) => void
}

export function HarnessKanbanView({
  sessions,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
  groupBy,
  projectGroups,
  isCollapsed,
  toggleCollapse,
}: HarnessKanbanViewProps) {
  const groupCounts = useMemo(() => {
    let design = 0
    let delivery = 0
    for (const s of sessions) {
      if (isDesignPhase(getSessionPhase(s))) design++
      else delivery++
    }
    return { design, delivery }
  }, [sessions])

  const phaseCounts = useMemo(() => {
    const counts: Record<string, number> = {}
    for (const s of sessions) {
      const phase = getSessionPhase(s)
      counts[phase] = (counts[phase] ?? 0) + 1
    }
    return counts
  }, [sessions])

  const isGrouped = groupBy === 'project-branch'

  return (
    <div className="flex flex-col h-full min-h-0 pb-4 overflow-y-auto">
      {PHASE_GROUPS.map((group) => {
        const groupCount = groupCounts[group.id as 'design' | 'delivery']
        return (
          <div
            key={group.id}
            className="rounded-lg border border-gray-200 dark:border-gray-800 mb-4"
          >
            {/* Group header */}
            <div className="px-4 py-2.5 flex items-center gap-2 border-b border-gray-200 dark:border-gray-800 bg-gray-50/50 dark:bg-gray-900/50 rounded-t-lg">
              <span className="text-base">{group.emoji}</span>
              <span className="text-sm font-semibold text-gray-700 dark:text-gray-300">
                {group.label}
              </span>
              <span className="text-xs text-gray-400 dark:text-gray-500">({groupCount})</span>
            </div>

            {/* Phase column headers */}
            <HarnessPhaseColumnHeaders phases={group.phases} counts={phaseCounts} />

            {/* Content: grouped or flat */}
            <div className="px-1 pb-3">
              {isGrouped ? (
                <HarnessGroupedContent
                  phases={group.phases}
                  projectGroups={projectGroups}
                  isDesign={group.id === 'design'}
                  selectedId={selectedId}
                  onSelect={onSelect}
                  stalledSessions={stalledSessions}
                  currentTime={currentTime}
                  onCardClick={onCardClick}
                  isCollapsed={isCollapsed}
                  toggleCollapse={toggleCollapse}
                />
              ) : (
                <FlatContent
                  phases={group.phases}
                  sessions={sessions}
                  isDesign={group.id === 'design'}
                  selectedId={selectedId}
                  onSelect={onSelect}
                  stalledSessions={stalledSessions}
                  currentTime={currentTime}
                  onCardClick={onCardClick}
                />
              )}
            </div>
          </div>
        )
      })}
    </div>
  )
}

/** Flat mode: cards directly distributed across phase columns without swimlanes */
function FlatContent({
  phases,
  sessions,
  isDesign,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
}: {
  phases: readonly PhaseColumn[]
  sessions: LiveSession[]
  isDesign: boolean
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
}) {
  const filtered = useMemo(
    () => sessions.filter((s) => isDesignPhase(getSessionPhase(s)) === isDesign),
    [sessions, isDesign],
  )
  const byPhase = useMemo(() => splitByPhase(filtered, phases), [filtered, phases])

  return (
    <HarnessPhaseRow
      phases={phases}
      byPhase={byPhase}
      selectedId={selectedId}
      onSelect={onSelect}
      stalledSessions={stalledSessions}
      currentTime={currentTime}
      onCardClick={onCardClick}
      hideProjectBranch={false}
    />
  )
}
