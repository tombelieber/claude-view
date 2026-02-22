import {
  Sparkles,
  Terminal,
  GitBranch,
  MessageCircle,
  FileCheck,
  Shield,
  AlertTriangle,
  CirclePause,
  Clock,
  CheckCircle2,
  LogOut,
  Package,
  Bell,
  Loader,
  type LucideIcon,
} from 'lucide-react'
import type { LiveSession } from '../live/use-live-sessions.ts'
import type { AgentState } from '../live/types.ts'

const ICON_MAP: Record<string, LucideIcon> = {
  awaiting_input: MessageCircle,
  awaiting_approval: FileCheck,
  needs_permission: Shield,
  error: AlertTriangle,
  interrupted: CirclePause,
  idle: Clock,
  thinking: Sparkles,
  acting: Terminal,
  delegating: GitBranch,
  task_complete: CheckCircle2,
  session_ended: LogOut,
  work_delivered: Package,
}

const GROUP_ICON: Record<string, LucideIcon> = {
  needs_you: Bell,
  autonomous: Loader,
}

const COLOR_MAP: Record<string, string> = {
  awaiting_input: 'text-amber-400',
  awaiting_approval: 'text-amber-400',
  needs_permission: 'text-red-400',
  error: 'text-red-400',
  interrupted: 'text-amber-400',
  idle: 'text-gray-400',
  thinking: 'text-green-400',
  acting: 'text-green-400',
  delegating: 'text-green-400',
  task_complete: 'text-gray-400',
  session_ended: 'text-gray-400',
  work_delivered: 'text-gray-400',
}

const GROUP_COLOR: Record<string, string> = {
  needs_you: 'text-amber-400',
  autonomous: 'text-green-400',
}

function getStateIcon(agentState: AgentState): LucideIcon {
  return ICON_MAP[agentState.state] ?? GROUP_ICON[agentState.group] ?? Clock
}

function getStateColor(agentState: AgentState): string {
  return COLOR_MAP[agentState.state] ?? GROUP_COLOR[agentState.group] ?? 'text-gray-400'
}

/** Compute context used percentage from contextWindowTokens (default 200k limit). */
function contextPercent(session: LiveSession): number {
  // Use 200_000 as the default context limit
  const limit = 200_000
  return Math.min(100, Math.round((session.contextWindowTokens / limit) * 100))
}

function contextBarColor(pct: number): string {
  if (pct >= 90) return 'bg-red-500'
  if (pct >= 70) return 'bg-amber-500'
  return 'bg-green-500'
}

function formatCost(usd: number): string {
  if (usd < 0.01) return '<$0.01'
  return `$${usd.toFixed(2)}`
}

/** Extracts the last path segment as a short project name. */
function shortProjectName(session: LiveSession): string {
  const path = session.projectPath || session.project
  if (!path) return 'Unknown'
  const segments = path.split('/')
  return segments[segments.length - 1] || segments[segments.length - 2] || path
}

interface MobileSessionCardProps {
  session: LiveSession
  onClick: () => void
}

export function MobileSessionCard({ session, onClick }: MobileSessionCardProps) {
  const StateIcon = getStateIcon(session.agentState)
  const stateColor = getStateColor(session.agentState)
  const pct = contextPercent(session)
  const totalCost = session.cost?.totalUsd ?? 0
  const subAgentCost = session.subAgents?.reduce((sum, a) => sum + (a.costUsd ?? 0), 0) ?? 0
  const combinedCost = totalCost + subAgentCost

  return (
    <button
      type="button"
      onClick={onClick}
      className="w-full text-left bg-gray-900 border border-gray-800 rounded-xl p-4 min-h-[44px] cursor-pointer active:bg-gray-800 transition-colors"
    >
      {/* Top row: project name + cost badge */}
      <div className="flex items-center justify-between mb-2">
        <span className="text-xs text-gray-500 font-medium truncate mr-2">
          {shortProjectName(session)}
        </span>
        <span className="text-xs text-gray-400 bg-gray-800 rounded px-1.5 py-0.5 whitespace-nowrap">
          {formatCost(combinedCost)}
        </span>
      </div>

      {/* Title / summary */}
      <p className="text-sm text-gray-200 font-medium truncate mb-2">
        {session.summary || session.lastUserMessage || session.title || 'Untitled session'}
      </p>

      {/* Agent state row */}
      <div className="flex items-center gap-1.5 mb-3">
        <StateIcon className={`w-4 h-4 ${stateColor} flex-shrink-0`} />
        <span className={`text-xs font-medium ${stateColor}`}>
          {session.agentState.label}
        </span>
      </div>

      {/* Context bar */}
      <div className="w-full h-1 bg-gray-800 rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all ${contextBarColor(pct)}`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </button>
  )
}
