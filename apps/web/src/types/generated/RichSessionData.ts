import type { ProgressItem } from './ProgressItem'
import type { SubAgentInfo } from './SubAgentInfo'

export interface RichSessionData {
  tokens: {
    inputTokens: number
    outputTokens: number
    cacheReadTokens: number
    cacheCreationTokens: number
    cacheCreation5mTokens: number
    cacheCreation1hrTokens: number
    totalTokens: number
  }
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
  subAgents: SubAgentInfo[]
  progressItems: ProgressItem[]
  contextWindowTokens: number
  model: string | null
  gitBranch: string | null
  turnCount: number
  firstUserMessage: string | null
  lastUserMessage: string | null
  lastCacheHitAt: number | null
}
