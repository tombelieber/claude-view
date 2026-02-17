import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { CostBreakdown } from './CostBreakdown'

describe('CostBreakdown', () => {
  it('renders total cost', () => {
    const cost = { totalUsd: 2.34, inputCostUsd: 1.50, outputCostUsd: 0.84, cacheReadCostUsd: 0.10, cacheCreationCostUsd: 0.05, cacheSavingsUsd: 0.50 }
    render(<CostBreakdown cost={cost} subAgents={[]} />)
    expect(screen.getByText('$2.34')).toBeInTheDocument()
  })

  it('renders sub-agent costs when present', () => {
    const cost = { totalUsd: 5.00, inputCostUsd: 3.00, outputCostUsd: 2.00, cacheReadCostUsd: 0, cacheCreationCostUsd: 0, cacheSavingsUsd: 0 }
    const subAgents = [
      { toolUseId: 'toolu_01', agentType: 'Explore', description: 'Search', status: 'complete' as const, startedAt: 0, costUsd: 0.50 },
      { toolUseId: 'toolu_02', agentType: 'code-reviewer', description: 'Review', status: 'complete' as const, startedAt: 0, costUsd: 0.30 },
    ]
    render(<CostBreakdown cost={cost} subAgents={subAgents} />)
    expect(screen.getByText('$0.50')).toBeInTheDocument()
    expect(screen.getByText('$0.30')).toBeInTheDocument()
  })
})
