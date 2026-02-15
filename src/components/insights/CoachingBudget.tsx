import { useState } from 'react'
import { cn } from '../../lib/utils'
import { useCoachingRules } from '../../hooks/use-coaching-rules'
import { CoachingRulesPanel } from './CoachingRulesPanel'

/**
 * Small pill showing "N/8 rules active" with color states.
 *
 * - Hidden when no rules are applied (zero state)
 * - Amber when 6-7 rules (nearing cap)
 * - Red when at max (8/8)
 * - Clicking opens the CoachingRulesPanel dropdown
 */
export function CoachingBudget() {
  const { count, maxRules, isLoading } = useCoachingRules()
  const [isOpen, setIsOpen] = useState(false)

  if (isLoading) return null

  // Don't show if no rules applied yet
  if (count === 0) return null

  const isWarning = count >= maxRules - 2
  const isFull = count >= maxRules

  return (
    <div className="relative">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className={cn(
          'text-[11px] px-2 py-0.5 rounded-full transition-colors duration-150',
          isFull
            ? 'bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400'
            : isWarning
              ? 'bg-amber-50 dark:bg-amber-900/20 text-amber-600 dark:text-amber-400'
              : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
        )}
      >
        {count}/{maxRules} rules active
      </button>
      {isOpen && (
        <div className="absolute right-0 top-full mt-2 z-50">
          <CoachingRulesPanel onClose={() => setIsOpen(false)} />
        </div>
      )}
    </div>
  )
}
