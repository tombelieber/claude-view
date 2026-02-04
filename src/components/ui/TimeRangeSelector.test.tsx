import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TimeRangeSelector } from './TimeRangeSelector'
import { BREAKPOINTS } from '../../hooks/use-media-query'

type TimeRange = '7d' | '30d' | '90d' | 'all'

const defaultOptions = [
  { value: '7d' as TimeRange, label: '7 days' },
  { value: '30d' as TimeRange, label: '30 days' },
  { value: '90d' as TimeRange, label: '90 days' },
  { value: 'all' as TimeRange, label: 'All time' },
]

// Mock matchMedia
function createMockMediaQueryList(matches: boolean) {
  return {
    matches,
    media: '',
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }
}

function setMobileViewport() {
  window.matchMedia = vi.fn((query: string) => {
    // Mobile: max-width: 639px
    if (query === `(max-width: ${BREAKPOINTS.sm - 1}px)`) {
      return createMockMediaQueryList(true)
    }
    return createMockMediaQueryList(false)
  })
}

function setDesktopViewport() {
  window.matchMedia = vi.fn(() => createMockMediaQueryList(false))
}

describe('TimeRangeSelector', () => {
  describe('desktop viewport (>=640px)', () => {
    beforeEach(() => {
      setDesktopViewport()
    })

    it('renders segmented control on desktop', () => {
      render(
        <TimeRangeSelector
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
        />
      )

      // Should render radiogroup (SegmentedControl)
      expect(screen.getByRole('radiogroup')).toBeInTheDocument()
      expect(screen.getByRole('radio', { name: '7 days' })).toBeInTheDocument()
      expect(screen.getByRole('radio', { name: '30 days' })).toBeInTheDocument()
    })

    it('calls onChange when option clicked on desktop', async () => {
      const user = userEvent.setup()
      const onChange = vi.fn()

      render(
        <TimeRangeSelector
          value="30d"
          onChange={onChange}
          options={defaultOptions}
        />
      )

      await user.click(screen.getByRole('radio', { name: '7 days' }))
      expect(onChange).toHaveBeenCalledWith('7d')
    })
  })

  describe('mobile viewport (<640px)', () => {
    beforeEach(() => {
      setMobileViewport()
    })

    it('renders dropdown select on mobile', () => {
      render(
        <TimeRangeSelector
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
        />
      )

      // Should render combobox (native select)
      const select = screen.getByRole('combobox')
      expect(select).toBeInTheDocument()
      expect(select).toHaveValue('30d')
    })

    it('renders all options in dropdown', () => {
      render(
        <TimeRangeSelector
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
        />
      )

      const options = screen.getAllByRole('option')
      expect(options).toHaveLength(4)
      expect(options[0]).toHaveTextContent('7 days')
      expect(options[1]).toHaveTextContent('30 days')
      expect(options[2]).toHaveTextContent('90 days')
      expect(options[3]).toHaveTextContent('All time')
    })

    it('calls onChange when option selected on mobile', async () => {
      const user = userEvent.setup()
      const onChange = vi.fn()

      render(
        <TimeRangeSelector
          value="30d"
          onChange={onChange}
          options={defaultOptions}
        />
      )

      await user.selectOptions(screen.getByRole('combobox'), '7d')
      expect(onChange).toHaveBeenCalledWith('7d')
    })

    it('has minimum 44px touch target on mobile', () => {
      render(
        <TimeRangeSelector
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
        />
      )

      const select = screen.getByRole('combobox')
      // Check for min-h-[44px] and min-w-[44px] classes
      expect(select.className).toContain('min-h-[44px]')
      expect(select.className).toContain('min-w-[44px]')
    })
  })

  describe('accessibility', () => {
    it('has aria-label on desktop', () => {
      setDesktopViewport()
      render(
        <TimeRangeSelector
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
          ariaLabel="Select period"
        />
      )

      expect(screen.getByRole('radiogroup')).toHaveAttribute(
        'aria-label',
        'Select period'
      )
    })

    it('has aria-label on mobile', () => {
      setMobileViewport()
      render(
        <TimeRangeSelector
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
          ariaLabel="Select period"
        />
      )

      expect(screen.getByRole('combobox')).toHaveAttribute(
        'aria-label',
        'Select period'
      )
    })

    it('uses default aria-label when not provided', () => {
      setMobileViewport()
      render(
        <TimeRangeSelector
          value="30d"
          onChange={() => {}}
          options={defaultOptions}
        />
      )

      expect(screen.getByRole('combobox')).toHaveAttribute(
        'aria-label',
        'Time range selector'
      )
    })
  })
})
