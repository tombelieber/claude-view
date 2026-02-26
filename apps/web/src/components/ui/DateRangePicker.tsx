import { useState, useRef, useEffect } from 'react'
import { DayPicker, type DateRange } from 'react-day-picker'
import * as Popover from '@radix-ui/react-popover'
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

/** Preset quick-select options */
const PRESETS = [
  { label: 'Last 7 days', days: 7 },
  { label: 'Last 14 days', days: 14 },
  { label: 'Last 30 days', days: 30 },
  { label: 'Last 90 days', days: 90 },
] as const

function daysAgo(n: number): Date {
  const d = new Date()
  d.setHours(0, 0, 0, 0)
  d.setDate(d.getDate() - n + 1)
  return d
}

function today(): Date {
  const d = new Date()
  d.setHours(23, 59, 59, 999)
  return d
}

const formatDate = (date: Date | undefined) => {
  if (!date) return '...'
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

/**
 * DateRangePicker - A polished popover with unified range calendar and preset buttons.
 *
 * Design:
 * - Trigger button shows current date range or "Custom"
 * - Radix Popover with portal for proper layering
 * - Side panel with quick-select presets (Last 7d, 14d, 30d, 90d)
 * - Two-month DayPicker in range mode with visual range highlighting
 * - Apply button commits the selection
 * - Click outside or Escape closes without applying
 */
export function DateRangePicker({
  value,
  onChange,
  className,
}: DateRangePickerProps) {
  const [isOpen, setIsOpen] = useState(false)
  const [tempRange, setTempRange] = useState<DateRange | undefined>(
    value ? { from: value.from, to: value.to } : undefined
  )

  // Only reset draft state when popover opens (prevIsOpenRef pattern)
  const prevIsOpenRef = useRef(false)
  useEffect(() => {
    if (isOpen && !prevIsOpenRef.current) {
      setTempRange(value ? { from: value.from, to: value.to } : undefined)
    }
    prevIsOpenRef.current = isOpen
  }, [isOpen, value])

  const handleApply = () => {
    if (tempRange?.from && tempRange?.to) {
      const from = tempRange.from <= tempRange.to ? tempRange.from : tempRange.to
      const to = tempRange.from <= tempRange.to ? tempRange.to : tempRange.from
      onChange({ from, to })
    }
    setIsOpen(false)
  }

  const handlePreset = (days: number) => {
    const from = daysAgo(days)
    const to = today()
    setTempRange({ from, to })
  }

  const displayLabel = value
    ? `${formatDate(value.from)} – ${formatDate(value.to)}`
    : 'Custom'

  const canApply = !!(tempRange?.from && tempRange?.to)

  return (
    <Popover.Root open={isOpen} onOpenChange={setIsOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          aria-haspopup="dialog"
          className={cn(
            'inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md cursor-pointer',
            'bg-white dark:bg-gray-700 border border-gray-200 dark:border-gray-600',
            'text-gray-700 dark:text-gray-200',
            'hover:bg-gray-50 dark:hover:bg-gray-600',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500',
            'transition-colors duration-150',
            className
          )}
        >
          <Calendar className="w-4 h-4 text-gray-500 dark:text-gray-400" />
          {displayLabel}
        </button>
      </Popover.Trigger>

      <Popover.Portal>
        <Popover.Content
          role="dialog"
          aria-label="Select custom date range"
          align="end"
          sideOffset={8}
          className={cn(
            'z-50 rounded-xl shadow-xl border',
            'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700',
            'animate-in fade-in-0 zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95'
          )}
        >
          <div className="flex">
            {/* Presets sidebar */}
            <div className="flex flex-col gap-1 p-3 border-r border-gray-100 dark:border-gray-700 min-w-[140px]">
              <span className="text-[11px] font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider px-2 mb-1">
                Quick select
              </span>
              {PRESETS.map((preset) => (
                <button
                  key={preset.days}
                  type="button"
                  onClick={() => handlePreset(preset.days)}
                  className={cn(
                    'text-left px-2 py-1.5 text-sm rounded-md cursor-pointer',
                    'text-gray-600 dark:text-gray-300',
                    'hover:bg-gray-100 dark:hover:bg-gray-700',
                    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500',
                    'transition-colors duration-100'
                  )}
                >
                  {preset.label}
                </button>
              ))}
            </div>

            {/* Calendar + footer */}
            <div className="flex flex-col">
              {/* Range calendar */}
              <div className="p-3 date-range-picker">
                <DayPicker
                  mode="range"
                  selected={tempRange}
                  onSelect={setTempRange}
                  numberOfMonths={2}
                  disabled={{ after: new Date() }}
                  showOutsideDays={false}
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

              {/* Footer: selection summary + actions */}
              <div className="flex items-center justify-between px-4 py-3 border-t border-gray-100 dark:border-gray-700">
                <span className="text-sm text-gray-500 dark:text-gray-400">
                  {tempRange?.from && tempRange?.to
                    ? `${formatDate(tempRange.from)} – ${formatDate(tempRange.to)}`
                    : tempRange?.from
                      ? `${formatDate(tempRange.from)} – ...`
                      : 'Select a range'}
                </span>
                <div className="flex items-center gap-2">
                  <Popover.Close asChild>
                    <button
                      type="button"
                      className={cn(
                        'px-3 py-1.5 text-sm font-medium rounded-md cursor-pointer',
                        'text-gray-600 dark:text-gray-300',
                        'hover:bg-gray-100 dark:hover:bg-gray-700',
                        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500',
                        'transition-colors duration-150'
                      )}
                    >
                      Cancel
                    </button>
                  </Popover.Close>
                  <button
                    type="button"
                    onClick={handleApply}
                    disabled={!canApply}
                    className={cn(
                      'px-4 py-1.5 text-sm font-medium rounded-md cursor-pointer',
                      'bg-blue-600 text-white',
                      'hover:bg-blue-700',
                      'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2',
                      'disabled:opacity-50 disabled:cursor-not-allowed',
                      'transition-colors duration-150'
                    )}
                  >
                    Apply
                  </button>
                </div>
              </div>
            </div>
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
