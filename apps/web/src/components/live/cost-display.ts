export interface UnpricedCostMeta {
  hasUnpricedUsage: boolean
  unpricedInputTokens: number
  unpricedOutputTokens: number
  unpricedCacheReadTokens: number
  unpricedCacheCreationTokens: number
  pricedTokenCoverage: number
}

export function hasUnavailableCost(
  totalUsd: number,
  cost: Pick<UnpricedCostMeta, 'hasUnpricedUsage'> | null | undefined,
  totalTokens: number,
): boolean {
  return totalUsd === 0 && Boolean(cost?.hasUnpricedUsage) && totalTokens > 0
}

export function unpricedTokenTotal(cost: UnpricedCostMeta | null | undefined): number {
  if (!cost) return 0
  return (
    cost.unpricedInputTokens +
    cost.unpricedOutputTokens +
    cost.unpricedCacheReadTokens +
    cost.unpricedCacheCreationTokens
  )
}

export function pricedCoveragePercent(
  cost: Pick<UnpricedCostMeta, 'pricedTokenCoverage'> | null | undefined,
): number {
  if (!cost || !Number.isFinite(cost.pricedTokenCoverage)) return 0
  return Math.max(0, Math.min(100, Math.round(cost.pricedTokenCoverage * 100)))
}
