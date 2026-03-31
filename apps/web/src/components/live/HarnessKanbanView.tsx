import { ChevronDown, ChevronRight, FlaskConical } from 'lucide-react'
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

  const needsYouIds = useMemo(() => new Set(needsYou.map((s) => s.id)), [needsYou])

  const autonomous = useMemo(
    () => sessions.filter((s) => !needsYouIds.has(s.id)),
    [sessions, needsYouIds],
  )

  const hasNeedsYou = needsYou.length > 0

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      {/* Preview callout */}
      <div className="mx-1 flex items-center gap-2 rounded-md border border-amber-300/30 dark:border-amber-700/30 bg-amber-50/60 dark:bg-amber-950/30 px-3 py-1.5 text-xs text-amber-700 dark:text-amber-400">
        <FlaskConical className="size-3.5 shrink-0" />
        <span>
          <span className="font-medium">Preview</span> — UI polish coming soon. Ideas?{' '}
          <a
            href="https://github.com/tombelieber/claude-view/issues"
            target="_blank"
            rel="noopener noreferrer"
            className="font-medium underline underline-offset-2 hover:text-amber-600 dark:hover:text-amber-300"
          >
            Open an issue
          </a>
        </span>
      </div>

      <div className="flex flex-1 min-h-0 gap-4">
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
          excludeIds={needsYouIds}
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
    </div>
  )
}

/** Phase pipeline: Design + Delivery groups with phase columns */
function PhasePipeline({
  sessions,
  excludeIds,
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
  excludeIds: Set<string>
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
        const groupKey = `harness-${group.id}`
        const groupCollapsed = isCollapsed(groupKey)
        const Chevron = groupCollapsed ? ChevronRight : ChevronDown
        return (
          <div key={group.id} className="rounded-lg border border-gray-200 dark:border-gray-800">
            {/* Sticky group header + phase columns */}
            <div className={cn('sticky top-0 z-10 bg-white dark:bg-gray-950', groupCollapsed ? 'rounded-lg' : 'rounded-t-lg')}>
              {/* Group header — clickable to collapse */}
              <button
                type="button"
                onClick={() => toggleCollapse(groupKey)}
                className={cn(
                  'w-full px-4 py-2.5 flex items-center gap-2 bg-gray-50/50 dark:bg-gray-900/50 cursor-pointer hover:bg-gray-100/50 dark:hover:bg-gray-800/50 transition-colors',
                  groupCollapsed ? 'rounded-lg' : 'rounded-t-lg border-b border-gray-200 dark:border-gray-800',
                )}
              >
                <Chevron className="size-3.5 text-gray-400 dark:text-gray-500 shrink-0" />
                <span className="text-base">{group.emoji}</span>
                <span className="text-sm font-semibold text-gray-700 dark:text-gray-300">
                  {group.label}
                </span>
                <span className="text-xs text-gray-400 dark:text-gray-500">({groupCount})</span>
              </button>

              {/* Phase column headers — hidden when collapsed */}
              {!groupCollapsed && (
                <HarnessPhaseColumnHeaders phases={group.phases} counts={phaseCounts} />
              )}
            </div>

            {/* Content: grouped or flat — hidden when collapsed */}
            {!groupCollapsed && (
              <div className="px-1 pb-3">
                {isGrouped ? (
                  <HarnessGroupedContent
                    phases={group.phases}
                    projectGroups={projectGroups}
                    excludeIds={excludeIds}
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
            )}
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
