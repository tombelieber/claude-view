import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { AIGenerationStats } from './AIGenerationStats'
import { formatModelName } from '../lib/format-model'

// Mock hooks
const mockUseAIGenerationStats = vi.fn()
vi.mock('../hooks/use-ai-generation', () => ({
  useAIGenerationStats: (...args: unknown[]) => mockUseAIGenerationStats(...args),
  formatTokens: (tokens: number | null | undefined) => {
    if (tokens === null || tokens === undefined) return '--'
    if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1)}M`
    if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)}k`
    return tokens.toString()
  },
  formatLineCount: (lines: number, showPlus = true) => {
    if (lines === 0) return '0'
    if (showPlus && lines > 0) return `+${lines}`
    return lines.toString()
  },
}))

vi.mock('../hooks/use-media-query', () => ({
  useIsMobile: () => false,
}))

function makeStats(overrides = {}) {
  return {
    linesAdded: 100,
    linesRemoved: 20,
    filesCreated: 5,
    totalInputTokens: 50000,
    totalOutputTokens: 30000,
    cacheReadTokens: 10000,
    cacheCreationTokens: 5000,
    tokensByModel: [
      { model: 'claude-opus-4-5-20251101', inputTokens: 30000, outputTokens: 20000 },
      { model: 'claude-sonnet-4-20250514', inputTokens: 20000, outputTokens: 10000 },
    ],
    tokensByProject: [
      { project: 'my-app', inputTokens: 40000, outputTokens: 25000 },
      { project: 'my-lib', inputTokens: 10000, outputTokens: 5000 },
    ],
    cost: {
      totalCostUsd: 1.50,
      inputCostUsd: 0.50,
      outputCostUsd: 0.60,
      cacheReadCostUsd: 0.10,
      cacheCreationCostUsd: 0.30,
      cacheSavingsUsd: 0.25,
    },
    ...overrides,
  }
}

describe('AIGenerationStats', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('loading state', () => {
    it('should show skeleton when loading', () => {
      mockUseAIGenerationStats.mockReturnValue({ data: null, isLoading: true, error: null })
      const { container } = render(<AIGenerationStats />)
      expect(container.querySelector('.animate-pulse')).toBeInTheDocument()
    })
  })

  describe('error state', () => {
    it('should show error card with retry button when error occurs', () => {
      const mockRefetch = vi.fn()
      mockUseAIGenerationStats.mockReturnValue({ data: null, isLoading: false, error: new Error('fail'), refetch: mockRefetch })
      render(<AIGenerationStats />)
      expect(screen.getByText('Failed to load AI generation stats')).toBeInTheDocument()
      expect(screen.getByText('Retry')).toBeInTheDocument()
    })

    it('should call refetch when retry button is clicked', () => {
      const mockRefetch = vi.fn()
      mockUseAIGenerationStats.mockReturnValue({ data: null, isLoading: false, error: new Error('fail'), refetch: mockRefetch })
      render(<AIGenerationStats />)
      screen.getByText('Retry').click()
      expect(mockRefetch).toHaveBeenCalledOnce()
    })
  })

  describe('null data state', () => {
    it('should return null when stats is null', () => {
      mockUseAIGenerationStats.mockReturnValue({ data: null, isLoading: false, error: null })
      const { container } = render(<AIGenerationStats />)
      expect(container.innerHTML).toBe('')
    })
  })

  describe('no meaningful data state', () => {
    it('should return null when all tokens and files are zero', () => {
      mockUseAIGenerationStats.mockReturnValue({
        data: makeStats({
          totalInputTokens: 0,
          totalOutputTokens: 0,
          filesCreated: 0,
          tokensByModel: [],
          tokensByProject: [],
        }),
        isLoading: false,
        error: null,
      })
      const { container } = render(<AIGenerationStats />)
      expect(container.innerHTML).toBe('')
    })
  })

  describe('success state', () => {
    beforeEach(() => {
      mockUseAIGenerationStats.mockReturnValue({
        data: makeStats(),
        isLoading: false,
        error: null,
      })
    })

    it('should render token usage by model and project sections', () => {
      render(<AIGenerationStats />)

      expect(screen.getByText('Token Usage by Model')).toBeInTheDocument()
      expect(screen.getByText('Top Projects by Token Usage')).toBeInTheDocument()
    })

    it('should render TokenBreakdown with total tokens processed', () => {
      render(<AIGenerationStats />)
      // Total: 50000 + 30000 + 10000 + 5000 = 95000 -> 95.0k
      expect(screen.getByText('Total Tokens Processed')).toBeInTheDocument()
      expect(screen.getByText('95.0k')).toBeInTheDocument()
    })

    it('should render TokenBreakdown detail cards', () => {
      render(<AIGenerationStats />)
      // The 4 detail cards: Cache Read, Cache Write, Output, Fresh Input
      expect(screen.getAllByText('Cache Read').length).toBeGreaterThanOrEqual(1)
      expect(screen.getAllByText('Cache Write').length).toBeGreaterThanOrEqual(1)
      expect(screen.getAllByText('Output').length).toBeGreaterThanOrEqual(1)
      expect(screen.getAllByText('Fresh Input').length).toBeGreaterThanOrEqual(1)
    })

    it('should render CostBreakdownCard when cost data is present', () => {
      render(<AIGenerationStats />)
      expect(screen.getByText('Estimated Total Cost')).toBeInTheDocument()
    })
  })

  describe('token usage by model', () => {
    it('should render ProgressBars for each model', () => {
      mockUseAIGenerationStats.mockReturnValue({
        data: makeStats(),
        isLoading: false,
        error: null,
      })
      render(<AIGenerationStats />)

      expect(screen.getByText('Token Usage by Model')).toBeInTheDocument()
      expect(screen.getByText('Claude Opus 4.5')).toBeInTheDocument()
      expect(screen.getByText('Claude Sonnet 4')).toBeInTheDocument()
    })
  })

  describe('token usage by project', () => {
    it('should render ProgressBars for each project', () => {
      mockUseAIGenerationStats.mockReturnValue({
        data: makeStats(),
        isLoading: false,
        error: null,
      })
      render(<AIGenerationStats />)

      expect(screen.getByText('Top Projects by Token Usage')).toBeInTheDocument()
      expect(screen.getByText('my-app')).toBeInTheDocument()
      expect(screen.getByText('my-lib')).toBeInTheDocument()
    })
  })

  describe('empty breakdowns', () => {
    it('should hide model section when tokensByModel is empty', () => {
      mockUseAIGenerationStats.mockReturnValue({
        data: makeStats({ tokensByModel: [], filesCreated: 1 }),
        isLoading: false,
        error: null,
      })
      render(<AIGenerationStats />)

      expect(screen.queryByText('Token Usage by Model')).not.toBeInTheDocument()
    })

    it('should hide project section when tokensByProject is empty', () => {
      mockUseAIGenerationStats.mockReturnValue({
        data: makeStats({ tokensByProject: [], filesCreated: 1 }),
        isLoading: false,
        error: null,
      })
      render(<AIGenerationStats />)

      expect(screen.queryByText('Top Projects by Token Usage')).not.toBeInTheDocument()
    })
  })

  describe('token breakdown visibility', () => {
    it('should show TokenBreakdown when token data is present', () => {
      mockUseAIGenerationStats.mockReturnValue({
        data: makeStats(),
        isLoading: false,
        error: null,
      })
      render(<AIGenerationStats />)

      expect(screen.getByText('Total Tokens Processed')).toBeInTheDocument()
    })

    it('should hide TokenBreakdown when all token values are zero', () => {
      mockUseAIGenerationStats.mockReturnValue({
        data: makeStats({
          totalInputTokens: 0,
          totalOutputTokens: 0,
          cacheReadTokens: 0,
          cacheCreationTokens: 0,
          filesCreated: 1,
        }),
        isLoading: false,
        error: null,
      })
      render(<AIGenerationStats />)

      expect(screen.queryByText('Total Tokens Processed')).not.toBeInTheDocument()
    })

    it('should hide CostBreakdownCard when cost totalCostUsd is zero', () => {
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
        error: null,
      })
      render(<AIGenerationStats />)

      expect(screen.queryByText('Estimated Total Cost')).not.toBeInTheDocument()
    })
  })

  describe('time range passthrough', () => {
    it('should pass timeRange to hook', () => {
      mockUseAIGenerationStats.mockReturnValue({ data: null, isLoading: true, error: null })
      const timeRange = { from: 1000, to: 2000 }
      render(<AIGenerationStats timeRange={timeRange} />)

      expect(mockUseAIGenerationStats).toHaveBeenCalledWith(timeRange, undefined, undefined)
    })
  })
})

describe('formatModelName', () => {
  describe('known model IDs (lookup table)', () => {
    it('should return friendly name for claude-opus-4-5-20251101', () => {
      expect(formatModelName('claude-opus-4-5-20251101')).toBe('Claude Opus 4.5')
    })

    it('should return friendly name for claude-opus-4-20250514', () => {
      expect(formatModelName('claude-opus-4-20250514')).toBe('Claude Opus 4')
    })

    it('should return friendly name for claude-sonnet-4-20250514', () => {
      expect(formatModelName('claude-sonnet-4-20250514')).toBe('Claude Sonnet 4')
    })

    it('should return friendly name for claude-3-5-sonnet-20241022', () => {
      expect(formatModelName('claude-3-5-sonnet-20241022')).toBe('Claude 3.5 Sonnet')
    })

    it('should return friendly name for claude-3-5-haiku-20241022', () => {
      expect(formatModelName('claude-3-5-haiku-20241022')).toBe('Claude 3.5 Haiku')
    })

    it('should return friendly name for claude-3-opus-20240229', () => {
      expect(formatModelName('claude-3-opus-20240229')).toBe('Claude 3 Opus')
    })

    it('should return friendly name for claude-3-haiku-20240307', () => {
      expect(formatModelName('claude-3-haiku-20240307')).toBe('Claude 3 Haiku')
    })
  })

  describe('unknown model IDs (regex fallback)', () => {
    it('should parse unknown claude model with date suffix', () => {
      expect(formatModelName('claude-3-5-opus-20260101')).toBe('Claude 3.5 Opus')
    })

    it('should parse unknown claude model without date suffix', () => {
      expect(formatModelName('claude-3-turbo')).toBe('Claude 3 Turbo')
    })

    it('should handle claude-4-5 pattern with version dots', () => {
      expect(formatModelName('claude-4-5-haiku-20260601')).toBe('Claude 4.5 Haiku')
    })

    it('should capitalize model variant names', () => {
      expect(formatModelName('claude-3-mega-20260101')).toBe('Claude 3 Mega')
    })

    it('should handle model with multiple name parts', () => {
      expect(formatModelName('claude-3-super-fast-20260101')).toBe('Claude 3 Super Fast')
    })

    it('should handle claude-opus-4-6 (no date suffix)', () => {
      expect(formatModelName('claude-opus-4-6')).toBe('Claude Opus 4.6')
    })

    it('should handle claude-opus-4-1-20250805 (with date suffix)', () => {
      expect(formatModelName('claude-opus-4-1-20250805')).toBe('Claude Opus 4.1')
    })

    it('should handle claude-sonnet-4-5-20250929 (with date suffix)', () => {
      expect(formatModelName('claude-sonnet-4-5-20250929')).toBe('Claude Sonnet 4.5')
    })

    it('should handle claude-haiku-4-5-20251001 (with date suffix)', () => {
      expect(formatModelName('claude-haiku-4-5-20251001')).toBe('Claude Haiku 4.5')
    })

    it('should handle claude-opus-4-20250514 (major only, with date)', () => {
      expect(formatModelName('claude-opus-4-20250514')).toBe('Claude Opus 4')
    })

    it('should handle claude-haiku-4-20250514 (major only, with date)', () => {
      expect(formatModelName('claude-haiku-4-20250514')).toBe('Claude Haiku 4')
    })

    it('should handle hypothetical claude-sonnet-5-0-20270101', () => {
      expect(formatModelName('claude-sonnet-5-0-20270101')).toBe('Claude Sonnet 5.0')
    })

    it('should handle hypothetical claude-opus-5-20270601 (major only)', () => {
      expect(formatModelName('claude-opus-5-20270601')).toBe('Claude Opus 5')
    })
  })

  describe('edge cases', () => {
    it('should return empty string as-is', () => {
      expect(formatModelName('')).toBe('')
    })

    it('should return non-claude model ID as-is', () => {
      expect(formatModelName('gpt-4-turbo')).toBe('gpt-4-turbo')
    })

    it('should capitalize short non-claude single-word string', () => {
      expect(formatModelName('unknown')).toBe('Unknown')
    })

    it('should handle claude with only two parts (below 3-part threshold)', () => {
      expect(formatModelName('claude-opus')).toBe('claude-opus')
    })

    it('should capitalize bare single-word "claude"', () => {
      expect(formatModelName('claude')).toBe('Claude')
    })

    it('should capitalize bare alias "opus"', () => {
      expect(formatModelName('opus')).toBe('Opus')
    })

    it('should capitalize bare alias "sonnet"', () => {
      expect(formatModelName('sonnet')).toBe('Sonnet')
    })

    it('should capitalize bare alias "haiku"', () => {
      expect(formatModelName('haiku')).toBe('Haiku')
    })
  })
})
