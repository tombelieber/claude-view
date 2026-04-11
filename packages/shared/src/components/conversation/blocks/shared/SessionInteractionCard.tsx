import { useCallback } from 'react'
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

export interface FullInteractionBlock {
  id: string
  variant: 'permission' | 'question' | 'plan' | 'elicitation'
  requestId: string | null
  resolved: boolean
  historicalSource: string | null
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

  return (
    <FullCard
      fullInteraction={fullInteraction}
      respond={respond}
    />
  )
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

  // ── Permission ────────────────────────────────────────────
  const handlePermissionRespond = useCallback(
    (requestId: string, allowed: boolean) => {
      respond?.({ variant: 'permission', requestId, allowed })
    },
    [respond],
  )

  const handlePermissionAlwaysAllow = useCallback(
    (requestId: string, allowed: boolean, updatedPermissions: unknown[]) => {
      respond?.({ variant: 'permission', requestId, allowed, updatedPermissions })
    },
    [respond],
  )

  // ── Question ──────────────────────────────────────────────
  const handleQuestionAnswer = useCallback(
    (requestId: string, answers: Record<string, string>) => {
      respond?.({ variant: 'question', requestId, answers })
    },
    [respond],
  )

  // ── Plan ──────────────────────────────────────────────────
  const handlePlanApprove = useCallback(
    (requestId: string, approved: boolean, feedback?: string, bypassPermissions?: boolean) => {
      respond?.({ variant: 'plan', requestId, approved, feedback, bypassPermissions })
    },
    [respond],
  )

  // ── Elicitation ───────────────────────────────────────────
  const handleElicitationSubmit = useCallback(
    (requestId: string, response: string) => {
      respond?.({ variant: 'elicitation', requestId, response })
    },
    [respond],
  )

  switch (variant) {
    case 'permission': {
      const permission = normalizePermissionRequest(data)
      if (!permission) return <InteractionError variant="permission" />
      return (
        <PermissionCard
          permission={permission}
          onRespond={respond ? handlePermissionRespond : undefined}
          onAlwaysAllow={
            respond && permission.suggestions?.length
              ? handlePermissionAlwaysAllow
              : undefined
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
      return (
        <PlanApprovalCard
          plan={plan}
          onApprove={respond ? handlePlanApprove : undefined}
        />
      )
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
}
