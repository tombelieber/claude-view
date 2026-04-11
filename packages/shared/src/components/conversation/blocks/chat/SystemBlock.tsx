import type { SystemBlock as SystemBlockType } from '../../../../types/blocks'
import type { QueueOperation } from '../../../../types/sidecar-protocol'
import {
  AgentNamePill,
  CustomTitlePill,
  PlanContentCard,
  PrLinkCard,
  QueueOperationBubble,
  TaskNotificationPill,
  TaskProgressPill,
  TaskStartedPill,
} from './system-variants'

/** Variants that ChatSystemBlock actually renders — used to filter items before Virtuoso. */
export const CHAT_SYSTEM_VARIANTS = new Set([
  'task_started',
  'task_progress',
  'task_notification',
  'queue_operation',
  'pr_link',
  'custom_title',
  'plan_content',
  'agent_name',
])

/** Returns true if a queue_operation system block should render in chat mode. */
export function isChatVisibleQueueOp(block: SystemBlockType): boolean {
  if (block.variant !== 'queue_operation') return false
  const data = block.data as QueueOperation
  return data.operation === 'enqueue' && !!data.content?.trim()
}

interface SystemBlockProps {
  block: SystemBlockType
}

export function ChatSystemBlock({ block }: SystemBlockProps) {
  switch (block.variant) {
    case 'task_started':
      return <TaskStartedPill data={block.data as never} />
    case 'task_progress':
      return <TaskProgressPill data={block.data as never} />
    case 'task_notification':
      return <TaskNotificationPill data={block.data as never} />
    case 'queue_operation':
      return <QueueOperationBubble data={block.data as never} />
    case 'pr_link':
      return <PrLinkCard data={block.data as Record<string, unknown>} />
    case 'custom_title':
      return <CustomTitlePill data={block.data as Record<string, unknown>} />
    case 'agent_name':
      return <AgentNamePill data={block.data as Record<string, unknown>} />
    case 'plan_content':
      return <PlanContentCard data={block.data as Record<string, unknown>} />
    default: {
      // @ts-expect-error — 17 variants will be added by A1
      const _exhaustive: never = block.variant
      void _exhaustive
      return null
    }
  }
}
