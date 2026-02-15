import { useEffect, useRef } from 'react'
import { toast } from 'sonner'
import { cn } from '../../lib/utils'
import { useCoachingRules } from '../../hooks/use-coaching-rules'

interface CoachingRulesPanelProps {
  onClose: () => void
}

/**
 * Dropdown panel showing all active coaching rules with remove buttons.
 *
 * Appears below the CoachingBudget pill. Closes on outside click or Escape.
 */
export function CoachingRulesPanel({ onClose }: CoachingRulesPanelProps) {
  const { rules, count, maxRules, removeRule, isRemoving } = useCoachingRules()
  const panelRef = useRef<HTMLDivElement>(null)

  // Close on outside click
  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (panelRef.current && !panelRef.current.contains(event.target as Node)) {
        onClose()
      }
    }

    function handleEscape(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        onClose()
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    document.addEventListener('keydown', handleEscape)
    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
      document.removeEventListener('keydown', handleEscape)
    }
  }, [onClose])

  const handleRemove = async (id: string) => {
    try {
      await removeRule(id)
      toast.success('Rule removed')
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Failed to remove rule')
    }
  }

  return (
    <div
      ref={panelRef}
      className="w-80 bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 shadow-lg overflow-hidden"
    >
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-100 dark:border-gray-800">
        <h3 className="text-sm font-medium text-gray-900 dark:text-gray-100">
          Active Coaching Rules ({count}/{maxRules})
        </h3>
        <button
          onClick={onClose}
          className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 text-sm"
        >
          {'\u2715'}
        </button>
      </div>

      {/* Rules list */}
      <div className="max-h-64 overflow-y-auto">
        {rules.length === 0 ? (
          <p className="px-4 py-6 text-xs text-gray-400 dark:text-gray-500 text-center">
            No coaching rules applied yet.
          </p>
        ) : (
          <ul className="divide-y divide-gray-100 dark:divide-gray-800">
            {rules.map((rule) => (
              <li key={rule.id} className="px-4 py-3">
                <div className="flex items-start justify-between gap-2">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-1.5">
                      <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500">
                        {rule.id}
                      </span>
                      <span className="text-xs font-medium text-gray-900 dark:text-gray-100 truncate">
                        {rule.title}
                      </span>
                    </div>
                    <p className="text-[11px] text-gray-500 dark:text-gray-400 mt-0.5 line-clamp-2">
                      {rule.body}
                    </p>
                    {rule.appliedAt && (
                      <span className="text-[10px] text-gray-400 dark:text-gray-500 mt-1 block">
                        Applied {rule.appliedAt}
                      </span>
                    )}
                  </div>
                  <button
                    onClick={() => handleRemove(rule.id)}
                    disabled={isRemoving}
                    className={cn(
                      'text-[10px] px-1.5 py-0.5 rounded text-red-500 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors shrink-0',
                      isRemoving && 'opacity-50 cursor-wait'
                    )}
                  >
                    Remove
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Footer */}
      <div className="px-4 py-2 border-t border-gray-100 dark:border-gray-800 bg-gray-50 dark:bg-gray-800/50">
        <p className="text-[10px] text-gray-400 dark:text-gray-500">
          Claude reads these rules at session start. Fewer, focused rules work better than many.
        </p>
      </div>
    </div>
  )
}
