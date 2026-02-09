import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, act } from '@testing-library/react'
import { DateRangePicker, type DateRangePickerProps } from './DateRangePicker'
import type { DateRange } from 'react-day-picker'

// Capture onSelect callback so tests can simulate range picks
let rangeOnSelect: ((range: DateRange | undefined) => void) | null = null
let lastSelected: DateRange | undefined = undefined

vi.mock('react-day-picker', () => ({
  DayPicker: (props: {
    mode: string
    selected?: DateRange
    onSelect?: (range: DateRange | undefined) => void
    numberOfMonths?: number
    disabled?: unknown
  }) => {
    rangeOnSelect = props.onSelect ?? null
    lastSelected = props.selected
    return (
      <div
        data-testid="range-calendar"
        data-mode={props.mode}
        data-months={props.numberOfMonths}
        data-from={props.selected?.from?.toISOString() ?? 'none'}
        data-to={props.selected?.to?.toISOString() ?? 'none'}
      />
    )
  },
}))

// Mock Radix Popover to render inline (no portal) with proper open/close behavior.
// We use a module-level ref so all sub-components share the same open/onOpenChange.
vi.mock('@radix-ui/react-popover', () => {
  const React = require('react')

  // Shared state between Root, Trigger, Content, Close
  const ctx = { open: false, onOpenChange: (_b: boolean) => {} }

  const Root = ({ open, onOpenChange, children }: { open: boolean; onOpenChange: (o: boolean) => void; children: React.ReactNode }) => {
    ctx.open = open
    ctx.onOpenChange = onOpenChange
    return <>{children}</>
  }

  const Trigger = React.forwardRef(({ asChild, children, ...props }: any, ref: any) => {
    const handleClick = (e: any) => {
      props.onClick?.(e)
      children?.props?.onClick?.(e)
      ctx.onOpenChange(!ctx.open)
    }
    if (asChild && React.isValidElement(children)) {
      return React.cloneElement(children, { ...props, ref, onClick: handleClick })
    }
    return <button ref={ref} {...props} onClick={handleClick}>{children}</button>
  })
  Trigger.displayName = 'Trigger'

  const Portal = ({ children }: { children: React.ReactNode }) => <>{children}</>

  const Content = React.forwardRef(({ children, ...props }: any, ref: any) => {
    if (!ctx.open) return null
    return <div ref={ref} {...props}>{children}</div>
  })
  Content.displayName = 'Content'

  const Close = React.forwardRef(({ asChild, children, ...props }: any, ref: any) => {
    const handleClick = (e: any) => {
      props.onClick?.(e)
      children?.props?.onClick?.(e)
      ctx.onOpenChange(false)
    }
    if (asChild && React.isValidElement(children)) {
      return React.cloneElement(children, { ...props, ref, onClick: handleClick })
    }
    return <button ref={ref} {...props} onClick={handleClick}>{children}</button>
  })
  Close.displayName = 'Close'

  return { Root, Trigger, Portal, Content, Close }
})

vi.mock('lucide-react', () => ({
  ChevronLeft: () => <span data-testid="chevron-left" />,
  ChevronRight: () => <span data-testid="chevron-right" />,
  Calendar: () => <span data-testid="calendar-icon" />,
}))

const jan1 = new Date(2024, 0, 1)
const jan15 = new Date(2024, 0, 15)
const jan20 = new Date(2024, 0, 20)
const feb1 = new Date(2024, 1, 1)

function renderPicker(overrides: Partial<DateRangePickerProps> = {}) {
  const defaultProps: DateRangePickerProps = {
    value: null,
    onChange: vi.fn(),
    ...overrides,
  }
  const result = render(<DateRangePicker {...defaultProps} />)
  return { ...result, onChange: defaultProps.onChange as ReturnType<typeof vi.fn> }
}

/** Click the trigger button to toggle the popover */
function clickTrigger() {
  const trigger = screen.getByRole('button', { name: /custom|jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec|\.\.\./i })
  fireEvent.click(trigger)
  return trigger
}

/** Simulate selecting a date range via the DayPicker */
function selectRange(from: Date, to: Date) {
  act(() => {
    rangeOnSelect?.({ from, to })
  })
}

/** Simulate selecting only a start date (incomplete range) */
function selectPartialRange(from: Date) {
  act(() => {
    rangeOnSelect?.({ from, to: undefined })
  })
}

describe('DateRangePicker', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    rangeOnSelect = null
    lastSelected = undefined
  })

  describe('initial render', () => {
    it('should show "Custom" label when value is null', () => {
      renderPicker()
      expect(screen.getByText('Custom')).toBeInTheDocument()
    })

    it('should show formatted date range when value is provided', () => {
      renderPicker({ value: { from: jan1, to: jan15 } })
      expect(screen.getByText(/Jan 1/)).toBeInTheDocument()
      expect(screen.getByText(/Jan 15/)).toBeInTheDocument()
    })

    it('should not show popover initially', () => {
      renderPicker()
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
  })

  describe('popover open/close', () => {
    it('should open popover on trigger click', () => {
      renderPicker()
      clickTrigger()
      expect(screen.getByRole('dialog')).toBeInTheDocument()
    })

    it('should render a single range calendar with two months', () => {
      renderPicker()
      clickTrigger()
      const cal = screen.getByTestId('range-calendar')
      expect(cal).toHaveAttribute('data-mode', 'range')
      expect(cal).toHaveAttribute('data-months', '2')
    })

    it('should close popover on second trigger click', () => {
      renderPicker()
      clickTrigger()
      expect(screen.getByRole('dialog')).toBeInTheDocument()
      clickTrigger()
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
  })

  describe('preset quick-select buttons', () => {
    it('should render preset buttons', () => {
      renderPicker()
      clickTrigger()
      expect(screen.getByText('Last 7 days')).toBeInTheDocument()
      expect(screen.getByText('Last 14 days')).toBeInTheDocument()
      expect(screen.getByText('Last 30 days')).toBeInTheDocument()
      expect(screen.getByText('Last 90 days')).toBeInTheDocument()
    })

    it('should update temp range when preset is clicked', () => {
      renderPicker()
      clickTrigger()
      fireEvent.click(screen.getByText('Last 7 days'))

      const cal = screen.getByTestId('range-calendar')
      expect(cal.getAttribute('data-from')).not.toBe('none')
      expect(cal.getAttribute('data-to')).not.toBe('none')
    })

    it('should not call onChange when preset is clicked (only on Apply)', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      fireEvent.click(screen.getByText('Last 30 days'))
      expect(onChange).not.toHaveBeenCalled()
    })
  })

  describe('temp state isolation (draft does not commit until Apply)', () => {
    it('should not call onChange when selecting dates without clicking Apply', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      selectRange(jan1, jan15)
      expect(onChange).not.toHaveBeenCalled()
    })

    it('should call onChange only after clicking Apply', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      selectRange(jan1, jan15)

      const applyBtn = screen.getByRole('button', { name: 'Apply' })
      fireEvent.click(applyBtn)

      expect(onChange).toHaveBeenCalledTimes(1)
      expect(onChange).toHaveBeenCalledWith({
        from: jan1,
        to: jan15,
      })
    })

    it('should close popover after Apply', () => {
      renderPicker()
      clickTrigger()
      selectRange(jan1, jan15)

      fireEvent.click(screen.getByRole('button', { name: 'Apply' }))
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
  })

  describe('date ordering enforcement (from/to swap)', () => {
    it('should swap dates if from is after to when applying', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      selectRange(jan20, jan1)

      fireEvent.click(screen.getByRole('button', { name: 'Apply' }))

      expect(onChange).toHaveBeenCalledWith({
        from: jan1,
        to: jan20,
      })
    })

    it('should handle same date for from and to', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      selectRange(jan15, jan15)

      fireEvent.click(screen.getByRole('button', { name: 'Apply' }))

      expect(onChange).toHaveBeenCalledWith({
        from: jan15,
        to: jan15,
      })
    })
  })

  describe('Apply button disabled when range incomplete', () => {
    it('should disable Apply when no range is selected', () => {
      renderPicker()
      clickTrigger()

      const applyBtn = screen.getByRole('button', { name: 'Apply' })
      expect(applyBtn).toBeDisabled()
    })

    it('should disable Apply when only start date is selected', () => {
      renderPicker()
      clickTrigger()
      selectPartialRange(jan1)

      const applyBtn = screen.getByRole('button', { name: 'Apply' })
      expect(applyBtn).toBeDisabled()
    })

    it('should enable Apply when full range is selected', () => {
      renderPicker()
      clickTrigger()
      selectRange(jan1, jan15)

      const applyBtn = screen.getByRole('button', { name: 'Apply' })
      expect(applyBtn).not.toBeDisabled()
    })

    it('should not call onChange when Apply is clicked while disabled', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      selectPartialRange(jan1)

      const applyBtn = screen.getByRole('button', { name: 'Apply' })
      fireEvent.click(applyBtn)

      expect(onChange).not.toHaveBeenCalled()
    })
  })

  describe('Cancel button', () => {
    it('should render a Cancel button', () => {
      renderPicker()
      clickTrigger()
      expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument()
    })

    it('should close popover when Cancel is clicked', () => {
      renderPicker()
      clickTrigger()
      expect(screen.getByRole('dialog')).toBeInTheDocument()

      fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })

    it('should not call onChange when Cancel is clicked', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      selectRange(jan1, jan15)

      fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
      expect(onChange).not.toHaveBeenCalled()
    })
  })

  describe('prevIsOpenRef pattern (draft resets on open transition, not on prop change)', () => {
    it('should initialize draft from value when opening', () => {
      renderPicker({ value: { from: jan1, to: jan15 } })

      clickTrigger()
      const cal = screen.getByTestId('range-calendar')

      expect(cal).toHaveAttribute('data-from', jan1.toISOString())
      expect(cal).toHaveAttribute('data-to', jan15.toISOString())
    })

    it('should not reset draft when value prop changes while popover is open', () => {
      const { rerender, onChange } = renderPicker({ value: { from: jan1, to: jan15 } })

      clickTrigger()
      selectRange(jan20, feb1)

      // Parent re-renders with a new value prop
      rerender(
        <DateRangePicker
          value={{ from: new Date(2024, 2, 1), to: new Date(2024, 2, 15) }}
          onChange={onChange}
        />
      )

      // Draft should still reflect user's in-progress selection
      const cal = screen.getByTestId('range-calendar')
      expect(cal).toHaveAttribute('data-from', jan20.toISOString())
      expect(cal).toHaveAttribute('data-to', feb1.toISOString())
    })

    it('should reset draft to latest value on next open transition after prop change', () => {
      const newFrom = new Date(2024, 2, 1)
      const newTo = new Date(2024, 2, 15)

      const { rerender, onChange } = renderPicker({ value: { from: jan1, to: jan15 } })

      // Open, modify, close
      clickTrigger()
      selectRange(jan20, feb1)
      clickTrigger() // close

      // Parent updates value
      rerender(
        <DateRangePicker
          value={{ from: newFrom, to: newTo }}
          onChange={onChange}
        />
      )

      // Re-open -- draft should reflect the NEW prop value
      clickTrigger()
      const cal = screen.getByTestId('range-calendar')

      expect(cal).toHaveAttribute('data-from', newFrom.toISOString())
      expect(cal).toHaveAttribute('data-to', newTo.toISOString())
    })

    it('should not reset draft on repeated renders while popover stays open', () => {
      const { rerender, onChange } = renderPicker({ value: { from: jan1, to: jan15 } })

      clickTrigger()
      selectRange(jan20, feb1)

      // Simulate multiple parent re-renders with same value
      for (let i = 0; i < 5; i++) {
        rerender(
          <DateRangePicker
            value={{ from: jan1, to: jan15 }}
            onChange={onChange}
          />
        )
      }

      // Draft should still show user's modification
      const cal = screen.getByTestId('range-calendar')
      expect(cal).toHaveAttribute('data-from', jan20.toISOString())
    })
  })

  describe('footer summary', () => {
    it('should show "Select a range" when no dates selected', () => {
      renderPicker()
      clickTrigger()
      expect(screen.getByText('Select a range')).toBeInTheDocument()
    })

    it('should show partial range when only from is selected', () => {
      renderPicker()
      clickTrigger()
      selectPartialRange(jan1)
      expect(screen.getByText(/Jan 1/)).toBeInTheDocument()
      expect(screen.getByText(/\.\.\./)).toBeInTheDocument()
    })

    it('should show full range when both dates selected', () => {
      renderPicker()
      clickTrigger()
      selectRange(jan1, jan15)
      // Footer shows the range summary
      const footer = screen.getAllByText(/Jan 1/).length
      expect(footer).toBeGreaterThanOrEqual(1)
    })
  })

  describe('accessibility', () => {
    it('should have aria-haspopup="dialog" on trigger', () => {
      renderPicker()
      const trigger = screen.getByRole('button')
      expect(trigger).toHaveAttribute('aria-haspopup', 'dialog')
    })

    it('should have aria-label on the dialog', () => {
      renderPicker()
      clickTrigger()
      const dialog = screen.getByRole('dialog')
      expect(dialog).toHaveAttribute('aria-label', 'Select custom date range')
    })
  })
})
