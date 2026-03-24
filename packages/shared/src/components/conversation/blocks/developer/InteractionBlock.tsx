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
import { useInteractionHandlers } from '../shared/use-interaction-handlers'
import { EventCard } from './EventCard'

interface InteractionBlockProps {
  block: InteractionBlockType
}

const VARIANT_CONFIG: Record<
  string,
  { chip: string; chipColor: string; dot: 'purple' | 'blue' | 'amber' }
> = {
  permission: {
    chip: 'Permission',
    chipColor: 'bg-amber-500/10 dark:bg-amber-500/20 text-amber-700 dark:text-amber-300',
    dot: 'amber',
  },
  question: {
    chip: 'Question',
    chipColor: 'bg-amber-500/10 dark:bg-amber-500/20 text-amber-700 dark:text-amber-300',
    dot: 'amber',
  },
  plan: {
    chip: 'Plan',
    chipColor: 'bg-purple-500/10 dark:bg-purple-500/20 text-purple-700 dark:text-purple-300',
    dot: 'purple',
  },
  elicitation: {
    chip: 'Elicitation',
    chipColor: 'bg-blue-500/10 dark:bg-blue-500/20 text-blue-700 dark:text-blue-300',
    dot: 'blue',
  },
}

export function DevInteractionBlock({ block }: InteractionBlockProps) {
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
  const config = VARIANT_CONFIG[block.variant] ?? {
    chip: block.variant,
    chipColor: '',
    dot: 'blue' as const,
  }

  const richContent = (() => {
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
  })()

  return (
    <EventCard
      dot={config.dot}
      chip={config.chip}
      chipColor={config.chipColor}
      label={(() => {
        switch (block.variant) {
          case 'permission': {
            const perm = block.data as { toolName?: string }
            return perm.toolName
              ? `${block.requestId.slice(0, 8)} · ${perm.toolName}`
              : block.requestId.slice(0, 8)
          }
          case 'question': {
            const q = block.data as { question?: string }
            return q.question?.slice(0, 50) || block.requestId.slice(0, 8)
          }
          case 'plan':
            return `Plan approval · ${block.requestId.slice(0, 8)}`
          case 'elicitation': {
            const e = block.data as { mcpServerName?: string }
            return e.mcpServerName || block.requestId.slice(0, 8)
          }
          default:
            return block.requestId.slice(0, 8)
        }
      })()}
      rawData={block}
    >
      {richContent}
    </EventCard>
  )
}
