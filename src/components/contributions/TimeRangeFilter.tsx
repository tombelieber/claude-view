import { ChevronDown } from 'lucide-react'
import { useState, useRef, useEffect } from 'react'
import type { TimeRange } from '../../hooks/use-contributions'
import { cn } from '../../lib/utils'

interface TimeRangeFilterProps {
  value: TimeRange
  onChange: (range: TimeRange) => void
  className?: string
}

const TIME_RANGE_OPTIONS: { value: TimeRange; label: string }[] = [
  { value: 'today', label: 'Today' },
  { value: 'week', label: 'This Week' },
  { value: 'month', label: 'This Month' },
  { value: '90days', label: 'Last 90 Days' },
  { value: 'all', label: 'All Time' },
]

/**
 * TimeRangeFilter dropdown for selecting contribution time range.
 */
export function TimeRangeFilter({ value, onChange, className }: TimeRangeFilterProps) {
  const [isOpen, setIsOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)

  const selectedOption = TIME_RANGE_OPTIONS.find((opt) => opt.value === value)

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false)
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  // Handle keyboard navigation
  const handleKeyDown = (event: React.KeyboardEvent) => {
    switch (event.key) {
      case 'Escape':
        setIsOpen(false)
        buttonRef.current?.focus()
        break
      case 'ArrowDown':
        event.preventDefault()
        if (!isOpen) {
          setIsOpen(true)
        }
        break
      case 'ArrowUp':
        event.preventDefault()
        if (!isOpen) {
          setIsOpen(true)
        }
        break
    }
  }

  const handleSelect = (newValue: TimeRange) => {
    onChange(newValue)
    setIsOpen(false)
    buttonRef.current?.focus()
  }

  return (
    <div ref={dropdownRef} className={cn('relative', className)}>
      <button
        ref={buttonRef}
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        onKeyDown={handleKeyDown}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
        aria-label={`Time range: ${selectedOption?.label}`}
        className={cn(
          'flex items-center gap-2 px-3 py-1.5 text-sm font-medium',
          'bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700',
          'rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400',
          'transition-colors cursor-pointer'
        )}
      >
        <span>{selectedOption?.label}</span>
        <ChevronDown
          className={cn(
            'w-4 h-4 text-gray-400 transition-transform',
            isOpen && 'rotate-180'
          )}
          aria-hidden="true"
        />
      </button>

      {isOpen && (
        <div
          role="listbox"
          aria-label="Select time range"
          className={cn(
            'absolute right-0 mt-1 w-40 py-1 z-10',
            'bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700',
            'rounded-lg shadow-lg'
          )}
        >
          {TIME_RANGE_OPTIONS.map((option) => (
            <button
              key={option.value}
              role="option"
              aria-selected={option.value === value}
              onClick={() => handleSelect(option.value)}
              className={cn(
                'w-full px-3 py-2 text-sm text-left',
                'hover:bg-gray-100 dark:hover:bg-gray-700',
                'focus-visible:outline-none focus-visible:bg-gray-100 dark:focus-visible:bg-gray-700',
                'transition-colors cursor-pointer',
                option.value === value
                  ? 'text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-900/20'
                  : 'text-gray-700 dark:text-gray-300'
              )}
            >
              {option.label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
