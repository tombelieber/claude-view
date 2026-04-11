import * as Tooltip from '@radix-ui/react-tooltip'
import {
  AlertTriangle,
  Archive,
  Bell,
  CheckCircle,
  CirclePause,
  Clock,
  FileCheck,
  FolderOpen,
  GitBranch,
  Loader,
  MessageCircle,
  Power,
  Shield,
  Sparkles,
  Terminal,
  TreePine,
} from 'lucide-react'
import { Link, useSearchParams } from 'react-router-dom'
import { CliTerminalCompact } from '../cli-terminal'
import { formatCostUsd } from '../../lib/format-utils'
import { buildSessionUrl } from '../../lib/url-utils'
import { cleanPreviewText } from '../../utils/get-session-title'
import { PhaseBadge, PhaseBadgeSkeleton } from '../PhaseBadge'
import { SourceBadge } from '../shared/SourceBadge'
import { SessionSpinner, pickVerb } from '../spinner'
import { useInteractionResponder } from '@claude-view/shared'
import { SessionInteractionCard } from '@claude-view/shared/components/conversation/blocks/shared/SessionInteractionCard'
import { useFullInteraction } from '../../hooks/use-full-interaction'
import { ContextGauge } from './ContextGauge'
import { CostTooltip } from './CostTooltip'
import { TaskProgressList } from './TaskProgressList'
import { hasUnavailableCost, unavailableCostReason } from './cost-display'
import { getEffectiveBranch } from './effective-branch'
import type { AgentState } from './types'
import { GROUP_DEFAULTS, KNOWN_STATES } from './types'
import { useTeamSidechains } from '../../hooks/use-teams'
import { type LiveSession, sessionTotalCost } from './use-live-sessions'

// ─── StateBadge (exported — used by Harness, ListView, TerminalOverlay) ──

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

// ─── Helpers ──────────────────────────────────────────────────────────

function formatDurationMs(ms: number): string {
  const secs = Math.round(ms / 1000)
  if (secs < 60) return `${secs}s`
  const mins = Math.floor(secs / 60)
  if (mins < 60) return `${mins}m`
  const hours = Math.floor(mins / 60)
  const remMins = mins % 60
  return remMins > 0 ? `${hours}h${remMins}m` : `${hours}h`
}

const TOOLTIP_CLS =
  'bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2 shadow-lg z-50 max-w-sm text-xs'
const ARROW_CLS = 'fill-gray-200 dark:fill-gray-700'

// ─── Shared detail-zone text classes ──────────────────────────────────
// 3 levels only: label (muted), content (readable), accent (colored)
const TXT = {
  label: 'text-gray-500 dark:text-gray-400', // labels, separators, overflow counts
  content: 'text-gray-700 dark:text-gray-300', // values — file names, tool names, member names
  accent: 'text-amber-600 dark:text-amber-400', // team name accent (Anthropic brand)
} as const
const SEP = <span className={`${TXT.label} mx-0.5`}>·</span>

function DetailLabel({ children }: { children: React.ReactNode }) {
  return <span className={`${TXT.label} shrink-0 select-none`}>{children}</span>
}

// ─── Detail-zone renderers (plain text, no colored pills) ─────────────

const FILE_KIND_ICON: Record<string, React.ReactNode> = {
  mention: <span className="text-emerald-500 dark:text-emerald-400 font-semibold">@</span>,
  ide: <span className="text-sky-500 dark:text-sky-400">↗</span>,
  pasted: <span className="text-violet-500 dark:text-violet-400">⎘</span>,
}

function FilesLine({ files }: { files: NonNullable<LiveSession['userFiles']> }) {
  if (files.length === 0) return null
  const visible = files.slice(0, 4)
  const overflow = files.length - 4

  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <div className="flex items-center gap-1.5 text-xs cursor-default min-w-0">
          <DetailLabel>Files</DetailLabel>
          <span className={`flex items-center gap-1 ${TXT.content} font-mono flex-wrap min-w-0`}>
            {visible.map((f, i) => (
              <span key={f.path} className="inline-flex items-center gap-0.5 shrink-0">
                {i > 0 && SEP}
                {FILE_KIND_ICON[f.kind] ?? FILE_KIND_ICON.mention}
                {f.displayName}
              </span>
            ))}
            {overflow > 0 && <span className={`${TXT.label} shrink-0 ml-0.5`}>+{overflow}</span>}
          </span>
        </div>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={TOOLTIP_CLS} sideOffset={5}>
          <div className="space-y-0.5">
            {files.map((f) => (
              <div key={f.path} className="flex items-center gap-1.5">
                <span className="shrink-0">{FILE_KIND_ICON[f.kind]}</span>
                <span className="font-mono text-gray-900 dark:text-gray-100 break-all">
                  {f.path}
                </span>
              </div>
            ))}
          </div>
          <Tooltip.Arrow className={ARROW_CLS} />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  )
}

function cleanToolName(name: string, kind: string): string {
  if (kind !== 'mcp') return name
  let cleaned = name.startsWith('plugin_') ? name.slice(7) : name
  const parts = cleaned.split('_')
  if (parts.length === 2 && parts[0] === parts[1]) cleaned = parts[0]
  return cleaned
}

function ToolsLine({ tools }: { tools: { name: string; kind: string }[] }) {
  if (tools.length === 0) return null
  const visible = tools.slice(0, 4)
  const overflow = tools.length - 4

  return (
    <div className="flex items-center gap-1.5 text-xs min-w-0">
      <DetailLabel>Tools</DetailLabel>
      <span className={`flex items-center gap-1 ${TXT.content} flex-wrap min-w-0`}>
        {visible.map((t, i) => (
          <span key={`${t.kind}-${t.name}`} className="inline-flex items-center gap-0.5 shrink-0">
            {i > 0 && SEP}
            <span className={TXT.label}>{t.kind === 'mcp' ? '⊞' : '⚡'}</span>
            {cleanToolName(t.name, t.kind)}
          </span>
        ))}
        {overflow > 0 && <span className={`${TXT.label} shrink-0 ml-0.5`}>+{overflow}</span>}
      </span>
    </div>
  )
}

const STATUS_DOT: Record<string, string> = {
  running: 'bg-green-500',
  complete: 'bg-gray-400 dark:bg-gray-500',
  error: 'bg-red-500',
}

function AgentsLine({ subAgents }: { subAgents: LiveSession['subAgents'] }) {
  if (!subAgents || subAgents.length === 0) return null
  const visible = subAgents.slice(0, 3)
  const overflow = subAgents.length - 3

  return (
    <div className="text-xs space-y-0.5">
      <DetailLabel>Agents</DetailLabel>
      {visible.map((a) => (
        <div key={a.toolUseId} className="flex items-center gap-1.5 pl-1 min-w-0">
          <span
            className={`w-1.5 h-1.5 rounded-full shrink-0 ${STATUS_DOT[a.status] ?? STATUS_DOT.complete}`}
          />
          <span className={`font-medium ${TXT.content} shrink-0`}>{a.agentType}</span>
          {a.description && <span className={TXT.label}>{a.description}</span>}
        </div>
      ))}
      {overflow > 0 && <div className={`pl-1 ${TXT.label}`}>+{overflow} more</div>}
    </div>
  )
}

function TeamLine({ session }: { session: LiveSession }) {
  if (!session.teamName) return null
  const members = session.teamMembers ?? []

  return (
    <div className="text-xs space-y-0.5">
      <div className="flex items-center gap-1.5">
        <DetailLabel>Team</DetailLabel>
        <span className={`font-medium ${TXT.accent}`}>{session.teamName}</span>
      </div>
      {members.length > 0 && (
        <div className={`flex items-center gap-1 pl-1 flex-wrap ${TXT.label}`}>
          {members.map((m, i) => (
            <span key={m.agentId} className="inline-flex items-center gap-0.5 shrink-0">
              {i > 0 && SEP}
              <span
                className="w-2 h-2 rounded-full shrink-0"
                style={{ backgroundColor: m.color }}
              />
              <span className={TXT.content}>{m.name}</span>
              <span className={TXT.label}>{m.model}</span>
            </span>
          ))}
        </div>
      )}
    </div>
  )
}

function LocationGrouped({ session }: { session: LiveSession }) {
  const { branch, isWorktree } = getEffectiveBranch(
    session.gitBranch,
    session.worktreeBranch ?? null,
    session.isWorktree ?? false,
  )

  return (
    <div className="text-xs">
      <div className={`flex items-center gap-1 ${TXT.content}`}>
        <FolderOpen className="w-3 h-3 shrink-0 text-amber-500 dark:text-amber-400" />
        <span className="truncate">{session.projectDisplayName || session.project}</span>
        {branch && (
          <>
            {SEP}
            <GitBranch className={`w-2.5 h-2.5 shrink-0 ${TXT.label}`} />
            <span className={`font-mono ${TXT.label}`}>{branch}</span>
            {isWorktree && (
              <TreePine className="w-2.5 h-2.5 shrink-0 text-green-500 dark:text-green-400" />
            )}
          </>
        )}
      </div>
    </div>
  )
}

function MetaFooter({ session }: { session: LiveSession }) {
  const parts: string[] = []
  if (session.statuslineVimMode) parts.push(`VIM:${session.statuslineVimMode}`)
  if (session.statuslineOutputStyle && session.statuslineOutputStyle !== 'default')
    parts.push(session.statuslineOutputStyle)
  if (session.statuslineVersion) parts.push(`v${session.statuslineVersion}`)
  if (session.sessionKind === 'background') parts.push('subagent')
  if (session.statuslineWorktreeName) parts.push(`wt:${session.statuslineWorktreeName}`)
  if (session.statuslineAgentName) parts.push(session.statuslineAgentName)

  if (parts.length === 0) return null

  return (
    <div className={`flex items-center gap-1.5 text-xs ${TXT.label} font-mono select-none`}>
      {parts.map((p, i) => (
        <span key={p}>
          {i > 0 && <span className="mr-1.5">·</span>}
          {p}
        </span>
      ))}
    </div>
  )
}

// ─── SessionCard ──────────────────────────────────────────────────────

interface SessionCardProps {
  session: LiveSession
  stalledSessions?: Set<string>
  currentTime: number
  /** When provided, renders as a div instead of Link. Used by Kanban for side panel. */
  onClickOverride?: () => void
  /** When true, hide project + branch badges (shown in swimlane header instead) */
  hideProjectBranch?: boolean
  /** When true, show StateBadge pill instead of green pulse dot (used by Harness view) */
  showStateBadge?: boolean
  /** Handler for expanding a CLI terminal session into a full dockview panel. */
  onExpandCliSession?: (sessionId: string) => void
}

export function SessionCard({
  session,
  stalledSessions,
  currentTime,
  onClickOverride,
  hideProjectBranch,
  showStateBadge,
  onExpandCliSession,
}: SessionCardProps) {
  const [searchParams] = useSearchParams()
  const respond = useInteractionResponder(session.id, session.ownership)
  const fullInteraction = useFullInteraction(session.id, session.pendingInteraction)
  const turnStart = session.currentTurnStartedAt ?? session.startedAt ?? currentTime
  const elapsedSeconds = currentTime - turnStart

  const rawLastMessage = session.lastUserMessage || ''
  const rawTitle = session.title || ''
  const cleanedLastMessage = rawLastMessage ? cleanPreviewText(rawLastMessage) : ''
  const cleanedTitle = rawTitle ? cleanPreviewText(rawTitle) : ''
  const aiTitle = session.aiTitle || ''
  const title = cleanedLastMessage || cleanedTitle || session.projectDisplayName || session.project

  const lastMsg = cleanedLastMessage
  const showLastMsg = lastMsg && lastMsg !== title

  // Fetch sidechain costs for team sessions (team sessions are rare — 1-2 max in any view)
  const { data: sidechainsData } = useTeamSidechains(
    session.teamName ?? null,
    session.teamName ? session.id : null,
  )
  const sidechainCostTotal = sidechainsData?.reduce((sum, sc) => sum + (sc.costUsd ?? 0), 0) ?? 0
  const totalCost = sessionTotalCost(session) + sidechainCostTotal
  const isCostUnavailable = hasUnavailableCost(totalCost, session.cost, session.tokens.totalTokens)
  const totalCostLabel = isCostUnavailable ? 'Unavailable' : formatCostUsd(totalCost)
  const costTitle = isCostUnavailable ? unavailableCostReason(session.cost) : undefined

  const isAutonomous = session.agentState.group === 'autonomous'
  const isCompacting = session.agentState.state === 'compacting'
  const cardClassName = `group block rounded-lg border bg-white dark:bg-gray-900 p-4 hover:bg-gray-50 dark:hover:bg-gray-800/70 cursor-pointer transition-colors ${
    isCompacting ? 'animate-live-compact-breathe' : 'border-gray-200 dark:border-gray-700'
  }`

  const files = session.userFiles ?? []
  const tools = session.toolsUsed ?? []
  const hasPhaseData = session.phase?.current?.phase && session.phase.current.phase !== 'working'
  const isClassifying = !hasPhaseData && isAutonomous && session.turnCount > 0

  const hasLocation = !hideProjectBranch
  const hasFiles = files.length > 0
  const hasTools = tools.length > 0
  const hasTeam = Boolean(session.teamName)
  const hasAgents = session.subAgents && session.subAgents.length > 0
  const hasDetails = hasLocation || hasFiles || hasTools || hasTeam || hasAgents

  const cardContent = (
    <Tooltip.Provider delayDuration={200}>
      {/* ── Row 1: Status + Phase (hero) → Cost ── */}
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2 min-w-0">
          <SourceBadge source={session.source} />
          {showStateBadge ? (
            <StateBadge agentState={session.agentState} />
          ) : (
            isAutonomous && (
              <span className="relative inline-flex flex-shrink-0 w-2.5 h-2.5" aria-hidden="true">
                <span className="absolute inset-0 rounded-full bg-green-400/70 motion-safe:animate-live-ring" />
                <span className="absolute inset-0 rounded-full bg-green-300/50 motion-safe:animate-live-ring2" />
                <span
                  data-testid="pulse-dot"
                  className="relative inline-block w-2.5 h-2.5 rounded-full bg-green-500 motion-safe:animate-live-breathe"
                />
              </span>
            )
          )}
          {hasPhaseData ? (
            <PhaseBadge
              phase={session.phase!.current!.phase}
              scope={session.phase?.current?.scope}
              freshness={session.phase?.freshness}
            />
          ) : isClassifying ? (
            <PhaseBadgeSkeleton />
          ) : null}
        </div>
        <CostTooltip
          cost={session.cost}
          cacheStatus={session.cacheStatus}
          tokens={session.tokens}
          subAgents={session.subAgents}
          sidechains={sidechainsData}
          compactCount={session.compactCount}
        >
          <span
            className="text-sm font-mono text-gray-500 dark:text-gray-400 tabular-nums shrink-0"
            title={costTitle}
          >
            {totalCostLabel}
          </span>
        </CostTooltip>
      </div>

      {/* ── AI title ── */}
      {aiTitle && (
        <p className={`text-xs ${TXT.label} mb-0.5 truncate`}>
          <span className="mr-1">✦</span>
          {aiTitle}
        </p>
      )}

      {/* ── Title ── */}
      <p className="text-sm font-medium text-gray-900 dark:text-gray-100 line-clamp-2 mb-0.5">
        {title}
      </p>

      {/* ── Last user message ── */}
      {showLastMsg && (
        <p className={`text-xs ${TXT.label} line-clamp-1 mb-1`}>
          <span className="mr-1">→</span>
          {lastMsg}
        </p>
      )}

      {/* ── Spinner ── */}
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

      {/* ── Pending interaction (permission, question, plan, elicitation) ── */}
      {session.pendingInteraction && (
        <SessionInteractionCard
          sessionId={session.id}
          meta={session.pendingInteraction}
          fullInteraction={fullInteraction}
          respond={respond}
        />
      )}

      {/* ── Task progress ── */}
      {session.progressItems && session.progressItems.length > 0 && (
        <TaskProgressList items={session.progressItems} />
      )}

      {/* ── Context gauge ── */}
      <ContextGauge
        contextWindowTokens={session.contextWindowTokens}
        model={session.model}
        group={session.agentState.group}
        tokens={session.tokens}
        turnCount={session.turnCount}
        agentLabel={session.agentState.label}
        agentStateKey={session.agentState.state}
        compactCount={session.compactCount}
        statuslineContextWindowSize={session.statuslineContextWindowSize}
        statuslineUsedPct={session.statuslineUsedPct}
        statuslineRemainingPct={session.statuslineRemainingPct}
        statuslineTotalInputTokens={session.statuslineTotalInputTokens}
        statuslineTotalOutputTokens={session.statuslineTotalOutputTokens}
      />

      {/* ── Duration + lines changed ── */}
      {(session.statuslineTotalDurationMs != null ||
        session.statuslineLinesAdded != null ||
        session.statuslineLinesRemoved != null) && (
        <div className="flex items-center gap-3 mt-1.5 text-xs text-gray-400 dark:text-gray-500 font-mono tabular-nums">
          {session.statuslineTotalDurationMs != null && (
            <span>{formatDurationMs(Number(session.statuslineTotalDurationMs))}</span>
          )}
          {(session.statuslineLinesAdded != null || session.statuslineLinesRemoved != null) && (
            <span>
              {session.statuslineLinesAdded != null && (
                <span className="text-green-500 dark:text-green-400">
                  +{Number(session.statuslineLinesAdded)}
                </span>
              )}
              {session.statuslineLinesAdded != null &&
                session.statuslineLinesRemoved != null &&
                ' / '}
              {session.statuslineLinesRemoved != null && (
                <span className="text-red-500 dark:text-red-400">
                  -{Number(session.statuslineLinesRemoved)}
                </span>
              )}
            </span>
          )}
        </div>
      )}

      {/* ── CLI Terminal compact view ── */}
      {session.id.startsWith('cv-') && (
        <div className="border-t border-gray-100 dark:border-gray-800 mt-2.5 pt-2">
          <CliTerminalCompact
            tmuxSessionId={session.id}
            onExpand={onExpandCliSession ? () => onExpandCliSession(session.id) : undefined}
          />
        </div>
      )}

      {/* ── Detail zone — plain text, labeled lines ── */}
      {hasDetails && (
        <div className="border-t border-gray-100 dark:border-gray-800 mt-2.5 pt-2 space-y-1">
          {hasLocation && <LocationGrouped session={session} />}
          {hasFiles && <FilesLine files={files} />}
          {hasTools && <ToolsLine tools={tools} />}
          {hasTeam && <TeamLine session={session} />}
          {hasAgents && <AgentsLine subAgents={session.subAgents} />}
          <MetaFooter session={session} />
        </div>
      )}
    </Tooltip.Provider>
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
