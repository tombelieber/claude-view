import { useState, useRef, useEffect } from 'react'
import { DayPicker } from 'react-day-picker'
import { ChevronLeft, ChevronRight, Calendar } from 'lucide-react'
import { cn } from '../../lib/utils'

export interface DateRangeValue {
  from: Date
  to: Date
}

export interface DateRangePickerProps {
  /** Current date range value */
  value: DateRangeValue | null
  /** Callback when date range changes (after Apply is clicked) */
  onChange: (value: DateRangeValue | null) => void
  /** Optional className for additional styling */
  className?: string
}

/**
 * DateRangePicker - A popover with start/end date pickers and Apply button.
 *
 * Design:
 * - Trigger button shows current date range or "Custom"
 * - Popover contains two mini calendars (start and end)
 * - Apply button commits the selection
 * - Click outside or Escape closes without applying
 *
 * Accessibility:
 * - Uses button + popover pattern
 * - Keyboard navigation within calendars
 * - Focus trap within popover
 */
export function DateRangePicker({
  value,
  onChange,
  className,
}: DateRangePickerProps) {
  const [isOpen, setIsOpen] = useState(false)
  const [tempFrom, setTempFrom] = useState<Date | undefined>(value?.from)
  const [tempTo, setTempTo] = useState<Date | undefined>(value?.to)
  const popoverRef = useRef<HTMLDivElement>(null)
  const triggerRef = useRef<HTMLButtonElement>(null)

  // Sync temp values when value prop changes
  useEffect(() => {
    setTempFrom(value?.from)
    setTempTo(value?.to)
  }, [value])

  // Handle click outside to close
  useEffect(() => {
    if (!isOpen) return

    const handleClickOutside = (e: MouseEvent) => {
      if (
        popoverRef.current &&
        !popoverRef.current.contains(e.target as Node) &&
        triggerRef.current &&
        !triggerRef.current.contains(e.target as Node)
      ) {
        setIsOpen(false)
        // Reset temp values on cancel
        setTempFrom(value?.from)
        setTempTo(value?.to)
      }
    }

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsOpen(false)
        setTempFrom(value?.from)
        setTempTo(value?.to)
        triggerRef.current?.focus()
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    document.addEventListener('keydown', handleEscape)
    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
      document.removeEventListener('keydown', handleEscape)
    }
  }, [isOpen, value])

  const handleApply = () => {
    if (tempFrom && tempTo) {
      // Ensure from is before to
      const from = tempFrom <= tempTo ? tempFrom : tempTo
      const to = tempFrom <= tempTo ? tempTo : tempFrom
      onChange({ from, to })
    }
    setIsOpen(false)
  }

  const formatDate = (date: Date | undefined) => {
    if (!date) return '...'
    return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
  }

  const displayLabel = value
    ? `${formatDate(value.from)} - ${formatDate(value.to)}`
    : 'Custom'

  return (
    <div className={cn('relative', className)}>
      {/* Trigger button */}
      <button
        ref={triggerRef}
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        aria-haspopup="dialog"
        aria-expanded={isOpen}
        className={cn(
          'inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md',
          'bg-white dark:bg-gray-700 border border-gray-200 dark:border-gray-600',
          'text-gray-700 dark:text-gray-200',
          'hover:bg-gray-50 dark:hover:bg-gray-600',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500',
          'transition-colors duration-150'
        )}
      >
        <Calendar className="w-4 h-4 text-gray-500 dark:text-gray-400" />
        {displayLabel}
      </button>

      {/* Popover */}
      {isOpen && (
        <div
          ref={popoverRef}
          role="dialog"
          aria-label="Select custom date range"
          className={cn(
            'absolute top-full right-0 mt-2 z-50',
            'bg-white dark:bg-gray-800 rounded-lg shadow-lg border border-gray-200 dark:border-gray-700',
            'p-4 animate-in fade-in-0 zoom-in-95'
          )}
        >
          <div className="flex flex-col gap-4">
            {/* Date selectors */}
            <div className="flex gap-4">
              {/* Start date */}
              <div className="flex flex-col gap-2">
                <label className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                  Start date
                </label>
                <DayPicker
                  mode="single"
                  selected={tempFrom}
                  onSelect={(date) => setTempFrom(date || undefined)}
                  disabled={{ after: tempTo || new Date() }}
                  showOutsideDays={false}
                  className="date-range-picker-calendar"
                  components={{
                    Chevron: ({ orientation }) =>
                      orientation === 'left' ? (
                        <ChevronLeft className="w-4 h-4" />
                      ) : (
                        <ChevronRight className="w-4 h-4" />
                      ),
                  }}
                />
              </div>

              {/* End date */}
              <div className="flex flex-col gap-2">
                <label className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                  End date
                </label>
                <DayPicker
                  mode="single"
                  selected={tempTo}
                  onSelect={(date) => setTempTo(date || undefined)}
                  disabled={{ before: tempFrom, after: new Date() }}
                  showOutsideDays={false}
                  className="date-range-picker-calendar"
                  components={{
                    Chevron: ({ orientation }) =>
                      orientation === 'left' ? (
                        <ChevronLeft className="w-4 h-4" />
                      ) : (
                        <ChevronRight className="w-4 h-4" />
                      ),
                  }}
                />
              </div>
            </div>

            {/* Apply button */}
            <div className="flex justify-end pt-2 border-t border-gray-100 dark:border-gray-700">
              <button
                type="button"
                onClick={handleApply}
                disabled={!tempFrom || !tempTo}
                className={cn(
                  'px-4 py-2 text-sm font-medium rounded-md',
                  'bg-blue-600 text-white',
                  'hover:bg-blue-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                  'transition-colors duration-150'
                )}
              >
                Apply
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
