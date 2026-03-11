import type { InteractionBlock as InteractionBlockType } from '@claude-view/shared/types/blocks'
import type {
  AskQuestion,
  Elicitation,
  PermissionRequest,
  PlanApproval,
} from '@claude-view/shared/types/sidecar-protocol'
import { AskUserQuestionCard } from '../shared/AskUserQuestionCard'
import { ElicitationCard } from '../shared/ElicitationCard'
import { PermissionCard } from '../shared/PermissionCard'
import { PlanApprovalCard } from '../shared/PlanApprovalCard'

interface InteractionBlockProps {
  block: InteractionBlockType
  onPermissionRespond?: (requestId: string, allowed: boolean) => void
  onQuestionAnswer?: (requestId: string, answers: Record<string, string>) => void
  onPlanApprove?: (requestId: string, approved: boolean, feedback?: string) => void
  onElicitationSubmit?: (requestId: string, response: string) => void
}

export function DevInteractionBlock({
  block,
  onPermissionRespond,
  onQuestionAnswer,
  onPlanApprove,
  onElicitationSubmit,
}: InteractionBlockProps) {
  switch (block.variant) {
    case 'permission':
      return (
        <PermissionCard
          permission={block.data as PermissionRequest}
          onRespond={onPermissionRespond}
          resolved={block.resolved ? { allowed: true } : undefined}
        />
      )
    case 'question':
      return (
        <AskUserQuestionCard
          question={block.data as AskQuestion}
          onAnswer={onQuestionAnswer}
          answered={block.resolved}
        />
      )
    case 'plan':
      return (
        <PlanApprovalCard
          plan={block.data as PlanApproval}
          onApprove={onPlanApprove}
          resolved={block.resolved ? { approved: true } : undefined}
        />
      )
    case 'elicitation':
      return (
        <ElicitationCard
          elicitation={block.data as Elicitation}
          onSubmit={onElicitationSubmit}
          resolved={block.resolved}
        />
      )
    default:
      return null
  }
}
