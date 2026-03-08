import type { PlanDocument } from '../../types/generated/PlanDocument'
import { PlanFileCard } from './PlanFileCard'

interface PlanTabProps {
  plans: PlanDocument[]
}

export function PlanTab({ plans }: PlanTabProps) {
  const isSingle = plans.length === 1

  return (
    <div className="p-4 overflow-y-auto h-full space-y-2">
      {/* Summary header — only for multiple plans */}
      {!isSingle && (
        <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
          <span className="font-medium text-gray-700 dark:text-gray-300">
            {plans.length} plan file{plans.length !== 1 ? 's' : ''}
          </span>
        </div>
      )}

      {/* Plan cards */}
      {plans.map((plan, idx) => (
        <PlanFileCard key={plan.filename} plan={plan} defaultExpanded={isSingle || idx === 0} />
      ))}
    </div>
  )
}
