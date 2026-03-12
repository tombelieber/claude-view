import { ClipboardCheck, MessageSquarePlus } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { InteractiveCardShell } from './InteractiveCardShell'

export interface PlanApprovalCardProps {
  requestId: string
  planData: unknown
  onApprove: (requestId: string, approved: boolean, feedback?: string) => void
  resolved?: { approved: boolean }
  isPending?: boolean
}

function extractPlanContent(planData: unknown): string {
  if (!planData || typeof planData !== 'object') {
    return typeof planData === 'string' ? planData : JSON.stringify(planData, null, 2)
  }
  const d = planData as Record<string, unknown>
  // ExitPlanMode tool_use may carry plan text in different fields
  if (typeof d.allowedPrompts === 'string') return d.allowedPrompts
  if (typeof d.plan === 'string') return d.plan
  if (typeof d.content === 'string') return d.content
  if (typeof d.message === 'string') return d.message
  return JSON.stringify(planData, null, 2)
}

export function PlanApprovalCard({
  requestId,
  planData,
  onApprove,
  resolved,
  isPending,
}: PlanApprovalCardProps) {
  const [showFeedback, setShowFeedback] = useState(false)
  const [feedback, setFeedback] = useState('')

  const planContent = useMemo(() => extractPlanContent(planData), [planData])

  const handleApprove = useCallback(() => {
    onApprove(requestId, true)
  }, [onApprove, requestId])

  const handleRequestChanges = useCallback(() => {
    if (!showFeedback) {
      setShowFeedback(true)
      return
    }
    onApprove(requestId, false, feedback.trim() || undefined)
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
        <>
          <button
            type="button"
            onClick={handleRequestChanges}
            disabled={isPending}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors disabled:opacity-50 disabled:cursor-wait"
          >
            <MessageSquarePlus className="w-3 h-3" />
            {showFeedback ? 'Submit Changes' : 'Request Changes'}
          </button>
          <button
            type="button"
            onClick={handleApprove}
            disabled={isPending}
            className="px-3 py-1.5 text-xs font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-wait"
          >
            {isPending ? 'Approving\u2026' : 'Approve Plan'}
          </button>
        </>
      }
    >
      <div className="space-y-2">
        <pre className="text-[11px] text-gray-800 dark:text-gray-200 whitespace-pre-wrap font-mono max-h-48 overflow-y-auto rounded border border-gray-200/50 dark:border-gray-700/50 bg-gray-50/50 dark:bg-gray-800/30 px-2 py-1.5">
          {planContent}
        </pre>

        {/* Feedback textarea — shown when "Request Changes" is clicked the first time */}
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
