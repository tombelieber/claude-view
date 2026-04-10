import type { PlanApproval } from '../../../../types/sidecar-protocol'
import { ChevronDown, ClipboardCheck, FileText, Shield, ShieldCheck } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Markdown } from './Markdown'
import { InteractiveCardShell } from './InteractiveCardShell'

export interface PlanApprovalCardProps {
  plan: PlanApproval
  onApprove?: (
    requestId: string,
    approved: boolean,
    feedback?: string,
    bypassPermissions?: boolean,
  ) => void
  resolved?: { approved: boolean }
  /** CLI terminal delegation — when present, sends keystrokes instead of SDK approval. */
  onTerminalDelegate?: (keys: string[]) => Promise<void>
}

interface AllowedPrompt {
  prompt: string
  tool: string
}

function extractPlanContent(planData: unknown): string {
  if (!planData || typeof planData !== 'object') {
    return typeof planData === 'string' ? planData : String(planData ?? '')
  }
  const d = planData as Record<string, unknown>
  if (typeof d.plan === 'string') return d.plan
  if (typeof d.planContent === 'string') return d.planContent
  if (typeof d.allowedPrompts === 'string') return d.allowedPrompts
  if (typeof d.content === 'string') return d.content
  if (typeof d.message === 'string') return d.message
  return JSON.stringify(planData, null, 2)
}

function extractAllowedPrompts(planData: unknown): AllowedPrompt[] {
  if (!planData || typeof planData !== 'object') return []
  const d = planData as Record<string, unknown>
  if (!Array.isArray(d.allowedPrompts)) return []
  return d.allowedPrompts.filter(
    (p): p is AllowedPrompt =>
      p != null &&
      typeof p === 'object' &&
      typeof p.prompt === 'string' &&
      typeof p.tool === 'string',
  )
}

function extractPlanFilePath(planData: unknown): string | undefined {
  if (!planData || typeof planData !== 'object') return undefined
  const d = planData as Record<string, unknown>
  return typeof d.planFilePath === 'string' ? d.planFilePath : undefined
}

export function PlanApprovalCard({
  plan,
  onApprove,
  resolved,
  onTerminalDelegate,
}: PlanApprovalCardProps) {
  const [showFeedback, setShowFeedback] = useState(false)
  const [feedback, setFeedback] = useState('')
  const [expanded, setExpanded] = useState(false)
  const feedbackRef = useRef<HTMLTextAreaElement | null>(null)

  // Sidecar shape: plan.planData holds { plan, allowedPrompts, ... }
  // Historical Rust shape: plan IS { planContent, approved, ... } (no planData wrapper)
  const effectiveData = plan?.planData ?? plan
  const planContent = useMemo(() => extractPlanContent(effectiveData), [effectiveData])
  const allowedPrompts = useMemo(() => extractAllowedPrompts(effectiveData), [effectiveData])
  const planFilePath = useMemo(() => extractPlanFilePath(effectiveData), [effectiveData])
  const requestId = plan?.requestId ?? ''

  useEffect(() => {
    if (!showFeedback) return
    feedbackRef.current?.focus()
  }, [showFeedback])

  const handleBypass = useCallback(() => {
    if (onTerminalDelegate) {
      onTerminalDelegate(['Enter'])
      return
    }
    onApprove?.(requestId, true, undefined, true)
  }, [onApprove, onTerminalDelegate, requestId])

  const handleManualApprove = useCallback(() => {
    if (onTerminalDelegate) {
      onTerminalDelegate(['ArrowDown', 'Enter'])
      return
    }
    onApprove?.(requestId, true, undefined, false)
  }, [onApprove, onTerminalDelegate, requestId])

  const handleRequestChanges = useCallback(() => {
    if (onTerminalDelegate) {
      onTerminalDelegate(['ArrowDown', 'ArrowDown', 'Enter'])
      return
    }
    if (!showFeedback) {
      setShowFeedback(true)
      return
    }
    onApprove?.(requestId, false, feedback.trim() || undefined)
  }, [showFeedback, onApprove, onTerminalDelegate, requestId, feedback])

  const resolvedState = resolved
    ? resolved.approved
      ? { label: 'Approved', variant: 'success' as const }
      : { label: 'Changes Requested', variant: 'denied' as const }
    : undefined

  // Filename from plan file path for display
  const planFileName = planFilePath?.split('/').pop()

  return (
    <InteractiveCardShell
      variant="plan"
      header="Plan Approval"
      icon={<ClipboardCheck className="w-4 h-4" />}
      resolved={resolvedState}
    >
      <div className="space-y-2.5">
        {/* ── Plan content — preview height with "Show all" ── */}
        <div className="rounded border border-gray-200/60 dark:border-gray-700/50 overflow-hidden">
          {planFileName && (
            <div className="flex items-center gap-2 px-2.5 py-1.5 border-b border-gray-100 dark:border-gray-800 bg-gray-50/50 dark:bg-gray-900/30">
              <FileText className="w-3 h-3 text-gray-400 shrink-0" />
              <span className="text-xs font-mono text-gray-500 dark:text-gray-400 truncate">
                {planFileName}
              </span>
            </div>
          )}
          <div
            className={`px-3 py-2 bg-gray-50/30 dark:bg-gray-900/20 overflow-hidden relative ${
              expanded ? 'max-h-none' : 'max-h-40'
            }`}
          >
            <Markdown content={planContent} />
            {!expanded && (
              <div className="absolute bottom-0 left-0 right-0 h-10 bg-gradient-to-t from-gray-50 dark:from-gray-900/80 to-transparent pointer-events-none" />
            )}
          </div>
          {!expanded && (
            <button
              type="button"
              onClick={() => setExpanded(true)}
              className="flex items-center justify-center gap-1 w-full px-2.5 py-1 text-xs text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/20 transition-colors border-t border-gray-100 dark:border-gray-800"
            >
              <ChevronDown className="w-3 h-3" />
              Show all
            </button>
          )}
        </div>

        {/* ── Allowed prompts ── */}
        {allowedPrompts.length > 0 && (
          <div className="rounded border border-blue-200/50 dark:border-blue-700/30 bg-blue-50/30 dark:bg-blue-900/10 px-2.5 py-1.5">
            <div className="text-xs font-medium text-blue-700 dark:text-blue-300 mb-1">
              Auto-approved if bypassing permissions:
            </div>
            <ul className="space-y-0.5">
              {allowedPrompts.map((p, i) => (
                <li
                  key={i}
                  className="text-xs text-gray-600 dark:text-gray-400 flex items-start gap-1.5"
                >
                  <span className="text-blue-400 dark:text-blue-500 shrink-0 mt-0.5">•</span>
                  <span className="font-mono flex-1 min-w-0">{p.prompt}</span>
                  <span className="text-gray-400 dark:text-gray-500 shrink-0">({p.tool})</span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {/* ── Vertical option list (matching CLI menu) ── */}
        {onApprove && (
          <div className="space-y-1">
            <OptionButton
              icon={<ShieldCheck className="w-3.5 h-3.5" />}
              label="Yes, and bypass permissions"
              description={
                allowedPrompts.length > 0
                  ? `Auto-approve ${allowedPrompts.length} listed actions`
                  : undefined
              }
              variant="primary"
              onClick={handleBypass}
            />
            <OptionButton
              icon={<Shield className="w-3.5 h-3.5" />}
              label="Yes, manually approve edits"
              description="Approve plan but confirm each tool individually"
              variant="secondary"
              onClick={handleManualApprove}
            />
            <OptionButton
              icon={<FileText className="w-3.5 h-3.5" />}
              label={showFeedback ? 'Submit changes' : 'Tell Claude what to change'}
              variant="ghost"
              onClick={handleRequestChanges}
            />
          </div>
        )}

        {/* ── Feedback textarea ── */}
        {showFeedback && (
          <textarea
            ref={feedbackRef}
            value={feedback}
            onChange={(e) => setFeedback(e.target.value)}
            placeholder="Describe what changes you'd like..."
            rows={3}
            className="w-full text-xs px-2.5 py-1.5 rounded border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-gray-800 dark:text-gray-200 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-blue-500/50 resize-none"
          />
        )}
      </div>
    </InteractiveCardShell>
  )
}

// ── Vertical option button ──────────────────────────────────────────

function OptionButton({
  icon,
  label,
  description,
  variant,
  onClick,
}: {
  icon: React.ReactNode
  label: string
  description?: string
  variant: 'primary' | 'secondary' | 'ghost'
  onClick: () => void
}) {
  const base =
    'flex items-center gap-2.5 w-full px-3 py-2 rounded-md text-left transition-colors cursor-pointer'
  const styles = {
    primary:
      'bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-700/50 hover:bg-blue-100 dark:hover:bg-blue-900/40 text-blue-800 dark:text-blue-200',
    secondary:
      'bg-gray-50 dark:bg-gray-800/50 border border-gray-200 dark:border-gray-700/50 hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-700 dark:text-gray-300',
    ghost:
      'border border-transparent hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-600 dark:text-gray-400',
  }

  return (
    <button type="button" onClick={onClick} className={`${base} ${styles[variant]}`}>
      <span className="shrink-0">{icon}</span>
      <div className="flex-1 min-w-0">
        <div className="text-xs font-medium">{label}</div>
        {description && (
          <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{description}</div>
        )}
      </div>
    </button>
  )
}
