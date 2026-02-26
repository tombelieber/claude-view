import { toast } from 'sonner'
import { cn } from '../../lib/utils'
import { useCoachingRules } from '../../hooks/use-coaching-rules'

interface ApplyRuleButtonProps {
  patternId: string
  recommendation: string
  title: string
  impactScore: number
  sampleSize: number
}

/**
 * Button to apply or remove a coaching rule from a pattern insight.
 *
 * States: idle -> loading -> applied (with remove-on-click), disabled at budget cap.
 * When applied, shows green "Applied" that changes to red "Remove" on hover.
 */
export function ApplyRuleButton({
  patternId,
  recommendation,
  title,
  impactScore,
  sampleSize,
}: ApplyRuleButtonProps) {
  const { rules, count, maxRules, applyRule, removeRule, isApplying, isRemoving } =
    useCoachingRules()

  const existingRule = rules.find((r) => r.patternId === patternId)
  const isApplied = !!existingRule
  const isBudgetFull = count >= maxRules && !isApplied
  const isLoading = isApplying || isRemoving

  const handleClick = async () => {
    if (isLoading) return

    try {
      if (isApplied && existingRule) {
        await removeRule(existingRule.id)
        toast.success('Rule removed')
      } else {
        await applyRule({
          patternId,
          recommendation,
          title,
          impactScore,
          sampleSize,
          scope: 'user',
        })
        toast.success('Rule applied — Claude will follow this in future sessions')
      }
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Something went wrong')
    }
  }

  if (isBudgetFull) {
    return (
      <button
        disabled
        className="text-[11px] px-2.5 py-1 rounded-md bg-gray-100 dark:bg-gray-800 text-gray-400 dark:text-gray-500 cursor-not-allowed"
        title={`Remove a rule first — maximum ${maxRules} coaching rules`}
      >
        Apply Rule
      </button>
    )
  }

  return (
    <button
      onClick={handleClick}
      disabled={isLoading}
      className={cn(
        'text-[11px] px-2.5 py-1 rounded-md transition-colors duration-150',
        isApplied
          ? 'bg-green-50 dark:bg-green-900/20 text-green-600 dark:text-green-400 hover:bg-red-50 dark:hover:bg-red-900/20 hover:text-red-600 dark:hover:text-red-400'
          : 'bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 hover:bg-blue-100 dark:hover:bg-blue-900/30',
        isLoading && 'opacity-50 cursor-wait'
      )}
    >
      {isLoading ? (
        <span className="flex items-center gap-1">
          <svg className="animate-spin h-3 w-3" viewBox="0 0 24 24" fill="none">
            <circle
              className="opacity-25"
              cx="12"
              cy="12"
              r="10"
              stroke="currentColor"
              strokeWidth="4"
            />
            <path
              className="opacity-75"
              fill="currentColor"
              d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
            />
          </svg>
          {isApplied ? 'Removing...' : 'Applying...'}
        </span>
      ) : isApplied ? (
        'Applied \u2713'
      ) : (
        'Apply Rule'
      )}
    </button>
  )
}
