import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { CostBreakdown } from './CostBreakdown'
import type { LiveSession } from './use-live-sessions'

describe('CostBreakdown', () => {
  it('renders total cost', () => {
    const cost: LiveSession['cost'] = {
      totalUsd: 2.34,
      inputCostUsd: 1.5,
      outputCostUsd: 0.84,
      cacheReadCostUsd: 0.1,
      cacheCreationCostUsd: 0.05,
      cacheSavingsUsd: 0.5,
      hasUnpricedUsage: false,
      unpricedInputTokens: 0,
      unpricedOutputTokens: 0,
      unpricedCacheReadTokens: 0,
      unpricedCacheCreationTokens: 0,
      pricedTokenCoverage: 1,
      totalCostSource: 'computed_priced_tokens_full',
    }
    render(<CostBreakdown cost={cost} subAgents={[]} />)
    // $2.34 appears in both Total Cost header and subtotal row — use getAllByText
    expect(screen.getAllByText('$2.34').length).toBeGreaterThan(0)
  })

  it('renders sub-agent costs when present', () => {
    const cost: LiveSession['cost'] = {
      totalUsd: 5.0,
      inputCostUsd: 3.0,
      outputCostUsd: 2.0,
      cacheReadCostUsd: 0,
      cacheCreationCostUsd: 0,
      cacheSavingsUsd: 0,
      hasUnpricedUsage: false,
      unpricedInputTokens: 0,
      unpricedOutputTokens: 0,
      unpricedCacheReadTokens: 0,
      unpricedCacheCreationTokens: 0,
      pricedTokenCoverage: 1,
      totalCostSource: 'computed_priced_tokens_full',
    }
    const subAgents = [
      {
        toolUseId: 'toolu_01',
        agentType: 'Explore',
        description: 'Search',
        status: 'complete' as const,
        startedAt: 0,
        costUsd: 0.5,
      },
      {
        toolUseId: 'toolu_02',
        agentType: 'code-reviewer',
        description: 'Review',
        status: 'complete' as const,
        startedAt: 0,
        costUsd: 0.3,
      },
    ]
    render(<CostBreakdown cost={cost} subAgents={subAgents} />)
    expect(screen.getByText('$0.50')).toBeInTheDocument()
    expect(screen.getByText('$0.30')).toBeInTheDocument()
  })

  it('shows unavailable when all usage is unpriced', () => {
    const cost: LiveSession['cost'] = {
      totalUsd: 0,
      inputCostUsd: 0,
      outputCostUsd: 0,
      cacheReadCostUsd: 0,
      cacheCreationCostUsd: 0,
      cacheSavingsUsd: 0,
      hasUnpricedUsage: true,
      unpricedInputTokens: 1000,
      unpricedOutputTokens: 500,
      unpricedCacheReadTokens: 0,
      unpricedCacheCreationTokens: 0,
      pricedTokenCoverage: 0,
      totalCostSource: 'computed_priced_tokens_partial',
    }
    const tokens: LiveSession['tokens'] = {
      inputTokens: 1000,
      outputTokens: 500,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
      cacheCreation5mTokens: 0,
      cacheCreation1hrTokens: 0,
      totalTokens: 1500,
    }
    render(<CostBreakdown cost={cost} tokens={tokens} subAgents={[]} />)
    expect(screen.getAllByText('Unavailable').length).toBeGreaterThan(0)
    expect(screen.getByText(/Partial pricing:/)).toBeInTheDocument()
  })
})
