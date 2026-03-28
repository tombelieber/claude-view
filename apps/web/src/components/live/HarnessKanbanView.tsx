import { useMemo } from 'react'
import { cn } from '../../lib/utils'
import { HarnessGroupedContent } from './HarnessGroupedContent'
import { HarnessNeedsYouSidebar } from './HarnessNeedsYouSidebar'
import { HarnessPhaseColumnHeaders, HarnessPhaseRow } from './HarnessPhaseRow'
import {
  PHASE_GROUPS,
  getSessionPhase,
  isDesignPhase,
  splitByPhase,
  type PhaseColumn,
} from './harness-phase-groups'
import { needsYouSortKey } from './KanbanView'
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
  // Split into needs_you (sidebar) vs autonomous (phase pipeline)
  const needsYou = useMemo(
    () =>
      sessions
        .filter((s) => s.agentState.group === 'needs_you')
        .sort((a, b) => {
          const cacheRank = (s: LiveSession) =>
            s.cacheStatus === 'warm' ? 0 : s.cacheStatus === 'unknown' ? 1 : 2
          const cacheDiff = cacheRank(a) - cacheRank(b)
          if (cacheDiff !== 0) return cacheDiff
          const keyDiff = needsYouSortKey(a) - needsYouSortKey(b)
          if (keyDiff !== 0) return keyDiff
          return b.lastActivityAt - a.lastActivityAt
        }),
    [sessions],
  )

  const autonomous = useMemo(
    () => sessions.filter((s) => s.agentState.group === 'autonomous'),
    [sessions],
  )

  const hasNeedsYou = needsYou.length > 0

  return (
    <div className="flex h-full min-h-0 gap-4">
      {/* Left sidebar -- needs_you sessions */}
      <div
        className={cn(
          'shrink-0 flex flex-col min-h-0 transition-all duration-200',
          hasNeedsYou ? 'w-72' : 'w-10',
        )}
      >
        <HarnessNeedsYouSidebar
          sessions={needsYou}
          collapsed={!hasNeedsYou}
          selectedId={selectedId}
          onSelect={onSelect}
          stalledSessions={stalledSessions}
          currentTime={currentTime}
          onCardClick={onCardClick}
        />
      </div>

      {/* Right -- phase pipeline (autonomous sessions only) */}
      <div className="flex-1 min-w-0 min-h-0 overflow-y-auto pb-4">
        <PhasePipeline
          sessions={autonomous}
          groupBy={groupBy}
          projectGroups={projectGroups}
          selectedId={selectedId}
          onSelect={onSelect}
          stalledSessions={stalledSessions}
          currentTime={currentTime}
          onCardClick={onCardClick}
          isCollapsed={isCollapsed}
          toggleCollapse={toggleCollapse}
        />
      </div>
    </div>
  )
}

/** Phase pipeline: Design + Delivery groups with phase columns */
function PhasePipeline({
  sessions,
  groupBy,
  projectGroups,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
  isCollapsed,
  toggleCollapse,
}: {
  sessions: LiveSession[]
  groupBy: KanbanGroupBy
  projectGroups: ProjectGroup[]
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
  isCollapsed: (key: string) => boolean
  toggleCollapse: (key: string) => void
}) {
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
    <div className="flex flex-col gap-4">
      {PHASE_GROUPS.map((group) => {
        const groupCount = groupCounts[group.id as 'design' | 'delivery']
        return (
          <div key={group.id} className="rounded-lg border border-gray-200 dark:border-gray-800">
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
