import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

/**
 * Orders sub-agents for compact card display: running (active) agents first,
 * then newest-spawned first within each tier.
 *
 * Sub-agents arrive appended in spawn order (oldest-first), so a naive
 * `slice(0, N)` surfaces the oldest agents and pushes the ones currently
 * doing work into the `+N more` overflow. Sorting active + newest first means
 * the visible slice always shows the most relevant agents and the overflow
 * hides the *oldest* ones instead.
 *
 * Pure and non-mutating — sub-agent lists arrive as React props / store-owned
 * data and must never be sorted in place. `Array.prototype.sort` is stable
 * (ES2019+), so agents sharing a tier and `startedAt` keep their spawn order.
 *
 * @see https://github.com/tombelieber/claude-view/issues/62
 */
export function sortSubAgentsForCard(subAgents: SubAgentInfo[]): SubAgentInfo[] {
  const runningRank = (a: SubAgentInfo): number => (a.status === 'running' ? 0 : 1)
  return [...subAgents].sort((a, b) => {
    const tier = runningRank(a) - runningRank(b)
    if (tier !== 0) return tier
    return b.startedAt - a.startedAt
  })
}
