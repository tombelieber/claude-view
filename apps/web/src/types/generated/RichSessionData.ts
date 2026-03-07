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
    hasUnpricedUsage: boolean
    unpricedInputTokens: number
    unpricedOutputTokens: number
    unpricedCacheReadTokens: number
    unpricedCacheCreationTokens: number
    pricedTokenCoverage: number
    totalCostSource: 'computed_priced_tokens_full' | 'computed_priced_tokens_partial'
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
  lastUserFile: string | null
}
