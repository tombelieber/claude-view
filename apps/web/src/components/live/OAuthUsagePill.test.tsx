import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { OAuthUsage } from '../../hooks/use-oauth-usage'
import { OAuthUsagePill } from './OAuthUsagePill'

const mockRefetch = vi.fn()
const mockUseOAuthUsage = vi.fn()
vi.mock('../../hooks/use-oauth-usage', () => ({
  useOAuthUsage: (...args: unknown[]) => mockUseOAuthUsage(...args),
}))

const mockUseAuthIdentity = vi.fn()
vi.mock('../../hooks/use-auth-identity', () => ({
  useAuthIdentity: (...args: unknown[]) => mockUseAuthIdentity(...args),
}))

const MULTI_TIER_DATA: OAuthUsage = {
  hasAuth: true,
  error: null,
  plan: 'Max',
  tiers: [
    {
      id: 'session',
      label: 'Session (5hr)',
      percentage: 11,
      resetAt: '2026-02-20T05:00:00Z',
      spent: null,
    },
    {
      id: 'weekly',
      label: 'Weekly (7 day)',
      percentage: 46,
      resetAt: '2026-02-24T09:00:00Z',
      spent: null,
    },
    {
      id: 'weekly_sonnet',
      label: 'Weekly Sonnet',
      percentage: 3,
      resetAt: '2026-02-25T11:00:00Z',
      spent: null,
    },
    {
      id: 'extra',
      label: 'Extra usage',
      percentage: 100,
      resetAt: '2026-03-01T00:00:00Z',
      spent: '$51.25 / $50.00 spent',
    },
  ],
}

describe('OAuthUsagePill', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mockUseAuthIdentity.mockReturnValue({
      data: {
        hasAuth: true,
        email: 'test@example.com',
        orgName: 'Test Corp',
        subscriptionType: 'max',
        authMethod: 'claude.ai',
      },
      isLoading: false,
    })
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
      mockUseOAuthUsage.mockReturnValue({
        data: undefined,
        isLoading: true,
        error: null,
        refetch: mockRefetch,
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
      })
      render(<OAuthUsagePill />)
      expect(screen.getByText('Loading usage...')).toBeInTheDocument()
    })

    it('returns null when hasAuth is false', () => {
      mockUseOAuthUsage.mockReturnValue({
        data: { hasAuth: false, error: null, plan: null, tiers: [] },
        isLoading: false,
        error: null,
        refetch: mockRefetch,
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
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
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
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
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
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
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
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
          tiers: [
            {
              id: 'extra',
              label: 'Usage',
              percentage: 62,
              resetAt: '2026-03-01T00:00:00Z',
              spent: '$50 / $80 spent',
            },
          ],
        },
        isLoading: false,
        error: null,
        refetch: mockRefetch,
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
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
          tiers: [
            { id: 'session', label: 'Session', percentage: 85, resetAt: '2026-02-20T04:00:00Z' },
          ],
        },
        isLoading: false,
        error: null,
        refetch: mockRefetch,
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
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
          tiers: [
            { id: 'session', label: 'Session', percentage: 98, resetAt: '2026-02-20T04:00:00Z' },
          ],
        },
        isLoading: false,
        error: null,
        refetch: mockRefetch,
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
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
        dataUpdatedAt: Date.now(),
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
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

      // Identity info (from mocked useAuthIdentity)
      expect(wrapper.textContent).toContain('test@example.com')
      expect(wrapper.textContent).toContain('Test Corp')
    })

    it('does NOT refetch on tooltip open (server-driven polling only)', async () => {
      mockUseOAuthUsage.mockReturnValue({
        data: MULTI_TIER_DATA,
        isLoading: false,
        error: null,
        refetch: mockRefetch,
        dataUpdatedAt: Date.now(),
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
      })
      render(<OAuthUsagePill />)

      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
      await user.hover(screen.getByText('11%'))

      await waitFor(() => {
        expect(getPopperWrapper()).not.toBeNull()
      })

      expect(mockRefetch).not.toHaveBeenCalled()
    })

    it('hides redundant org name matching email pattern', async () => {
      mockUseAuthIdentity.mockReturnValue({
        data: {
          hasAuth: true,
          email: 'alice@example.com',
          orgName: "alice's Organization",
          subscriptionType: 'max',
          authMethod: 'claude.ai',
        },
        isLoading: false,
      })
      mockUseOAuthUsage.mockReturnValue({
        data: MULTI_TIER_DATA,
        isLoading: false,
        error: null,
        refetch: mockRefetch,
        dataUpdatedAt: Date.now(),
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
      })

      render(<OAuthUsagePill />)

      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
      await user.hover(screen.getByText('11%'))

      await waitFor(() => {
        expect(getPopperWrapper()).not.toBeNull()
      })

      const wrapper = getPopperWrapper()!
      expect(wrapper.textContent).toContain('alice@example.com')
      // Redundant org name should be hidden
      expect(wrapper.textContent).not.toContain("alice's Organization")
    })

    it('shows refresh button and calls forceRefresh on click', async () => {
      const mockForceRefreshMutate = vi.fn()
      mockUseOAuthUsage.mockReturnValue({
        data: MULTI_TIER_DATA,
        isLoading: false,
        error: null,
        refetch: mockRefetch,
        dataUpdatedAt: Date.now(),
        forceRefresh: { mutate: mockForceRefreshMutate, isPending: false, isError: false },
      })

      render(<OAuthUsagePill />)

      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
      await user.hover(screen.getByText('11%'))

      await waitFor(() => {
        expect(document.querySelector('[data-radix-popper-content-wrapper]')).not.toBeNull()
      })

      // Find the refresh button inside the visible popper (Radix duplicates content for a11y)
      const popperWrapper = document.querySelector('[data-radix-popper-content-wrapper]')!
      const refreshButton = popperWrapper.querySelector(
        'button[title="Refresh usage"]',
      ) as HTMLElement
      await user.click(refreshButton)

      expect(mockForceRefreshMutate).toHaveBeenCalledTimes(1)
    })
  })

  describe('2026-04-30 expanded tier set', () => {
    function getPopperWrapper(): Element | null {
      return document.querySelector('[data-radix-popper-content-wrapper]')
    }

    beforeEach(() => {
      vi.useFakeTimers({ shouldAdvanceTime: true })
      vi.setSystemTime(new Date('2026-04-30T00:00:00Z'))
    })

    afterEach(() => {
      vi.useRealTimers()
    })

    it('renders section headings when multiple tier kinds are present', async () => {
      const data: OAuthUsage = {
        hasAuth: true,
        error: null,
        plan: 'Max',
        tiers: [
          {
            id: 'five_hour',
            label: 'Session (5hr)',
            kind: 'session',
            percentage: 12,
            resetAt: '2026-04-30T05:00:00Z',
          },
          {
            id: 'seven_day',
            label: 'Weekly',
            kind: 'window',
            percentage: 5,
            resetAt: '2026-05-01T05:00:00Z',
          },
          {
            id: 'seven_day_opus',
            label: 'Weekly Opus',
            kind: 'window',
            percentage: 0,
            resetAt: '2026-05-07T05:00:00Z',
          },
          {
            id: 'seven_day_omelette',
            label: 'Weekly · omelette',
            kind: 'other',
            percentage: 8,
            resetAt: '2026-05-03T05:00:00Z',
          },
          {
            id: 'extra',
            label: 'Extra usage',
            kind: 'extra',
            percentage: 24.68,
            resetAt: '',
            spent: '12.34 EUR / 50.00 EUR spent',
            currency: 'EUR',
          },
        ],
      }
      mockUseOAuthUsage.mockReturnValue({
        data,
        isLoading: false,
        error: null,
        dataUpdatedAt: Date.now(),
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
      })

      render(<OAuthUsagePill />)
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
      await user.hover(screen.getByText('12%'))
      await waitFor(() => expect(getPopperWrapper()).not.toBeNull())

      const wrapper = getPopperWrapper()!
      // Section headings present
      expect(wrapper.textContent).toContain('Session')
      expect(wrapper.textContent).toContain('Weekly windows')
      expect(wrapper.textContent).toContain('Additional tiers')
      expect(wrapper.textContent).toContain('Extra usage')
      // New tiers visible by their backend-curated label
      expect(wrapper.textContent).toContain('Weekly Opus')
      expect(wrapper.textContent).toContain('Weekly · omelette')
      // Non-USD currency badge appears
      expect(wrapper.textContent).toContain('EUR')
      expect(wrapper.textContent).toContain('12.34 EUR / 50.00 EUR spent')
    })

    it('hydrates kind from id for old backends that omit it', async () => {
      // Pre-2026-04 backend: no `kind` field at all. UI must still group correctly.
      const data: OAuthUsage = {
        hasAuth: true,
        error: null,
        plan: 'Max',
        tiers: [
          {
            id: 'session',
            label: 'Session (5hr)',
            percentage: 12,
            resetAt: '2026-04-30T05:00:00Z',
          },
          {
            id: 'weekly',
            label: 'Weekly (7 day)',
            percentage: 5,
            resetAt: '2026-05-01T05:00:00Z',
          },
        ],
      }
      mockUseOAuthUsage.mockReturnValue({
        data,
        isLoading: false,
        error: null,
        dataUpdatedAt: Date.now(),
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
      })

      render(<OAuthUsagePill />)
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
      await user.hover(screen.getByText('12%'))
      await waitFor(() => expect(getPopperWrapper()).not.toBeNull())

      const wrapper = getPopperWrapper()!
      // Both tiers render with their original labels
      expect(wrapper.textContent).toContain('Session (5hr)')
      expect(wrapper.textContent).toContain('Weekly (7 day)')
      // Section headings appear because two distinct kinds were inferred
      expect(wrapper.textContent).toContain('Session')
      expect(wrapper.textContent).toContain('Weekly windows')
    })

    it('USD spent string omits the currency badge in the tooltip', async () => {
      // Verifies the rule: when currency is USD (or absent), no extra " · USD"
      // suffix is rendered — the `$` symbol in the spent string already says it.
      const data: OAuthUsage = {
        hasAuth: true,
        error: null,
        plan: null,
        tiers: [
          {
            id: 'five_hour',
            label: 'Session (5hr)',
            kind: 'session',
            percentage: 12,
            resetAt: '2026-04-30T05:00:00Z',
          },
          {
            id: 'extra',
            label: 'Extra usage',
            kind: 'extra',
            percentage: 50,
            resetAt: '',
            spent: '$25.00 / $50.00 spent',
            currency: 'USD',
          },
        ],
      }
      mockUseOAuthUsage.mockReturnValue({
        data,
        isLoading: false,
        error: null,
        dataUpdatedAt: Date.now(),
        forceRefresh: { mutate: vi.fn(), isPending: false, isError: false },
      })
      render(<OAuthUsagePill />)
      const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime })
      await user.hover(screen.getByText('12%'))
      await waitFor(() => expect(getPopperWrapper()).not.toBeNull())

      const wrapper = getPopperWrapper()!
      expect(wrapper.textContent).toContain('$25.00 / $50.00 spent')
      // No "· USD" badge appended when the spent string is USD.
      expect(wrapper.textContent).not.toContain('· USD')
    })
  })
})
