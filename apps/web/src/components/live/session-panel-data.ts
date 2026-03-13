import type { SessionInfo } from '../../types/generated'
import type { ProgressItem } from '../../types/generated/ProgressItem'
import type { RichSessionData } from '../../types/generated/RichSessionData'
import type { SessionDetail } from '../../types/generated/SessionDetail'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
import type { TaskItem } from '../../types/generated/TaskItem'
import type { RichMessage } from './RichPane'
import type { AgentState } from './types'
// src/components/live/session-panel-data.ts
import type { LiveSession } from './use-live-sessions'

/**
 * Unified data shape that SessionDetailPanel can render from.
 * Both LiveSession and historical SessionDetail+RichSessionData map to this.
 */
export interface SessionPanelData {
  // Identity
  id: string
  project: string
  projectDisplayName: string
  projectPath: string
  gitBranch: string | null
  worktreeBranch: string | null
  isWorktree: boolean
  effectiveBranch: string | null

  // Status
  status: 'working' | 'paused' | 'done'

  // Metrics
  model: string | null
  turnCount: number
  tokens: {
    inputTokens: number
    outputTokens: number
    cacheReadTokens: number
    cacheCreationTokens: number
    cacheCreation5mTokens: number
    cacheCreation1hrTokens: number
    totalTokens: number
  }
  contextWindowTokens: number
  cost: {
    totalUsd: number
    inputCostUsd: number
    outputCostUsd: number
    cacheReadCostUsd: number
    cacheCreationCostUsd: number
    cacheSavingsUsd: number
    hasUnpricedUsage: boolean
    unpricedInputTokens: number
    unpricedOutputTokens: number
    unpricedCacheReadTokens: number
    unpricedCacheCreationTokens: number
    pricedTokenCoverage: number
    totalCostSource: string
  }
  cacheStatus: 'warm' | 'cold' | 'unknown'

  // Sub-agents
  subAgents?: SubAgentInfo[]

  // Team name if this session is a team lead (from backend)
  teamName?: string | null

  // Progress items (live tasks/todos)
  progressItems?: ProgressItem[]

  // Persistent task data from ~/.claude/tasks/
  tasks?: TaskItem[]

  compactCount?: number

  // Statusline fields (authoritative from Claude Code per-turn summary)
  statuslineContextWindowSize?: number | null
  statuslineUsedPct?: number | null

  // NEW statusline authority fields (live sessions only)
  modelDisplayName?: string | null
  statuslineCostUsd?: number | null
  statuslineTotalDurationMs?: number | null
  statuslineLinesAdded?: number | null
  statuslineLinesRemoved?: number | null
  statuslineInputTokens?: number | null
  statuslineOutputTokens?: number | null
  statuslineCacheReadTokens?: number | null
  statuslineCacheCreationTokens?: number | null
  statuslineCwd?: string | null
  statuslineProjectDir?: string | null

  // Live-only fields (optional)
  startedAt?: number | null
  lastActivityAt?: number
  lastUserMessage?: string
  lastCacheHitAt?: number | null
  agentState?: AgentState
  pid?: number | null
  currentActivity?: string

  // History-only extensions (optional)
  historyExtras?: {
    sessionDetail: SessionDetail
    sessionInfo?: SessionInfo
  }

  /** Session slug for plan file lookup */
  slug?: string | null
  /** Whether plan files exist for this session's slug */
  hasPlans?: boolean

  // Terminal messages source
  // - For live: undefined (uses WebSocket via useLiveSessionMessages)
  // - For history: pre-converted RichMessage[] from messagesToRichMessages
  terminalMessages?: RichMessage[]
}

/** Adapt a LiveSession into SessionPanelData (thin wrapper, mostly passthrough). */
export function liveSessionToPanelData(session: LiveSession): SessionPanelData {
  return {
    id: session.id,
    project: session.project,
    projectDisplayName: session.projectDisplayName,
    projectPath: session.projectPath,
    gitBranch: session.gitBranch,
    worktreeBranch: session.worktreeBranch,
    isWorktree: session.isWorktree,
    effectiveBranch: session.effectiveBranch,
    status: session.status,
    model: session.model,
    turnCount: session.turnCount,
    tokens: session.tokens,
    contextWindowTokens: session.contextWindowTokens,
    cost: session.cost,
    cacheStatus: session.cacheStatus,
    subAgents: session.subAgents,
    teamName: session.teamName ?? null,
    progressItems: session.progressItems,
    compactCount: session.compactCount,
    statuslineContextWindowSize: session.statuslineContextWindowSize ?? null,
    statuslineUsedPct: session.statuslineUsedPct ?? null,
    modelDisplayName: session.modelDisplayName ?? null,
    statuslineCostUsd: session.statuslineCostUsd ?? null,
    statuslineTotalDurationMs:
      session.statuslineTotalDurationMs != null ? Number(session.statuslineTotalDurationMs) : null,
    statuslineLinesAdded:
      session.statuslineLinesAdded != null ? Number(session.statuslineLinesAdded) : null,
    statuslineLinesRemoved:
      session.statuslineLinesRemoved != null ? Number(session.statuslineLinesRemoved) : null,
    statuslineInputTokens:
      session.statuslineInputTokens != null ? Number(session.statuslineInputTokens) : null,
    statuslineOutputTokens:
      session.statuslineOutputTokens != null ? Number(session.statuslineOutputTokens) : null,
    statuslineCacheReadTokens:
      session.statuslineCacheReadTokens != null ? Number(session.statuslineCacheReadTokens) : null,
    statuslineCacheCreationTokens:
      session.statuslineCacheCreationTokens != null
        ? Number(session.statuslineCacheCreationTokens)
        : null,
    statuslineCwd: session.statuslineCwd ?? null,
    statuslineProjectDir: session.statuslineProjectDir ?? null,
    startedAt: session.startedAt,
    lastActivityAt: session.lastActivityAt,
    lastUserMessage: session.lastUserMessage,
    lastCacheHitAt: session.lastCacheHitAt,
    agentState: session.agentState,
    pid: session.pid,
    currentActivity: session.currentActivity,
    slug: session.slug ?? null,
    hasPlans: false, // Live sessions check on-demand via usePlanDocuments hook
  }
}

/** Adapt history data (SessionDetail + RichSessionData) into SessionPanelData. */
export function historyToPanelData(
  sessionDetail: SessionDetail,
  richData: RichSessionData | undefined,
  sessionInfo: SessionInfo | undefined,
  terminalMessages: RichMessage[],
): SessionPanelData {
  const tokens = richData?.tokens ?? {
    inputTokens: sessionDetail.totalInputTokens ?? 0,
    outputTokens: sessionDetail.totalOutputTokens ?? 0,
    cacheReadTokens: sessionDetail.totalCacheReadTokens ?? 0,
    cacheCreationTokens: sessionDetail.totalCacheCreationTokens ?? 0,
    cacheCreation5mTokens: 0,
    cacheCreation1hrTokens: 0,
    totalTokens:
      (sessionDetail.totalInputTokens ?? 0) +
      (sessionDetail.totalOutputTokens ?? 0) +
      (sessionDetail.totalCacheReadTokens ?? 0) +
      (sessionDetail.totalCacheCreationTokens ?? 0),
  }

  const cost = richData?.cost ?? {
    totalUsd: 0,
    inputCostUsd: 0,
    outputCostUsd: 0,
    cacheReadCostUsd: 0,
    cacheCreationCostUsd: 0,
    cacheSavingsUsd: 0,
    hasUnpricedUsage: true,
    unpricedInputTokens: 0,
    unpricedOutputTokens: 0,
    unpricedCacheReadTokens: 0,
    unpricedCacheCreationTokens: 0,
    pricedTokenCoverage: 0,
    totalCostSource: 'no_cost_data' as const,
  }

  return {
    id: sessionDetail.id,
    project: sessionDetail.project,
    projectDisplayName: sessionDetail.displayName,
    projectPath: sessionDetail.projectPath,
    gitBranch: richData?.gitBranch ?? sessionDetail.gitBranch ?? null,
    worktreeBranch: null,
    isWorktree: false,
    effectiveBranch: richData?.gitBranch ?? sessionDetail.gitBranch ?? null,
    status: 'done',
    model: richData?.model ?? sessionDetail.primaryModel ?? null,
    turnCount: richData?.turnCount ?? sessionDetail.turnCount,
    tokens,
    contextWindowTokens: richData?.contextWindowTokens ?? 0,
    cost,
    cacheStatus: richData?.cacheStatus ?? 'unknown',
    subAgents: richData?.subAgents,
    teamName: richData?.teamName ?? null,
    progressItems: richData?.progressItems,
    tasks: sessionDetail.tasks,
    compactCount: sessionDetail.compactionCount,
    startedAt: sessionDetail.firstMessageAt ?? undefined,
    lastActivityAt: sessionDetail.modifiedAt,
    lastUserMessage: richData?.lastUserMessage ?? undefined,
    slug: sessionDetail.slug ?? null,
    hasPlans: sessionDetail.hasPlans ?? false,
    // Statusline fields are not available for history sessions
    modelDisplayName: null,
    statuslineCostUsd: null,
    statuslineTotalDurationMs: null,
    statuslineLinesAdded: null,
    statuslineLinesRemoved: null,
    statuslineInputTokens: null,
    statuslineOutputTokens: null,
    statuslineCacheReadTokens: null,
    statuslineCacheCreationTokens: null,
    statuslineCwd: null,
    statuslineProjectDir: null,
    historyExtras: {
      sessionDetail,
      sessionInfo,
    },
    terminalMessages,
  }
}
