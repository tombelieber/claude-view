import type { InteractionBlock as InteractionBlockType } from '../../../../types/blocks'
import type {
  AskQuestion,
  Elicitation,
  PermissionRequest,
  PlanApproval,
} from '../../../../types/sidecar-protocol'
import { AskUserQuestionCard } from '../shared/AskUserQuestionCard'
import { ElicitationCard } from '../shared/ElicitationCard'
import { PermissionCard } from '../shared/PermissionCard'
import { PlanApprovalCard } from '../shared/PlanApprovalCard'
import { StatusBadge } from '../shared/StatusBadge'
import { useInteractionHandlers } from '../shared/use-interaction-handlers'

interface InteractionBlockProps {
  block: InteractionBlockType
}

export function ChatInteractionBlock({ block }: InteractionBlockProps) {
  const {
    localResponse,
    respondPermission,
    alwaysAllow,
    answerQuestion,
    approvePlan,
    submitElicitation,
  } = useInteractionHandlers(block.requestId)

  const responded = block.resolved || localResponse !== null

  const provenanceBadge = block.historicalSource ? (
    <div className="mb-1">
      <StatusBadge
        label={block.historicalSource === 'system_variant' ? 'system_variant' : 'inferred'}
        color={block.historicalSource === 'system_variant' ? 'green' : 'amber'}
      />
    </div>
  ) : null

  switch (block.variant) {
    case 'permission': {
      const allowed = localResponse?.variant === 'permission' ? localResponse.allowed : true
      return (
        <>
          {provenanceBadge}
          <PermissionCard
            permission={block.data as PermissionRequest}
            onRespond={responded ? undefined : respondPermission}
            onAlwaysAllow={responded ? undefined : alwaysAllow}
            resolved={responded ? { allowed } : undefined}
          />
        </>
      )
    }
    case 'question':
      return (
        <>
          {provenanceBadge}
          <AskUserQuestionCard
            question={block.data as AskQuestion}
            onAnswer={responded ? undefined : answerQuestion}
            answered={responded}
          />
        </>
      )
    case 'plan': {
      const approved = localResponse?.variant === 'plan' ? localResponse.approved : true
      return (
        <>
          {provenanceBadge}
          <PlanApprovalCard
            plan={block.data as PlanApproval}
            onApprove={responded ? undefined : approvePlan}
            resolved={responded ? { approved } : undefined}
          />
        </>
      )
    }
    case 'elicitation':
      return (
        <>
          {provenanceBadge}
          <ElicitationCard
            elicitation={block.data as Elicitation}
            onSubmit={responded ? undefined : submitElicitation}
            resolved={responded}
          />
        </>
      )
    default:
      return null
  }
}
