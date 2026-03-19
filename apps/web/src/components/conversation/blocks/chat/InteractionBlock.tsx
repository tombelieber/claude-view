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
import { useInteractionHandlers } from '../shared/use-interaction-handlers'

interface InteractionBlockProps {
  block: InteractionBlockType
}

export function ChatInteractionBlock({ block }: InteractionBlockProps) {
  const {
    localResponse,
    isPending,
    respondPermission,
    alwaysAllow,
    answerQuestion,
    approvePlan,
    submitElicitation,
  } = useInteractionHandlers(block.requestId)

  const responded = block.resolved || localResponse !== null

  switch (block.variant) {
    case 'permission': {
      const allowed = localResponse?.variant === 'permission' ? localResponse.allowed : true
      return (
        <PermissionCard
          permission={block.data as PermissionRequest}
          onRespond={responded ? undefined : respondPermission}
          onAlwaysAllow={responded ? undefined : alwaysAllow}
          resolved={responded ? { allowed } : undefined}
          isPending={isPending}
        />
      )
    }
    case 'question':
      return (
        <AskUserQuestionCard
          question={block.data as AskQuestion}
          onAnswer={responded ? undefined : answerQuestion}
          answered={responded}
        />
      )
    case 'plan': {
      const approved = localResponse?.variant === 'plan' ? localResponse.approved : true
      return (
        <PlanApprovalCard
          plan={block.data as PlanApproval}
          onApprove={responded ? undefined : approvePlan}
          resolved={responded ? { approved } : undefined}
        />
      )
    }
    case 'elicitation':
      return (
        <ElicitationCard
          elicitation={block.data as Elicitation}
          onSubmit={responded ? undefined : submitElicitation}
          resolved={responded}
        />
      )
    default:
      return null
  }
}
