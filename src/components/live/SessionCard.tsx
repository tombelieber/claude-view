import { Link, useSearchParams } from 'react-router-dom'
import {
  GitBranch,
  MessageCircle, FileCheck, Shield, AlertTriangle, Clock,
  Sparkles, Terminal, CheckCircle, Power, Bell, Loader, Archive, CirclePause,
} from 'lucide-react'
import { sessionTotalCost, type LiveSession } from './use-live-sessions'
import type { AgentState } from './types'
import { KNOWN_STATES, GROUP_DEFAULTS } from './types'
import { ContextGauge } from './ContextGauge'
import { CostTooltip } from './CostTooltip'
import { SubAgentPills } from './SubAgentPills'
import { TaskProgressList } from './TaskProgressList'
import { buildSessionUrl } from '../../lib/url-utils'
import { cleanPreviewText } from '../../utils/get-session-title'
import { SessionSpinner, pickVerb } from '../spinner'

const ICON_MAP: Record<string, React.ComponentType<{ className?: string }>> = {
  MessageCircle, FileCheck, Shield, AlertTriangle, Clock,
  Sparkles, Terminal, GitBranch, CheckCircle, Power, Bell, Loader, Archive, CirclePause,
}

const COLOR_MAP: Record<string, string> = {
  amber: 'bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-300',
  red: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300',
  green: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300',
  blue: 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300',
  gray: 'bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400',
  orange: 'bg-orange-100 text-orange-700 dark:bg-orange-900/30 dark:text-orange-400',
}

function formatCostUsd(usd: number): string {
  if (usd === 0) return '$0.00'
  if (usd < 0.01) return `$${usd.toFixed(4)}`
  return `$${usd.toFixed(2)}`
}

function StateBadge({ agentState }: { agentState: AgentState }) {
  const known = KNOWN_STATES[agentState.state]
  const defaults = GROUP_DEFAULTS[agentState.group]
  const config = known ?? defaults

  const Icon = ICON_MAP[config.icon] ?? Bell
  const colorClass = COLOR_MAP[config.color] ?? COLOR_MAP.gray

  return (
    <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${colorClass}`}>
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
}

export function SessionCard({ session, stalledSessions, currentTime, onClickOverride }: SessionCardProps) {
  const [searchParams] = useSearchParams()
  const turnStart = session.currentTurnStartedAt ?? session.startedAt ?? currentTime
  const elapsedSeconds = currentTime - turnStart

  // Title: last user message (cleaned) > first user message > project display name
  const rawLastMessage = session.lastUserMessage || ''
  const rawTitle = session.title || ''
  const cleanedLastMessage = rawLastMessage ? cleanPreviewText(rawLastMessage) : ''
  const cleanedTitle = rawTitle ? cleanPreviewText(rawTitle) : ''
  const title = cleanedLastMessage || cleanedTitle || (session.projectDisplayName || session.project)

  // Show "last message" only when different from title
  const lastMsg = cleanedLastMessage
  const showLastMsg = lastMsg && lastMsg !== title
  const totalCost = sessionTotalCost(session)
  const estimatedPrefix = session.cost?.isEstimated ? '~' : ''

  const cardClassName = "group block rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-4 hover:bg-gray-50 dark:hover:bg-gray-800/70 cursor-pointer transition-colors"

  const cardContent = (
    <>
      {/* Header: badges + cost */}
      <div className="flex items-center gap-2 mb-1">
        <div className="flex items-center gap-1.5 min-w-0 flex-1 overflow-hidden">
          {session.agentState.group === 'autonomous' && (
            <span
              data-testid="pulse-dot"
              className="inline-block w-2 h-2 rounded-full bg-green-500 flex-shrink-0 motion-safe:animate-pulse"
              aria-hidden="true"
            />
          )}
          <span
            className="inline-block px-1.5 py-0.5 text-[10px] font-medium bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 rounded truncate max-w-[120px]"
            title={session.projectPath || session.projectDisplayName || session.project}
          >
            {session.projectDisplayName || session.project}
          </span>
          {session.gitBranch && (
            <span
              className="inline-flex items-center gap-0.5 px-1.5 py-0.5 text-[10px] font-mono bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 rounded max-w-[120px]"
              title={session.gitBranch}
            >
              <GitBranch className="w-2.5 h-2.5 flex-shrink-0" />
              <span className="truncate">{session.gitBranch}</span>
            </span>
          )}
        </div>
        <div className="flex items-center gap-1.5 flex-shrink-0">
          <CostTooltip cost={session.cost} cacheStatus={session.cacheStatus} subAgents={session.subAgents}>
            <span className="text-sm font-mono text-gray-500 dark:text-gray-400 tabular-nums">
              {estimatedPrefix}{formatCostUsd(totalCost)}
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
          <span className="text-gray-300 dark:text-gray-600 mr-1">{'->'}</span>{lastMsg}
        </p>
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
          agentStateLabel={session.agentState.label}
          spinnerVerb={pickVerb(session.id)}
          lastCacheHitAt={session.lastCacheHitAt}
          lastTurnTaskSeconds={session.lastTurnTaskSeconds}
        />
      </div>

      {/* Task progress */}
      {session.progressItems && session.progressItems.length > 0 && (
        <TaskProgressList items={session.progressItems} />
      )}

      {/* Sub-agent pills */}
      {session.subAgents && session.subAgents.length > 0 && (
        <div className="mb-2 -mx-1">
          <SubAgentPills subAgents={session.subAgents} />
        </div>
      )}

      {/* Context gauge */}
      <ContextGauge
        contextWindowTokens={session.contextWindowTokens}
        model={session.model}
        group={session.agentState.group}
        tokens={session.tokens}
        turnCount={session.turnCount}
      />

      {/* Footer: turns */}
      <div className="flex items-center gap-3 mt-2 text-xs text-gray-400 dark:text-gray-500">
        <span>{session.turnCount} turns</span>
      </div>
    </>
  )

  if (onClickOverride) {
    return (
      <div
        onClick={(e) => { e.stopPropagation(); onClickOverride() }}
        className={cardClassName}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onClickOverride() } }}
      >
        {cardContent}
      </div>
    )
  }

  return (
    <Link to={buildSessionUrl(session.id, searchParams)} className={cardClassName} style={{ cursor: 'pointer' }}>
      {cardContent}
    </Link>
  )
}
