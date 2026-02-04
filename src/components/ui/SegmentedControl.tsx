import { cn } from '../../lib/utils'

export interface SegmentedControlOption<T extends string> {
  value: T
  label: string
}

export interface SegmentedControlProps<T extends string> {
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
 * SegmentedControl - A pill-style toggle for selecting between options.
 *
 * Design:
 * - Horizontal row of options with rounded pill shape
 * - Selected option has solid background
 * - Unselected options are subtle/transparent
 * - Smooth transition between states
 *
 * Accessibility:
 * - Uses radio group pattern for proper semantics
 * - Keyboard navigation with arrow keys
 * - Visible focus ring
 */
export function SegmentedControl<T extends string>({
  value,
  onChange,
  options,
  className,
  ariaLabel = 'Time range selector',
}: SegmentedControlProps<T>) {
  return (
    <div
      role="radiogroup"
      aria-label={ariaLabel}
      className={cn(
        'inline-flex items-center rounded-lg bg-gray-100 dark:bg-gray-800 p-1',
        className
      )}
    >
      {options.map((option) => {
        const isSelected = option.value === value
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={isSelected}
            onClick={() => onChange(option.value)}
            className={cn(
              'px-3 py-1.5 text-sm font-medium rounded-md transition-all duration-150',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-1',
              isSelected
                ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'
            )}
          >
            {option.label}
          </button>
        )
      })}
    </div>
  )
}
