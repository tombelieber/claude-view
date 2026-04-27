import type { SystemBlock as SystemBlockType } from '../../../../types/blocks'
import type { QueueOperation } from '../../../../types/sidecar-protocol'
import {
  AgentNamePill,
  AiTitlePill,
  AttachmentPill,
  AwaySummaryPill,
  CommandOutputBlock,
  CustomTitlePill,
  ElicitationCompletePill,
  FileHistorySnapshotPill,
  FilesSavedPill,
  HookEventPill,
  InformationalBlock,
  LastPromptPill,
  LocalCommandBlock,
  PermissionModeChangePill,
  PlanContentCard,
  PrLinkCard,
  QueueOperationBubble,
  ScheduledTaskFirePill,
  SessionInitPill,
  SessionStatusPill,
  StreamDeltaPill,
  TaskNotificationPill,
  TaskProgressPill,
  TaskStartedPill,
  UnknownSystemPill,
  WorktreeStatePill,
} from './system-variants'

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
    case 'session_init':
      return <SessionInitPill data={block.data as never} />
    case 'session_status':
      return <SessionStatusPill data={block.data as never} />
    case 'elicitation_complete':
      return <ElicitationCompletePill data={block.data as never} />
    case 'hook_event':
      return <HookEventPill data={block.data as never} />
    case 'task_started':
      return <TaskStartedPill data={block.data as never} />
    case 'task_progress':
      return <TaskProgressPill data={block.data as never} />
    case 'task_notification':
      return <TaskNotificationPill data={block.data as never} />
    case 'files_saved':
      return <FilesSavedPill data={block.data as never} />
    case 'command_output':
      return <CommandOutputBlock data={block.data as never} />
    case 'stream_delta':
      return <StreamDeltaPill data={block.data as never} />
    case 'local_command':
      return <LocalCommandBlock data={block.data as never} />
    case 'queue_operation':
      return <QueueOperationBubble data={block.data as never} />
    case 'file_history_snapshot':
      return <FileHistorySnapshotPill data={block.data as never} />
    case 'ai_title':
      return <AiTitlePill data={block.data as never} />
    case 'last_prompt':
      return <LastPromptPill data={block.data as never} />
    case 'worktree_state':
      return <WorktreeStatePill data={block.data as never} />
    case 'pr_link':
      return <PrLinkCard data={block.data as Record<string, unknown>} />
    case 'custom_title':
      return <CustomTitlePill data={block.data as Record<string, unknown>} />
    case 'plan_content':
      return <PlanContentCard data={block.data as Record<string, unknown>} />
    case 'informational':
      return <InformationalBlock data={block.data as never} />
    case 'agent_name':
      return <AgentNamePill data={block.data as Record<string, unknown>} />
    case 'attachment':
      return <AttachmentPill data={block.data as Record<string, unknown>} />
    case 'permission_mode_change':
      return <PermissionModeChangePill data={block.data as Record<string, unknown>} />
    case 'scheduled_task_fire':
      return <ScheduledTaskFirePill data={block.data as Record<string, unknown>} />
    case 'away_summary':
      return <AwaySummaryPill data={block.data as Record<string, unknown>} />
    case 'unknown':
      return <UnknownSystemPill data={block.data as Record<string, unknown>} />
    default: {
      const _exhaustive: never = block.variant
      void _exhaustive
      return null
    }
  }
}
