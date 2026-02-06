import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, act } from '@testing-library/react'
import { DateRangePicker, type DateRangePickerProps } from './DateRangePicker'

// Capture onSelect callbacks so tests can simulate date picks
let startOnSelect: ((date: Date | undefined) => void) | null = null
let endOnSelect: ((date: Date | undefined) => void) | null = null

vi.mock('react-day-picker', () => ({
  DayPicker: (props: { selected?: Date; onSelect?: (date: Date | undefined) => void; disabled?: unknown }) => {
    // Distinguish start vs end calendar by the disabled prop shape.
    // Start calendar has { after: ... }, end has { before: ..., after: ... }
    const isEndCalendar = props.disabled && typeof props.disabled === 'object' && 'before' in props.disabled
    if (isEndCalendar) {
      endOnSelect = props.onSelect ?? null
    } else {
      startOnSelect = props.onSelect ?? null
    }
    return (
      <div
        data-testid={isEndCalendar ? 'end-calendar' : 'start-calendar'}
        data-selected={props.selected?.toISOString() ?? 'none'}
      />
    )
  },
}))

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

/** Simulate selecting a date on the start calendar */
function selectStartDate(date: Date) {
  act(() => {
    startOnSelect?.(date)
  })
}

/** Simulate selecting a date on the end calendar */
function selectEndDate(date: Date) {
  act(() => {
    endOnSelect?.(date)
  })
}

describe('DateRangePicker', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    startOnSelect = null
    endOnSelect = null
  })

  describe('initial render', () => {
    it('should show "Custom" label when value is null', () => {
      renderPicker()
      expect(screen.getByText('Custom')).toBeInTheDocument()
    })

    it('should show formatted date range when value is provided', () => {
      renderPicker({ value: { from: jan1, to: jan15 } })
      // The component formats dates as "MMM D" e.g. "Jan 1 - Jan 15"
      expect(screen.getByText(/Jan 1/)).toBeInTheDocument()
      expect(screen.getByText(/Jan 15/)).toBeInTheDocument()
    })

    it('should not show popover initially', () => {
      renderPicker()
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })

    it('should set aria-expanded=false on trigger initially', () => {
      renderPicker()
      const trigger = screen.getByRole('button')
      expect(trigger).toHaveAttribute('aria-expanded', 'false')
    })
  })

  describe('popover open/close via trigger click', () => {
    it('should open popover on trigger click', () => {
      renderPicker()
      clickTrigger()
      expect(screen.getByRole('dialog')).toBeInTheDocument()
    })

    it('should set aria-expanded=true when popover is open', () => {
      renderPicker()
      const trigger = clickTrigger()
      expect(trigger).toHaveAttribute('aria-expanded', 'true')
    })

    it('should close popover on second trigger click', () => {
      renderPicker()
      clickTrigger()
      expect(screen.getByRole('dialog')).toBeInTheDocument()
      clickTrigger()
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })

    it('should render start and end date calendars when open', () => {
      renderPicker()
      clickTrigger()
      expect(screen.getByTestId('start-calendar')).toBeInTheDocument()
      expect(screen.getByTestId('end-calendar')).toBeInTheDocument()
    })

    it('should render labels for start and end date', () => {
      renderPicker()
      clickTrigger()
      expect(screen.getByText('Start date')).toBeInTheDocument()
      expect(screen.getByText('End date')).toBeInTheDocument()
    })
  })

  describe('popover close via Escape', () => {
    it('should close popover when Escape is pressed', () => {
      renderPicker()
      clickTrigger()
      expect(screen.getByRole('dialog')).toBeInTheDocument()

      fireEvent.keyDown(document, { key: 'Escape' })
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })

    it('should not call onChange when closing via Escape', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      selectStartDate(jan1)
      selectEndDate(jan15)

      fireEvent.keyDown(document, { key: 'Escape' })
      expect(onChange).not.toHaveBeenCalled()
    })
  })

  describe('popover close via click outside', () => {
    it('should close popover when clicking outside', () => {
      renderPicker()
      clickTrigger()
      expect(screen.getByRole('dialog')).toBeInTheDocument()

      // mousedown on document body (outside both trigger and popover)
      fireEvent.mouseDown(document.body)
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })

    it('should not call onChange when closing via click outside', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      selectStartDate(jan1)
      selectEndDate(jan15)

      fireEvent.mouseDown(document.body)
      expect(onChange).not.toHaveBeenCalled()
    })
  })

  describe('temp state isolation (draft does not commit until Apply)', () => {
    it('should not call onChange when selecting dates without clicking Apply', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      selectStartDate(jan1)
      selectEndDate(jan15)

      // No Apply click -- onChange should not be called
      expect(onChange).not.toHaveBeenCalled()
    })

    it('should call onChange only after clicking Apply', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      selectStartDate(jan1)
      selectEndDate(jan15)

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
      selectStartDate(jan1)
      selectEndDate(jan15)

      fireEvent.click(screen.getByRole('button', { name: 'Apply' }))
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    })
  })

  describe('date ordering enforcement (from/to swap)', () => {
    it('should swap dates if from is after to when applying', () => {
      const { onChange } = renderPicker()
      clickTrigger()

      // Select end date before start date (reversed order)
      selectStartDate(jan20)
      selectEndDate(jan1)

      fireEvent.click(screen.getByRole('button', { name: 'Apply' }))

      expect(onChange).toHaveBeenCalledWith({
        from: jan1,  // swapped: earlier date becomes from
        to: jan20,   // swapped: later date becomes to
      })
    })

    it('should handle same date for from and to', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      selectStartDate(jan15)
      selectEndDate(jan15)

      fireEvent.click(screen.getByRole('button', { name: 'Apply' }))

      expect(onChange).toHaveBeenCalledWith({
        from: jan15,
        to: jan15,
      })
    })
  })

  describe('Apply button disabled when dates incomplete', () => {
    it('should disable Apply when no dates are selected', () => {
      renderPicker()
      clickTrigger()

      const applyBtn = screen.getByRole('button', { name: 'Apply' })
      expect(applyBtn).toBeDisabled()
    })

    it('should disable Apply when only start date is selected', () => {
      renderPicker()
      clickTrigger()
      selectStartDate(jan1)

      const applyBtn = screen.getByRole('button', { name: 'Apply' })
      expect(applyBtn).toBeDisabled()
    })

    it('should disable Apply when only end date is selected', () => {
      renderPicker()
      clickTrigger()
      selectEndDate(jan15)

      const applyBtn = screen.getByRole('button', { name: 'Apply' })
      expect(applyBtn).toBeDisabled()
    })

    it('should enable Apply when both dates are selected', () => {
      renderPicker()
      clickTrigger()
      selectStartDate(jan1)
      selectEndDate(jan15)

      const applyBtn = screen.getByRole('button', { name: 'Apply' })
      expect(applyBtn).not.toBeDisabled()
    })

    it('should not call onChange when Apply is clicked while disabled', () => {
      const { onChange } = renderPicker()
      clickTrigger()
      selectStartDate(jan1)
      // No end date selected

      const applyBtn = screen.getByRole('button', { name: 'Apply' })
      fireEvent.click(applyBtn)

      expect(onChange).not.toHaveBeenCalled()
    })
  })

  describe('reset on cancel (temp values revert)', () => {
    it('should revert temp dates when closing via Escape after modification', () => {
      renderPicker({ value: { from: jan1, to: jan15 } })

      // Open, modify dates
      clickTrigger()
      selectStartDate(jan20)
      selectEndDate(feb1)

      // Cancel via Escape
      fireEvent.keyDown(document, { key: 'Escape' })

      // Re-open -- draft should be reset to original value
      clickTrigger()
      const startCal = screen.getByTestId('start-calendar')
      const endCal = screen.getByTestId('end-calendar')

      expect(startCal).toHaveAttribute('data-selected', jan1.toISOString())
      expect(endCal).toHaveAttribute('data-selected', jan15.toISOString())
    })

    it('should revert temp dates when closing via click outside after modification', () => {
      renderPicker({ value: { from: jan1, to: jan15 } })

      // Open, modify dates
      clickTrigger()
      selectStartDate(jan20)
      selectEndDate(feb1)

      // Cancel via click outside
      fireEvent.mouseDown(document.body)

      // Re-open -- draft should be reset to original value
      clickTrigger()
      const startCal = screen.getByTestId('start-calendar')
      const endCal = screen.getByTestId('end-calendar')

      expect(startCal).toHaveAttribute('data-selected', jan1.toISOString())
      expect(endCal).toHaveAttribute('data-selected', jan15.toISOString())
    })

    it('should show "none" for temp dates when value is null and popover is reopened after cancel', () => {
      renderPicker({ value: null })

      clickTrigger()
      selectStartDate(jan1)
      selectEndDate(jan15)

      // Cancel
      fireEvent.keyDown(document, { key: 'Escape' })

      // Re-open
      clickTrigger()
      const startCal = screen.getByTestId('start-calendar')
      const endCal = screen.getByTestId('end-calendar')

      expect(startCal).toHaveAttribute('data-selected', 'none')
      expect(endCal).toHaveAttribute('data-selected', 'none')
    })
  })

  describe('prevIsOpenRef pattern (draft resets on open transition, not on prop change)', () => {
    it('should initialize draft from value when opening', () => {
      renderPicker({ value: { from: jan1, to: jan15 } })

      clickTrigger()
      const startCal = screen.getByTestId('start-calendar')
      const endCal = screen.getByTestId('end-calendar')

      expect(startCal).toHaveAttribute('data-selected', jan1.toISOString())
      expect(endCal).toHaveAttribute('data-selected', jan15.toISOString())
    })

    it('should not reset draft when value prop changes while popover is open', () => {
      const { rerender, onChange } = renderPicker({ value: { from: jan1, to: jan15 } })

      // Open popover and modify draft
      clickTrigger()
      selectStartDate(jan20)
      selectEndDate(feb1)

      // Parent re-renders with a new value prop (simulating external change)
      rerender(
        <DateRangePicker
          value={{ from: new Date(2024, 2, 1), to: new Date(2024, 2, 15) }}
          onChange={onChange}
        />
      )

      // Draft should still reflect the user's in-progress selection, not the new prop
      const startCal = screen.getByTestId('start-calendar')
      const endCal = screen.getByTestId('end-calendar')

      expect(startCal).toHaveAttribute('data-selected', jan20.toISOString())
      expect(endCal).toHaveAttribute('data-selected', feb1.toISOString())
    })

    it('should reset draft to latest value on next open transition after prop change', () => {
      const newFrom = new Date(2024, 2, 1)
      const newTo = new Date(2024, 2, 15)

      const { rerender, onChange } = renderPicker({ value: { from: jan1, to: jan15 } })

      // Open, modify, close
      clickTrigger()
      selectStartDate(jan20)
      fireEvent.keyDown(document, { key: 'Escape' })

      // Parent updates value
      rerender(
        <DateRangePicker
          value={{ from: newFrom, to: newTo }}
          onChange={onChange}
        />
      )

      // Re-open -- draft should reflect the NEW prop value
      clickTrigger()
      const startCal = screen.getByTestId('start-calendar')
      const endCal = screen.getByTestId('end-calendar')

      expect(startCal).toHaveAttribute('data-selected', newFrom.toISOString())
      expect(endCal).toHaveAttribute('data-selected', newTo.toISOString())
    })

    it('should not reset draft on repeated renders while popover stays open', () => {
      const { rerender, onChange } = renderPicker({ value: { from: jan1, to: jan15 } })

      clickTrigger()
      selectStartDate(jan20)

      // Simulate multiple parent re-renders (e.g. React Query refetch) with same value
      for (let i = 0; i < 5; i++) {
        rerender(
          <DateRangePicker
            value={{ from: jan1, to: jan15 }}
            onChange={onChange}
          />
        )
      }

      // Draft should still show user's modification
      const startCal = screen.getByTestId('start-calendar')
      expect(startCal).toHaveAttribute('data-selected', jan20.toISOString())
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
