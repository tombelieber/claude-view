import { useCallback, useState } from 'react'
import {
  normalizePermissionRequest,
  normalizeAskQuestion,
  normalizePlanApproval,
  normalizeElicitation,
} from '../../../../lib/interaction-normalizers'
import { PermissionCard } from './PermissionCard'
import { AskUserQuestionCard } from './AskUserQuestionCard'
import { PlanApprovalCard } from './PlanApprovalCard'
import { ElicitationCard } from './ElicitationCard'
import { CompactInteractionPreview } from './CompactInteractionPreview'
import type { PendingInteractionMeta } from './CompactInteractionPreview'
import { InteractionError } from './InteractionError'

// Re-export so consumers can import types from this file
export type { PendingInteractionMeta } from './CompactInteractionPreview'

// ── Types ──────────────────────────────────────────────────────────

// Local mirror of the ts-rs-generated `InteractionBlock` contract. `shared`
// cannot import `apps/web/src/types/generated/InteractionBlock` (it escapes
// shared's tsconfig rootDir — TS6059), so the shape is declared here with the
// SAME optionality as the generated contract: `requestId`/`historicalSource`
// are OPTIONAL (the previous fork wrongly marked them required), and
// `historicalSource` is the `HistoricalSource` union (not bare `string`).
export interface FullInteractionBlock {
  id: string
  variant: 'permission' | 'question' | 'plan' | 'elicitation'
  requestId?: string | null
  resolved: boolean
  historicalSource?: 'system_variant' | 'inferred_from_tool_pattern' | null
  data: unknown
}

export type InteractRequest =
  | { variant: 'permission'; requestId: string; allowed: boolean; updatedPermissions?: unknown[] }
  | { variant: 'question'; requestId: string; answers: Record<string, string> }
  | {
      variant: 'plan'
      requestId: string
      approved: boolean
      feedback?: string
      bypassPermissions?: boolean
    }
  | { variant: 'elicitation'; requestId: string; response: string }

export type InteractResult = { ok: true } | { ok: false; status: number; reason: string }

export interface SessionInteractionCardProps {
  sessionId: string
  meta: PendingInteractionMeta
  fullInteraction: FullInteractionBlock | null
  respond?: (request: InteractRequest) => Promise<InteractResult>
}

// ── Component ──────────────────────────────────────────────────────

export function SessionInteractionCard({
  meta,
  fullInteraction,
  respond,
}: SessionInteractionCardProps) {
  // While full data is loading, show compact preview
  if (!fullInteraction) {
    return <CompactInteractionPreview meta={meta} />
  }

  return <FullCard fullInteraction={fullInteraction} respond={respond} />
}

// ── Inner card renderer (avoids conditional hooks in parent) ──────

function FullCard({
  fullInteraction,
  respond,
}: {
  fullInteraction: FullInteractionBlock
  respond?: (request: InteractRequest) => Promise<InteractResult>
}) {
  const { variant, data } = fullInteraction

  // Trust-over-accuracy: await the delivery ack. The card does NOT optimistically
  // resolve — the server clears the pending interaction (and unmounts this card)
  // via SSE only once the agent actually consumed the decision. If delivery is
  // not confirmed (4xx/5xx/offline), keep the card and surface a retry hint so a
  // failed send is never shown as success. 寧願唔顯示，都唔顯示錯嘅嘢.
  const [deliveryError, setDeliveryError] = useState<string | null>(null)

  const runRespond = useCallback(
    async (request: InteractRequest) => {
      if (!respond) return
      setDeliveryError(null)
      const result = await respond(request)
      if (!result.ok) {
        const detail =
          result.status === 0 ? 'no connection' : `the agent didn't confirm (${result.status})`
        setDeliveryError(`Couldn't deliver your response — ${detail}. Tap again to retry.`)
      }
    },
    [respond],
  )

  // ── Permission ────────────────────────────────────────────
  const handlePermissionRespond = useCallback(
    (requestId: string, allowed: boolean) => {
      void runRespond({ variant: 'permission', requestId, allowed })
    },
    [runRespond],
  )

  const handlePermissionAlwaysAllow = useCallback(
    (requestId: string, allowed: boolean, updatedPermissions: unknown[]) => {
      void runRespond({ variant: 'permission', requestId, allowed, updatedPermissions })
    },
    [runRespond],
  )

  // ── Question ──────────────────────────────────────────────
  const handleQuestionAnswer = useCallback(
    (requestId: string, answers: Record<string, string>) => {
      void runRespond({ variant: 'question', requestId, answers })
    },
    [runRespond],
  )

  // ── Plan ──────────────────────────────────────────────────
  const handlePlanApprove = useCallback(
    (requestId: string, approved: boolean, feedback?: string, bypassPermissions?: boolean) => {
      void runRespond({ variant: 'plan', requestId, approved, feedback, bypassPermissions })
    },
    [runRespond],
  )

  // ── Elicitation ───────────────────────────────────────────
  const handleElicitationSubmit = useCallback(
    (requestId: string, response: string) => {
      void runRespond({ variant: 'elicitation', requestId, response })
    },
    [runRespond],
  )

  const card = (() => {
    switch (variant) {
      case 'permission': {
        const permission = normalizePermissionRequest(data)
        if (!permission) return <InteractionError variant="permission" />
        return (
          <PermissionCard
            permission={permission}
            onRespond={respond ? handlePermissionRespond : undefined}
            onAlwaysAllow={
              respond && permission.suggestions?.length ? handlePermissionAlwaysAllow : undefined
            }
          />
        )
      }

      case 'question': {
        const question = normalizeAskQuestion(data)
        if (!question) return <InteractionError variant="question" />
        return (
          <AskUserQuestionCard
            question={question}
            onAnswer={respond ? handleQuestionAnswer : undefined}
          />
        )
      }

      case 'plan': {
        const plan = normalizePlanApproval(data)
        if (!plan) return <InteractionError variant="plan" />
        return <PlanApprovalCard plan={plan} onApprove={respond ? handlePlanApprove : undefined} />
      }

      case 'elicitation': {
        const elicitation = normalizeElicitation(data)
        if (!elicitation) return <InteractionError variant="elicitation" />
        return (
          <ElicitationCard
            elicitation={elicitation}
            onSubmit={respond ? handleElicitationSubmit : undefined}
          />
        )
      }

      default:
        return <InteractionError variant={variant} />
    }
  })()

  if (!deliveryError) return card

  return (
    <div>
      <div
        role="alert"
        className="mb-1.5 flex items-center gap-2 rounded-md border border-red-200 dark:border-red-800/40 bg-red-50 dark:bg-red-900/20 px-2.5 py-1.5 text-xs text-red-700 dark:text-red-400"
      >
        {deliveryError}
      </div>
      {card}
    </div>
  )
}
