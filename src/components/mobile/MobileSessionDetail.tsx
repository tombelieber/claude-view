import * as Dialog from '@radix-ui/react-dialog'
import {
  X,
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
  Cpu,
  DollarSign,
  Users,
  type LucideIcon,
} from 'lucide-react'
import type { LiveSession } from '../live/use-live-sessions.ts'
import type { AgentState } from '../live/types.ts'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo.ts'

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

const BG_COLOR_MAP: Record<string, string> = {
  awaiting_input: 'bg-amber-900/30',
  awaiting_approval: 'bg-amber-900/30',
  needs_permission: 'bg-red-900/30',
  error: 'bg-red-900/30',
  interrupted: 'bg-amber-900/30',
  idle: 'bg-gray-800/50',
  thinking: 'bg-green-900/30',
  acting: 'bg-green-900/30',
  delegating: 'bg-green-900/30',
  task_complete: 'bg-gray-800/50',
  session_ended: 'bg-gray-800/50',
  work_delivered: 'bg-gray-800/50',
}

const GROUP_COLOR: Record<string, string> = {
  needs_you: 'text-amber-400',
  autonomous: 'text-green-400',
}

const GROUP_BG: Record<string, string> = {
  needs_you: 'bg-amber-900/30',
  autonomous: 'bg-green-900/30',
}

function getStateIcon(agentState: AgentState): LucideIcon {
  return ICON_MAP[agentState.state] ?? GROUP_ICON[agentState.group] ?? Clock
}

function getStateColor(agentState: AgentState): string {
  return COLOR_MAP[agentState.state] ?? GROUP_COLOR[agentState.group] ?? 'text-gray-400'
}

function getStateBg(agentState: AgentState): string {
  return BG_COLOR_MAP[agentState.state] ?? GROUP_BG[agentState.group] ?? 'bg-gray-800/50'
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

function formatCost(usd: number): string {
  if (usd === 0) return '$0.00'
  if (usd < 0.001) return '<$0.001'
  if (usd < 0.01) return `$${usd.toFixed(3)}`
  return `$${usd.toFixed(2)}`
}

function shortProjectName(session: LiveSession): string {
  const path = session.projectPath || session.project
  if (!path) return 'Unknown'
  const segments = path.split('/')
  return segments[segments.length - 1] || segments[segments.length - 2] || path
}

interface DetailRowProps {
  label: string
  value: string
  className?: string
}

function DetailRow({ label, value, className }: DetailRowProps) {
  return (
    <div className="flex items-center justify-between py-1.5">
      <span className="text-xs text-gray-500">{label}</span>
      <span className={`text-xs font-mono ${className ?? 'text-gray-300'}`}>{value}</span>
    </div>
  )
}

function SubAgentRow({ agent }: { agent: SubAgentInfo }) {
  const statusColor =
    agent.status === 'running'
      ? 'text-green-400'
      : agent.status === 'error'
        ? 'text-red-400'
        : 'text-gray-400'

  return (
    <div className="flex items-center justify-between py-1.5 pl-2 border-l-2 border-gray-800">
      <div className="flex flex-col min-w-0 flex-1 mr-2">
        <span className="text-xs text-gray-300 font-medium truncate">{agent.agentType}</span>
        <span className="text-xs text-gray-500 truncate">{agent.description}</span>
      </div>
      <div className="flex items-center gap-2 flex-shrink-0">
        {agent.costUsd != null && (
          <span className="text-xs text-gray-400 font-mono">{formatCost(agent.costUsd)}</span>
        )}
        <span className={`text-xs font-medium ${statusColor}`}>{agent.status}</span>
      </div>
    </div>
  )
}

interface MobileSessionDetailProps {
  session: LiveSession | null
  open: boolean
  onClose: () => void
}

export function MobileSessionDetail({ session, open, onClose }: MobileSessionDetailProps) {
  if (!session) return null

  const StateIcon = getStateIcon(session.agentState)
  const stateColor = getStateColor(session.agentState)
  const stateBg = getStateBg(session.agentState)
  const totalCost = session.cost?.totalUsd ?? 0
  const subAgentCost = session.subAgents?.reduce((sum, a) => sum + (a.costUsd ?? 0), 0) ?? 0
  const combinedCost = totalCost + subAgentCost
  const hasSubAgents = session.subAgents && session.subAgents.length > 0

  return (
    <Dialog.Root open={open} onOpenChange={(v) => { if (!v) onClose() }}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/60 z-40" />
        <Dialog.Content
          className="fixed bottom-0 left-0 right-0 z-50 bg-gray-900 rounded-t-2xl max-h-[85vh] overflow-y-auto focus:outline-none"
          onOpenAutoFocus={(e) => e.preventDefault()}
        >
          {/* Drag handle */}
          <div className="flex justify-center py-3">
            <div className="w-10 h-1 rounded-full bg-gray-700" />
          </div>

          {/* Header */}
          <div className="px-4 pb-3 border-b border-gray-800">
            <div className="flex items-center justify-between mb-1">
              <span className="text-xs text-gray-500 font-medium">
                {shortProjectName(session)}
              </span>
              <Dialog.Close asChild>
                <button
                  type="button"
                  className="w-8 h-8 flex items-center justify-center rounded-full hover:bg-gray-800 active:bg-gray-700 cursor-pointer min-h-[44px] min-w-[44px]"
                  aria-label="Close"
                >
                  <X className="w-5 h-5 text-gray-400" />
                </button>
              </Dialog.Close>
            </div>
            <Dialog.Title className="text-base font-semibold text-gray-100">
              {session.summary || session.lastUserMessage || session.title || 'Untitled session'}
            </Dialog.Title>
          </div>

          {/* Agent state */}
          <div className="px-4 py-3">
            <div className={`flex items-center gap-2 px-3 py-2.5 rounded-lg ${stateBg}`}>
              <StateIcon className={`w-5 h-5 ${stateColor} flex-shrink-0`} />
              <span className={`text-sm font-medium ${stateColor}`}>
                {session.agentState.label}
              </span>
            </div>
          </div>

          {/* Token breakdown */}
          <div className="px-4 py-2">
            <div className="flex items-center gap-1.5 mb-2">
              <Cpu className="w-4 h-4 text-gray-500" />
              <span className="text-xs font-medium text-gray-400 uppercase tracking-wide">Tokens</span>
            </div>
            <div className="bg-gray-800/50 rounded-lg px-3 py-1">
              <DetailRow label="Input" value={formatTokens(session.tokens.inputTokens)} />
              <DetailRow label="Output" value={formatTokens(session.tokens.outputTokens)} />
              <DetailRow label="Cache read" value={formatTokens(session.tokens.cacheReadTokens)} />
              <DetailRow label="Cache creation" value={formatTokens(session.tokens.cacheCreationTokens)} />
              <DetailRow
                label="Total"
                value={formatTokens(session.tokens.totalTokens)}
                className="text-gray-100 font-semibold"
              />
            </div>
          </div>

          {/* Cost breakdown */}
          <div className="px-4 py-2">
            <div className="flex items-center gap-1.5 mb-2">
              <DollarSign className="w-4 h-4 text-gray-500" />
              <span className="text-xs font-medium text-gray-400 uppercase tracking-wide">Cost</span>
            </div>
            <div className="bg-gray-800/50 rounded-lg px-3 py-1">
              <DetailRow label="Input" value={formatCost(session.cost.inputCostUsd)} />
              <DetailRow label="Output" value={formatCost(session.cost.outputCostUsd)} />
              <DetailRow label="Cache read" value={formatCost(session.cost.cacheReadCostUsd)} />
              <DetailRow label="Cache creation" value={formatCost(session.cost.cacheCreationCostUsd)} />
              {session.cost.cacheSavingsUsd > 0 && (
                <DetailRow
                  label="Cache savings"
                  value={`-${formatCost(session.cost.cacheSavingsUsd)}`}
                  className="text-green-400"
                />
              )}
              {subAgentCost > 0 && (
                <DetailRow label="Sub-agents" value={formatCost(subAgentCost)} />
              )}
              <DetailRow
                label="Total"
                value={formatCost(combinedCost)}
                className="text-gray-100 font-semibold"
              />
            </div>
          </div>

          {/* Sub-agents */}
          {hasSubAgents && (
            <div className="px-4 py-2 pb-4">
              <div className="flex items-center gap-1.5 mb-2">
                <Users className="w-4 h-4 text-gray-500" />
                <span className="text-xs font-medium text-gray-400 uppercase tracking-wide">
                  Sub-agents ({session.subAgents!.length})
                </span>
              </div>
              <div className="bg-gray-800/50 rounded-lg px-3 py-1">
                {session.subAgents!.map((agent) => (
                  <SubAgentRow key={agent.toolUseId} agent={agent} />
                ))}
              </div>
            </div>
          )}

          {/* Bottom safe area padding */}
          <div className="h-6" />
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
