import type { PlanApproval } from '@claude-view/shared/types/sidecar-protocol'
import { ClipboardCheck, MessageSquarePlus } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { InteractiveCardShell } from '../../../chat/cards/InteractiveCardShell'

export interface PlanApprovalCardProps {
  plan: PlanApproval
  onApprove?: (requestId: string, approved: boolean, feedback?: string) => void
  resolved?: { approved: boolean }
}

function extractPlanContent(planData: Record<string, unknown>): string {
  if (typeof planData.allowedPrompts === 'string') return planData.allowedPrompts
  if (typeof planData.plan === 'string') return planData.plan
  if (typeof planData.content === 'string') return planData.content
  if (typeof planData.message === 'string') return planData.message
  return JSON.stringify(planData, null, 2)
}

export function PlanApprovalCard({ plan, onApprove, resolved }: PlanApprovalCardProps) {
  const [showFeedback, setShowFeedback] = useState(false)
  const [feedback, setFeedback] = useState('')

  const planContent = useMemo(() => extractPlanContent(plan.planData), [plan.planData])
  const requestId = plan.requestId

  const handleApprove = useCallback(() => {
    onApprove?.(requestId, true)
  }, [onApprove, requestId])

  const handleRequestChanges = useCallback(() => {
    if (!showFeedback) {
      setShowFeedback(true)
      return
    }
    onApprove?.(requestId, false, feedback.trim() || undefined)
  }, [showFeedback, onApprove, requestId, feedback])

  const resolvedState = resolved
    ? resolved.approved
      ? { label: 'Approved', variant: 'success' as const }
      : { label: 'Changes Requested', variant: 'denied' as const }
    : undefined

  return (
    <InteractiveCardShell
      variant="plan"
      header="Plan Approval"
      icon={<ClipboardCheck className="w-4 h-4" />}
      resolved={resolvedState}
      actions={
        onApprove ? (
          <>
            <button
              type="button"
              onClick={handleRequestChanges}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
            >
              <MessageSquarePlus className="w-3 h-3" />
              {showFeedback ? 'Submit Changes' : 'Request Changes'}
            </button>
            <button
              type="button"
              onClick={handleApprove}
              className="px-3 py-1.5 text-xs font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 transition-colors"
            >
              Approve Plan
            </button>
          </>
        ) : undefined
      }
    >
      <div className="space-y-2">
        <pre className="text-[11px] text-gray-800 dark:text-gray-200 whitespace-pre-wrap font-mono max-h-48 overflow-y-auto rounded border border-gray-200/50 dark:border-gray-700/50 bg-gray-50/50 dark:bg-gray-800/30 px-2 py-1.5">
          {planContent}
        </pre>

        {showFeedback && (
          <textarea
            value={feedback}
            onChange={(e) => setFeedback(e.target.value)}
            placeholder="Describe what changes you'd like..."
            rows={3}
            className="w-full text-xs px-2 py-1.5 rounded border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-gray-800 dark:text-gray-200 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-blue-500/50 resize-none"
          />
        )}
      </div>
    </InteractiveCardShell>
  )
}
