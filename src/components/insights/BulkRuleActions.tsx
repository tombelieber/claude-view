import { useState } from 'react'
import { toast } from 'sonner'
import { cn } from '../../lib/utils'
import { useCoachingRules } from '../../hooks/use-coaching-rules'
import type { GeneratedInsight } from '../../types/generated/GeneratedInsight'

interface BulkRuleActionsProps {
  patterns: GeneratedInsight[]
}

export function BulkRuleActions({ patterns }: BulkRuleActionsProps) {
  const { rules, count, maxRules, applyBulk, removeAll, isApplying, isRemoving } =
    useCoachingRules()
  const [busy, setBusy] = useState(false)

  const hasAppliedRules = count > 0
  const hasApplicable = patterns.some(
    (p) => !rules.some((r) => r.patternId === p.patternId)
  )
  const isLoading = busy || isApplying || isRemoving

  const handleApplyAll = async () => {
    if (isLoading) return
    setBusy(true)
    try {
      const requests = patterns.map((p) => ({
        patternId: p.patternId,
        recommendation: p.recommendation!,
        title: p.title,
        impactScore: p.impactScore,
        sampleSize: p.evidence.sampleSize,
        scope: 'user',
      }))
      await applyBulk(requests)
      toast.success('Rules applied')
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Failed to apply rules')
    } finally {
      setBusy(false)
    }
  }

  const handleRemoveAll = async () => {
    if (isLoading) return
    setBusy(true)
    try {
      await removeAll()
      toast.success('All rules removed')
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Failed to remove rules')
    } finally {
      setBusy(false)
    }
  }

  if (patterns.length === 0) return null

  return (
    <div className="flex items-center gap-1.5">
      {hasApplicable && count < maxRules && (
        <button
          onClick={handleApplyAll}
          disabled={isLoading}
          className={cn(
            'text-[11px] px-2 py-0.5 rounded-md transition-colors duration-150',
            'bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400 hover:bg-blue-100 dark:hover:bg-blue-900/30',
            isLoading && 'opacity-50 cursor-wait'
          )}
        >
          {isLoading && isApplying ? 'Applying...' : 'Apply All'}
        </button>
      )}
      {hasAppliedRules && (
        <button
          onClick={handleRemoveAll}
          disabled={isLoading}
          className={cn(
            'text-[11px] px-2 py-0.5 rounded-md transition-colors duration-150',
            'text-gray-500 dark:text-gray-400 hover:bg-red-50 dark:hover:bg-red-900/20 hover:text-red-600 dark:hover:text-red-400',
            isLoading && 'opacity-50 cursor-wait'
          )}
        >
          {isLoading && isRemoving ? 'Removing...' : 'Remove All'}
        </button>
      )}
    </div>
  )
}
