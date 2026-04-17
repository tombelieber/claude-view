import { render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { TokenCostSummary } from './TokenCostSummary'

const mockUseAIGenerationStats = vi.fn()
vi.mock('../hooks/use-ai-generation', () => ({
  useAIGenerationStats: (...args: unknown[]) => mockUseAIGenerationStats(...args),
  formatTokens: (tokens: number | null | undefined) => {
    if (tokens === null || tokens === undefined) return '--'
    if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1)}M`
    if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)}k`
    return tokens.toString()
  },
}))

function makeStats(overrides = {}) {
  return {
    linesAdded: 0,
    linesRemoved: 0,
    filesCreated: 0,
    totalInputTokens: 50_000,
    totalOutputTokens: 30_000,
    cacheReadTokens: 10_000,
    cacheCreationTokens: 5_000,
    tokensByModel: [],
    tokensByProject: [],
    cost: {
      totalCostUsd: 1.5,
      inputCostUsd: 0.5,
      outputCostUsd: 0.6,
      cacheReadCostUsd: 0.1,
      cacheCreationCostUsd: 0.3,
      cacheSavingsUsd: 0.25,
    },
    ...overrides,
  }
}

describe('TokenCostSummary', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders skeleton while loading', () => {
    mockUseAIGenerationStats.mockReturnValue({ data: null, isLoading: true })
    const { container } = render(<TokenCostSummary />)
    expect(container.querySelector('.animate-pulse')).toBeInTheDocument()
  })

  it('renders nothing when stats is null', () => {
    mockUseAIGenerationStats.mockReturnValue({ data: null, isLoading: false })
    const { container } = render(<TokenCostSummary />)
    expect(container.innerHTML).toBe('')
  })

  it('renders TokenBreakdown and CostBreakdownCard when both are present', () => {
    mockUseAIGenerationStats.mockReturnValue({ data: makeStats(), isLoading: false })
    render(<TokenCostSummary />)
    expect(screen.getByText('Total Tokens Processed')).toBeInTheDocument()
    expect(screen.getByText('Total Cost')).toBeInTheDocument()
  })

  it('hides TokenBreakdown when no token data', () => {
    mockUseAIGenerationStats.mockReturnValue({
      data: makeStats({
        totalInputTokens: 0,
        totalOutputTokens: 0,
        cacheReadTokens: 0,
        cacheCreationTokens: 0,
      }),
      isLoading: false,
    })
    render(<TokenCostSummary />)
    expect(screen.queryByText('Total Tokens Processed')).not.toBeInTheDocument()
    expect(screen.getByText('Total Cost')).toBeInTheDocument()
  })

  it('hides CostBreakdownCard when totalCostUsd is zero', () => {
    mockUseAIGenerationStats.mockReturnValue({
      data: makeStats({
        cost: {
          totalCostUsd: 0,
          inputCostUsd: 0,
          outputCostUsd: 0,
          cacheReadCostUsd: 0,
          cacheCreationCostUsd: 0,
          cacheSavingsUsd: 0,
        },
      }),
      isLoading: false,
    })
    render(<TokenCostSummary />)
    expect(screen.getByText('Total Tokens Processed')).toBeInTheDocument()
    expect(screen.queryByText('Total Cost')).not.toBeInTheDocument()
  })

  it('renders nothing when neither tokens nor cost have data', () => {
    mockUseAIGenerationStats.mockReturnValue({
      data: makeStats({
        totalInputTokens: 0,
        totalOutputTokens: 0,
        cacheReadTokens: 0,
        cacheCreationTokens: 0,
        cost: {
          totalCostUsd: 0,
          inputCostUsd: 0,
          outputCostUsd: 0,
          cacheReadCostUsd: 0,
          cacheCreationCostUsd: 0,
          cacheSavingsUsd: 0,
        },
      }),
      isLoading: false,
    })
    const { container } = render(<TokenCostSummary />)
    expect(container.innerHTML).toBe('')
  })

  it('passes timeRange/project/branch to the hook', () => {
    mockUseAIGenerationStats.mockReturnValue({ data: null, isLoading: true })
    render(<TokenCostSummary timeRange={{ from: 1, to: 2 }} project="foo" branch="main" />)
    expect(mockUseAIGenerationStats).toHaveBeenCalledWith({ from: 1, to: 2 }, 'foo', 'main')
  })
})
