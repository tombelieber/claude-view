import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { OAuthUsagePill } from './OAuthUsagePill'
import type { OAuthUsage } from '../../hooks/use-oauth-usage'

const mockRefetch = vi.fn()
const mockUseOAuthUsage = vi.fn()
vi.mock('../../hooks/use-oauth-usage', () => ({
  useOAuthUsage: (...args: unknown[]) => mockUseOAuthUsage(...args),
}))

const MULTI_TIER_DATA: OAuthUsage = {
  hasAuth: true,
  error: null,
  plan: 'Max',
  tiers: [
    { id: 'session', label: 'Session (5hr)', percentage: 11, resetAt: '2026-02-20T05:00:00Z', spent: null },
    { id: 'weekly', label: 'Weekly (7 day)', percentage: 46, resetAt: '2026-02-24T09:00:00Z', spent: null },
    { id: 'weekly_sonnet', label: 'Weekly Sonnet', percentage: 3, resetAt: '2026-02-25T11:00:00Z', spent: null },
    { id: 'extra', label: 'Extra usage', percentage: 100, resetAt: '2026-03-01T00:00:00Z', spent: '$51.25 / $50.00 spent' },
  ],
}

describe('OAuthUsagePill', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('basic states', () => {
    beforeEach(() => {
      vi.useFakeTimers()
      vi.setSystemTime(new Date('2026-02-20T00:00:00Z'))
    })

    afterEach(() => {
      vi.useRealTimers()
    })

    it('renders loading state', () => {
      mockUseOAuthUsage.mockReturnValue({ data: undefined, isLoading: true, error: null, refetch: mockRefetch })
      render(<OAuthUsagePill />)
      expect(screen.getByText('Loading usage...')).toBeInTheDocument()
    })

    it('returns null when hasAuth is false', () => {
      mockUseOAuthUsage.mockReturnValue({
        data: { hasAuth: false, error: null, plan: null, tiers: [] },
        isLoading: false,
        error: null,
        refetch: mockRefetch,
      })
      const { container } = render(<OAuthUsagePill />)
      expect(container.innerHTML).toBe('')
    })

    it('returns null when tiers is empty', () => {
      mockUseOAuthUsage.mockReturnValue({
        data: { hasAuth: true, error: null, plan: null, tiers: [] },
        isLoading: false,
        error: null,
        refetch: mockRefetch,
      })
      const { container } = render(<OAuthUsagePill />)
      expect(container.innerHTML).toBe('')
    })

    it('shows error state with title tooltip', () => {
      mockUseOAuthUsage.mockReturnValue({
        data: undefined,
        isLoading: false,
        error: new Error('Network failure'),
        refetch: mockRefetch,
      })
      render(<OAuthUsagePill />)
      const el = screen.getByText('Usage unavailable')
      expect(el).toBeInTheDocument()
      expect(el).toHaveAttribute('title', 'Network failure')
    })

    it('renders compact pill with session tier percentage and reset time', () => {
      mockUseOAuthUsage.mockReturnValue({
        data: MULTI_TIER_DATA,
        isLoading: false,
        error: null,
        refetch: mockRefetch,
      })
      render(<OAuthUsagePill />)

      // Session tier: 11%, resets at 5am = 5 hours from midnight
      expect(screen.getByText('11%')).toBeInTheDocument()
      expect(screen.getByText('5h')).toBeInTheDocument()
    })

    it('falls back to first tier when no session tier exists', () => {
      mockUseOAuthUsage.mockReturnValue({
        data: {
          hasAuth: true,
          error: null,
          plan: null,
          tiers: [{ id: 'extra', label: 'Usage', percentage: 62, resetAt: '2026-03-01T00:00:00Z', spent: '$50 / $80 spent' }],
        },
        isLoading: false,
        error: null,
        refetch: mockRefetch,
      })
      render(<OAuthUsagePill />)

      expect(screen.getByText('62%')).toBeInTheDocument()
      expect(screen.getByText('9d')).toBeInTheDocument()
    })
  })

  describe('bar colors', () => {
    it('shows amber color for >80% usage', () => {
      mockUseOAuthUsage.mockReturnValue({
        data: {
          hasAuth: true,
          error: null,
          plan: null,
          tiers: [{ id: 'session', label: 'Session', percentage: 85, resetAt: '2026-02-20T04:00:00Z' }],
        },
        isLoading: false,
        error: null,
        refetch: mockRefetch,
      })
      const { container } = render(<OAuthUsagePill />)
      expect(container.querySelector('.bg-amber-500')).toBeInTheDocument()
    })

    it('shows red color for >95% usage', () => {
      mockUseOAuthUsage.mockReturnValue({
        data: {
          hasAuth: true,
          error: null,
          plan: null,
          tiers: [{ id: 'session', label: 'Session', percentage: 98, resetAt: '2026-02-20T04:00:00Z' }],
        },
        isLoading: false,
        error: null,
        refetch: mockRefetch,
      })
      const { container } = render(<OAuthUsagePill />)
      expect(container.querySelector('.bg-red-500')).toBeInTheDocument()
    })
  })

  describe('tooltip popover (hover)', () => {
    /** The visible tooltip popper wrapper (not the hidden a11y span). */
    function getPopperWrapper(): Element | null {
      return document.querySelector('[data-radix-popper-content-wrapper]')
    }

    beforeEach(() => {
      vi.useFakeTimers({ shouldAdvanceTime: true })
    })

    afterEach(() => {
      vi.useRealTimers()
    })

    it('shows all tiers in tooltip on hover', async () => {
      mockUseOAuthUsage.mockReturnValue({
        data: MULTI_TIER_DATA,
        isLoading: false,
        error: null,
        refetch: mockRefetch,
      })
      render(<OAuthUsagePill />)

      expect(getPopperWrapper()).toBeNull()

      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
      await user.hover(screen.getByText('11%'))

      await waitFor(() => {
        expect(getPopperWrapper()).not.toBeNull()
      })

      const wrapper = getPopperWrapper()!
      // All tier labels
      expect(wrapper.textContent).toContain('Session (5hr)')
      expect(wrapper.textContent).toContain('Weekly (7 day)')
      expect(wrapper.textContent).toContain('Weekly Sonnet')
      expect(wrapper.textContent).toContain('Extra usage')
      // Plan badge
      expect(wrapper.textContent).toContain('Max')
      // Dollar amounts
      expect(wrapper.textContent).toContain('$51.25 / $50.00 spent')
    })

    it('triggers refetch when tooltip opens', async () => {
      mockUseOAuthUsage.mockReturnValue({
        data: MULTI_TIER_DATA,
        isLoading: false,
        error: null,
        refetch: mockRefetch,
      })
      render(<OAuthUsagePill />)

      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
      await user.hover(screen.getByText('11%'))

      await waitFor(() => {
        expect(getPopperWrapper()).not.toBeNull()
      })

      expect(mockRefetch).toHaveBeenCalled()
    })
  })
})
