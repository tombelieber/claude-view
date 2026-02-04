import { ChevronDown } from 'lucide-react'
import { useIsMobile } from '../../hooks/use-media-query'
import { SegmentedControl, type SegmentedControlOption } from './SegmentedControl'
import { cn } from '../../lib/utils'

export interface TimeRangeSelectorProps<T extends string> {
  /** Currently selected value */
  value: T
  /** Callback when selection changes */
  onChange: (value: T) => void
  /** Available options */
  options: SegmentedControlOption<T>[]
  /** Optional className for additional styling */
  className?: string
  /** Aria label for accessibility */
  ariaLabel?: string
}

/**
 * TimeRangeSelector - Responsive time range selection component.
 *
 * Adapts based on viewport:
 * - Desktop (>=640px): Segmented control `[ 7d | 30d | 90d | All | Custom ]`
 * - Mobile (<640px): Dropdown selector `[ 30d ]`
 *
 * Design:
 * - Uses native select for mobile (better UX, no extra dependencies)
 * - Matches SegmentedControl styling for consistency
 * - Minimum 44x44px touch target on mobile (WCAG 2.1 AA)
 *
 * Accessibility:
 * - Proper ARIA labels
 * - Visible focus ring
 * - Touch-friendly target sizes
 */
export function TimeRangeSelector<T extends string>({
  value,
  onChange,
  options,
  className,
  ariaLabel = 'Time range selector',
}: TimeRangeSelectorProps<T>) {
  const isMobile = useIsMobile()

  // Mobile: Render dropdown with styled native select
  if (isMobile) {
    return (
      <div className={cn('relative inline-flex', className)}>
        <select
          value={value}
          onChange={(e) => onChange(e.target.value as T)}
          aria-label={ariaLabel}
          className={cn(
            // Base styles
            'appearance-none cursor-pointer',
            'min-h-[44px] min-w-[44px] px-3 pr-8 py-2',
            // Typography
            'text-sm font-medium',
            // Colors & background
            'bg-gray-100 dark:bg-gray-800',
            'text-gray-900 dark:text-gray-100',
            // Border & shape
            'border border-gray-200 dark:border-gray-700 rounded-lg',
            // Focus state
            'focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-1',
            // Transitions
            'transition-colors duration-150'
          )}
        >
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        {/* Dropdown chevron icon */}
        <ChevronDown
          className="absolute right-2 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500 dark:text-gray-400 pointer-events-none"
          aria-hidden="true"
        />
      </div>
    )
  }

  // Desktop: Render segmented control
  return (
    <SegmentedControl
      value={value}
      onChange={onChange}
      options={options}
      className={className}
      ariaLabel={ariaLabel}
    />
  )
}
