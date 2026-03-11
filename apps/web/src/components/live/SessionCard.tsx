import * as Tooltip from '@radix-ui/react-tooltip'
import {
  AlertTriangle,
  Archive,
  Bell,
  CheckCircle,
  CirclePause,
  Clock,
  FileCheck,
  FileText,
  FolderOpen,
  GitBranch,
  Loader,
  MessageCircle,
  Minimize2,
  Power,
  Shield,
  Sparkles,
  Terminal,
  TreePine,
} from 'lucide-react'
import { Link, useSearchParams } from 'react-router-dom'
import { useTeamDetail } from '../../hooks/use-teams'
import { formatCostUsd } from '../../lib/format-utils'
import { buildSessionUrl } from '../../lib/url-utils'
import { cleanPreviewText } from '../../utils/get-session-title'
import { SessionSpinner, pickVerb } from '../spinner'
import { AskUserQuestionDisplay, isAskUserQuestionInput } from './AskUserQuestionDisplay'
import { ContextGauge } from './ContextGauge'
import { CostTooltip } from './CostTooltip'
import { SessionToolChips } from './SessionToolChips'
import { SubAgentPills } from './SubAgentPills'
import { TaskProgressList } from './TaskProgressList'
import { TeamMemberPills } from './TeamMemberPills'
import { hasUnavailableCost } from './cost-display'
import { getEffectiveBranch } from './effective-branch'
import type { AgentState } from './types'
import { GROUP_DEFAULTS, KNOWN_STATES } from './types'
import { type LiveSession, sessionTotalCost } from './use-live-sessions'

const ICON_MAP: Record<string, React.ComponentType<{ className?: string }>> = {
  MessageCircle,
  FileCheck,
  Shield,
  AlertTriangle,
  Clock,
  Sparkles,
  Terminal,
  GitBranch,
  CheckCircle,
  Power,
  Bell,
  Loader,
  Archive,
  CirclePause,
}

const COLOR_MAP: Record<string, string> = {
  green: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300',
  amber: 'bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-300',
  red: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300',
  gray: 'bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400',
}

function StateBadge({ agentState }: { agentState: AgentState }) {
  const known = KNOWN_STATES[agentState.state]
  const defaults = GROUP_DEFAULTS[agentState.group]
  const config = known ?? defaults

  const Icon = ICON_MAP[config.icon] ?? Bell
  const colorClass = COLOR_MAP[config.color] ?? COLOR_MAP.gray

  return (
    <span
      className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${colorClass}`}
    >
      <Icon className="h-3 w-3" />
      {agentState.label}
    </span>
  )
}

export { StateBadge }

interface SessionCardProps {
  session: LiveSession
  stalledSessions?: Set<string>
  currentTime: number
  /** When provided, renders as a div instead of Link. Used by Kanban for side panel. */
  onClickOverride?: () => void
  /** When true, hide project + branch badges (shown in swimlane header instead) */
  hideProjectBranch?: boolean
}

export function SessionCard({
  session,
  stalledSessions,
  currentTime,
  onClickOverride,
  hideProjectBranch,
}: SessionCardProps) {
  const [searchParams] = useSearchParams()
  const { data: teamDetail } = useTeamDetail(session.teamName ?? null)
  const turnStart = session.currentTurnStartedAt ?? session.startedAt ?? currentTime
  const elapsedSeconds = currentTime - turnStart

  // Title: last user message (cleaned) > first user message > project display name
  const rawLastMessage = session.lastUserMessage || ''
  const rawTitle = session.title || ''
  const cleanedLastMessage = rawLastMessage ? cleanPreviewText(rawLastMessage) : ''
  const cleanedTitle = rawTitle ? cleanPreviewText(rawTitle) : ''
  const title = cleanedLastMessage || cleanedTitle || session.projectDisplayName || session.project

  // Show "last message" only when different from title
  const lastMsg = cleanedLastMessage
  const showLastMsg = lastMsg && lastMsg !== title
  const totalCost = sessionTotalCost(session)
  const totalCostLabel = hasUnavailableCost(totalCost, session.cost, session.tokens.totalTokens)
    ? 'Unavailable'
    : formatCostUsd(totalCost)

  const isAutonomous = session.agentState.group === 'autonomous'
  const isCompacting = session.agentState.state === 'compacting'
  const cardClassName = `group block rounded-lg border bg-white dark:bg-gray-900 p-4 hover:bg-gray-50 dark:hover:bg-gray-800/70 cursor-pointer transition-colors ${
    isCompacting ? 'animate-live-compact-breathe' : 'border-gray-200 dark:border-gray-700'
  }`

  const cardContent = (
    <>
      {/* Header: badges + cost */}
      <div className="flex items-center gap-2 mb-1">
        <div className="flex items-center gap-1.5 min-w-0 flex-1 overflow-hidden">
          {isAutonomous && (
            <span className="relative inline-flex flex-shrink-0 w-2.5 h-2.5" aria-hidden="true">
              <span className="absolute inset-0 rounded-full bg-green-400/70 motion-safe:animate-live-ring" />
              <span className="absolute inset-0 rounded-full bg-green-300/50 motion-safe:animate-live-ring2" />
              <span
                data-testid="pulse-dot"
                className="relative inline-block w-2.5 h-2.5 rounded-full bg-green-500 motion-safe:animate-live-breathe"
              />
            </span>
          )}
          {!hideProjectBranch && (
            <>
              <span
                className="inline-flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-medium text-gray-700 dark:text-gray-300 rounded truncate max-w-30"
                title={session.projectPath || session.projectDisplayName || session.project}
              >
                <FolderOpen className="w-2.5 h-2.5 shrink-0 text-amber-500 dark:text-amber-400" />
                {session.projectDisplayName || session.project}
              </span>
              {(() => {
                const { branch, driftOrigin, isWorktree } = getEffectiveBranch(
                  session.gitBranch,
                  session.worktreeBranch ?? null,
                  session.isWorktree ?? false,
                )
                if (!branch) return null
                return (
                  <span
                    className="inline-flex items-center gap-0.5 px-1.5 py-0.5 text-[10px] font-mono bg-violet-50 dark:bg-violet-950/50 border border-violet-200 dark:border-violet-800 text-violet-700 dark:text-violet-300 rounded"
                    title={driftOrigin ? `${branch} (worktree, started on ${driftOrigin})` : branch}
                  >
                    <GitBranch className="w-2.5 h-2.5 shrink-0" />
                    <span className="truncate">{branch}</span>
                    {isWorktree && (
                      <TreePine className="w-2.5 h-2.5 shrink-0 text-green-600 dark:text-green-400" />
                    )}
                    {driftOrigin && (
                      <span className="text-[9px] text-violet-400 dark:text-violet-500 ml-0.5">
                        {'↗'}
                        {driftOrigin}
                      </span>
                    )}
                  </span>
                )
              })()}
            </>
          )}
        </div>
        <div className="flex items-center gap-1.5 flex-shrink-0">
          <CostTooltip
            cost={session.cost}
            cacheStatus={session.cacheStatus}
            tokens={session.tokens}
            subAgents={session.subAgents}
            compactCount={session.compactCount}
          >
            <span className="text-sm font-mono text-gray-500 dark:text-gray-400 tabular-nums">
              {totalCostLabel}
            </span>
          </CostTooltip>
        </div>
      </div>

      {/* Title: latest human prompt, with fallback to first prompt/project name */}
      <p className="text-sm font-medium text-gray-900 dark:text-gray-100 line-clamp-2 mb-1">
        {title}
      </p>

      {/* Last user message (if different from title) */}
      {showLastMsg && (
        <p className="text-xs text-gray-500 dark:text-gray-400 line-clamp-1 mb-1">
          <span className="text-gray-300 dark:text-gray-600 mr-1">{'->'}</span>
          {lastMsg}
        </p>
      )}

      {/* IDE file context chip */}
      {session.lastUserFile && (
        <div className="flex items-center gap-1 mb-1">
          <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-mono rounded bg-sky-50 dark:bg-sky-950/40 border border-sky-200 dark:border-sky-800 text-sky-700 dark:text-sky-300 truncate max-w-full">
            <FileText className="h-2.5 w-2.5 shrink-0" />
            {session.lastUserFile.split('/').pop()}
          </span>
        </div>
      )}

      {/* Spinner row */}
      <div className="mb-2">
        <SessionSpinner
          mode="live"
          durationSeconds={elapsedSeconds}
          inputTokens={session.tokens.inputTokens}
          outputTokens={session.tokens.outputTokens}
          model={session.model}
          isStalled={stalledSessions?.has(session.id)}
          agentStateGroup={session.agentState.group}
          agentStateKey={session.agentState.state}
          spinnerVerb={pickVerb(session.id)}
          lastCacheHitAt={session.lastCacheHitAt}
          lastTurnTaskSeconds={session.lastTurnTaskSeconds}
        />
      </div>

      {/* Question card (AskUserQuestion) — show whenever context has questions,
          regardless of specific state (awaiting_input, needs_permission, etc.) */}
      {session.agentState.context && isAskUserQuestionInput(session.agentState.context) && (
        <AskUserQuestionDisplay inputData={session.agentState.context} variant="amber" />
      )}

      {/* Task progress */}
      {session.progressItems && session.progressItems.length > 0 && (
        <TaskProgressList items={session.progressItems} />
      )}

      {/* Team section — badge + member pills */}
      {session.teamName && (
        <div className="mb-2 -mx-1 px-1">
          <div className="flex items-center gap-1.5 mb-1">
            <span className="text-[10px] font-medium uppercase tracking-wider text-zinc-400 dark:text-zinc-500">
              Team
            </span>
            <Tooltip.Provider delayDuration={200}>
              <Tooltip.Root>
                <Tooltip.Trigger asChild>
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 text-[10px] font-semibold rounded-full bg-indigo-100 dark:bg-indigo-950/50 border border-indigo-300 dark:border-indigo-700 text-indigo-800 dark:text-indigo-200 cursor-default">
                    <TreePine className="h-3 w-3 shrink-0" />
                    {session.teamName}
                  </span>
                </Tooltip.Trigger>
                <Tooltip.Portal>
                  <Tooltip.Content
                    className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2 shadow-lg z-50 max-w-xs text-xs"
                    sideOffset={5}
                  >
                    <p className="font-medium text-gray-900 dark:text-gray-100">
                      Agent Team: {session.teamName}
                    </p>
                    <p className="text-gray-500 dark:text-gray-400 mt-0.5">
                      Coordinated multi-agent team. Open the detail panel's Teams tab for members
                      and inbox.
                    </p>
                    <Tooltip.Arrow className="fill-gray-200 dark:fill-gray-700" />
                  </Tooltip.Content>
                </Tooltip.Portal>
              </Tooltip.Root>
            </Tooltip.Provider>
          </div>
          {teamDetail && teamDetail.members.length > 0 && (
            <TeamMemberPills members={teamDetail.members} />
          )}
        </div>
      )}

      {/* Sub-agents section — with label when present */}
      {session.subAgents && session.subAgents.length > 0 && (
        <div className="mb-2 -mx-1 px-1">
          <span className="text-[10px] font-medium uppercase tracking-wider text-zinc-400 dark:text-zinc-500 mb-0.5 block">
            Sub-Agents
          </span>
          <SubAgentPills subAgents={session.subAgents} />
        </div>
      )}

      {/* Tool integrations (MCP servers, Skills) */}
      {session.toolsUsed && session.toolsUsed.length > 0 && (
        <div className="mb-2 -mx-1">
          <SessionToolChips tools={session.toolsUsed} />
        </div>
      )}

      {/* Context gauge */}
      <ContextGauge
        contextWindowTokens={session.contextWindowTokens}
        model={session.model}
        group={session.agentState.group}
        tokens={session.tokens}
        turnCount={session.turnCount}
        agentLabel={session.agentState.label}
        agentStateKey={session.agentState.state}
        compactCount={session.compactCount}
      />

      {/* Footer: turns + compactions */}
      <div className="flex items-center gap-3 mt-2 text-xs text-gray-400 dark:text-gray-500">
        <span>{session.turnCount} turns</span>
        {(session.compactCount ?? 0) > 0 && (
          <span className="inline-flex items-center gap-0.5 text-sky-500 dark:text-sky-400">
            <Minimize2 className="h-3 w-3" />
            {session.compactCount} {session.compactCount === 1 ? 'compact' : 'compacts'}
          </span>
        )}
      </div>
    </>
  )

  if (onClickOverride) {
    return (
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation()
          onClickOverride()
        }}
        className={`w-full text-left ${cardClassName}`}
      >
        {cardContent}
      </button>
    )
  }

  return (
    <Link
      to={buildSessionUrl(session.id, searchParams)}
      className={cardClassName}
      style={{ cursor: 'pointer' }}
    >
      {cardContent}
    </Link>
  )
}
