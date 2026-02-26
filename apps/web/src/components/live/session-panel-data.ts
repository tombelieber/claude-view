// src/components/live/session-panel-data.ts
import type { LiveSession } from './use-live-sessions'
import type { AgentState } from './types'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
import type { SessionDetail } from '../../types/generated/SessionDetail'
import type { SessionInfo } from '../../types/generated'
import type { RichSessionData } from '../../types/generated/RichSessionData'
import type { RichMessage } from './RichPane'

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
    isEstimated: boolean
  }
  cacheStatus: 'warm' | 'cold' | 'unknown'

  // Sub-agents
  subAgents?: SubAgentInfo[]

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
    status: session.status,
    model: session.model,
    turnCount: session.turnCount,
    tokens: session.tokens,
    contextWindowTokens: session.contextWindowTokens,
    cost: session.cost,
    cacheStatus: session.cacheStatus,
    subAgents: session.subAgents,
    startedAt: session.startedAt,
    lastActivityAt: session.lastActivityAt,
    lastUserMessage: session.lastUserMessage,
    lastCacheHitAt: session.lastCacheHitAt,
    agentState: session.agentState,
    pid: session.pid,
    currentActivity: session.currentActivity,
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
    totalTokens: (sessionDetail.totalInputTokens ?? 0) + (sessionDetail.totalOutputTokens ?? 0),
  }

  const cost = richData?.cost ?? {
    totalUsd: 0,
    inputCostUsd: 0,
    outputCostUsd: 0,
    cacheReadCostUsd: 0,
    cacheCreationCostUsd: 0,
    cacheSavingsUsd: 0,
    isEstimated: true,
  }

  return {
    id: sessionDetail.id,
    project: sessionDetail.project,
    projectDisplayName: sessionDetail.project, // history doesn't have displayName
    projectPath: sessionDetail.projectPath,
    gitBranch: richData?.gitBranch ?? sessionDetail.gitBranch ?? null,
    status: 'done',
    model: richData?.model ?? sessionDetail.primaryModel ?? null,
    turnCount: richData?.turnCount ?? sessionDetail.turnCount,
    tokens,
    contextWindowTokens: richData?.contextWindowTokens ?? 0,
    cost,
    cacheStatus: richData?.cacheStatus ?? 'unknown',
    subAgents: richData?.subAgents,
    lastUserMessage: richData?.lastUserMessage ?? undefined,
    historyExtras: {
      sessionDetail,
      sessionInfo,
    },
    terminalMessages,
  }
}
