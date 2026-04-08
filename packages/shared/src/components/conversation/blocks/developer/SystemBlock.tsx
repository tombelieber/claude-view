import { ExternalLink, FileText, StopCircle, Tag } from 'lucide-react'
import { useConversationActions } from '../../../../contexts/conversation-actions-context'
import type { SystemBlock as SystemBlockType } from '../../../../types/blocks'
import type {
  AgentName,
  AiTitle,
  CommandOutput,
  ElicitationComplete,
  FileHistorySnapshot,
  FilesSaved,
  Informational,
  LastPrompt,
  LocalCommand,
  QueueOperation,
  SessionInit,
  HookEvent as SidecarHookEvent,
  SessionStatus as SidecarSessionStatus,
  StreamDelta,
  TaskNotification,
  TaskProgressEvent,
  TaskStarted,
  UnknownSdkEvent,
  WorktreeState,
} from '../../../../types/sidecar-protocol'
import { cn } from '../../../../utils/cn'
import { FileSnapshotCard } from '../../../FileSnapshotCard'
import { LocalCommandEventCard } from '../../../LocalCommandEventCard'
import { MessageQueueEventCard } from '../../../MessageQueueEventCard'
import { Markdown } from '../shared/Markdown'
import { DurationBadge } from './DurationBadge'
import { EventCard } from './EventCard'
import { RENDERED_KEYS as API_ERROR_KEYS, ApiErrorDetail } from './details/ApiErrorDetail'
import { RENDERED_KEYS as HOOK_KEYS, HookMetadataDetail } from './details/HookMetadataDetail'
import { RENDERED_KEYS as LINEAGE_KEYS, MessageLineageDetail } from './details/MessageLineageDetail'
import { RawEnvelopeDetail } from './details/RawEnvelopeDetail'
import { RENDERED_KEYS as RETRY_KEYS, RetryDetail } from './details/RetryDetail'
import { RENDERED_KEYS as STOP_KEYS, StopReasonDetail } from './details/StopReasonDetail'

const SYSTEM_RENDERED_KEYS = [
  ...RETRY_KEYS,
  ...API_ERROR_KEYS,
  ...HOOK_KEYS,
  ...STOP_KEYS,
  ...LINEAGE_KEYS,
  'permissionMode',
  'planContent',
  'prUrl',
  'prNumber',
  'prRepository',
  'customTitle',
  'promptId',
  'durationMs',
] as string[]

interface SystemBlockProps {
  block: SystemBlockType
}

// ── Variant renderers ───────────────────────────────────────────────────────

function SessionInitDetail({ data }: { data: SessionInit }) {
  return (
    <EventCard
      dot="green"
      chip="Init"
      label={`${data.model} — ${data.tools.length} tools`}
      rawData={data}
    >
      <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 text-xs">
        <span className="text-gray-500 dark:text-gray-400">Model</span>
        <span className="font-mono text-gray-700 dark:text-gray-300 truncate">{data.model}</span>
        <span className="text-gray-500 dark:text-gray-400">Mode</span>
        <span className="font-mono text-gray-700 dark:text-gray-300">{data.permissionMode}</span>
        <span className="text-gray-500 dark:text-gray-400">CWD</span>
        <span className="font-mono text-gray-700 dark:text-gray-300 truncate">{data.cwd}</span>
        {data.agents.length > 0 && (
          <>
            <span className="text-gray-500 dark:text-gray-400">Agents</span>
            <span className="font-mono text-gray-700 dark:text-gray-300">
              {data.agents.join(', ')}
            </span>
          </>
        )}
      </div>
    </EventCard>
  )
}

function SessionStatusDetail({ data }: { data: SidecarSessionStatus }) {
  return (
    <EventCard
      dot={data.status === 'compacting' ? 'amber' : 'blue'}
      chip="Status"
      label={data.status ?? 'idle'}
      pulse={data.status === 'compacting'}
      rawData={data}
      meta={
        data.permissionMode ? (
          <span className="text-xs font-mono text-gray-500 dark:text-gray-400 bg-gray-500/10 px-1.5 py-0.5 rounded">
            {data.permissionMode}
          </span>
        ) : undefined
      }
    />
  )
}

function ElicitationCompleteDetail({ data }: { data: ElicitationComplete }) {
  return (
    <EventCard
      dot="green"
      chip="Elicitation"
      label={`${data.mcpServerName} / ${data.elicitationId}`}
      rawData={data}
    />
  )
}

function HookEventDetail({ data }: { data: SidecarHookEvent }) {
  const isError = data.outcome === 'error'
  return (
    <EventCard
      dot={isError ? 'red' : 'amber'}
      chip="Hook"
      label={`${data.hookName} (${data.phase})`}
      error={isError}
      rawData={data}
      meta={
        data.outcome ? (
          <span
            className={cn(
              'text-xs font-mono px-1.5 py-0.5 rounded',
              isError
                ? 'text-red-600 dark:text-red-400 bg-red-500/10'
                : 'text-green-600 dark:text-green-400 bg-green-500/10',
            )}
          >
            {data.outcome}
          </span>
        ) : undefined
      }
    >
      {(data.stdout || data.stderr) && (
        <pre className="text-xs font-mono text-gray-500 dark:text-gray-400 whitespace-pre-wrap max-h-64 overflow-y-auto">
          {data.stdout || data.stderr}
        </pre>
      )}
    </EventCard>
  )
}

function TaskStartedDetail({ data }: { data: TaskStarted }) {
  const convActions = useConversationActions()
  return (
    <EventCard
      dot="indigo"
      chip="Task"
      chipColor="bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-700 dark:text-indigo-300"
      label={data.description}
      pulse
      rawData={data}
      meta={
        <div className="flex items-center gap-2 flex-shrink-0">
          <span className="text-xs font-mono text-gray-500 dark:text-gray-400">
            {data.taskId.slice(0, 8)}
          </span>
          {convActions?.stopTask && (
            <button
              type="button"
              onClick={() => convActions.stopTask?.(data.taskId)}
              className="text-red-500 dark:text-red-400 hover:text-red-600 dark:hover:text-red-500 transition-colors duration-200 cursor-pointer"
              title="Stop task"
            >
              <StopCircle className="w-3 h-3" />
            </button>
          )}
        </div>
      }
    />
  )
}

function TaskProgressDetail({ data }: { data: TaskProgressEvent }) {
  return (
    <EventCard
      dot="indigo"
      chip="Task"
      chipColor="bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-700 dark:text-indigo-300"
      label={data.summary ?? data.description}
      rawData={data}
    >
      <div className="flex items-center gap-3 text-xs font-mono text-gray-500 dark:text-gray-400">
        <span>{(data.usage?.totalTokens ?? 0).toLocaleString()} tok</span>
        <span>{data.usage?.toolUses ?? 0} tools</span>
        <span>{((data.usage?.durationMs ?? 0) / 1000).toFixed(1)}s</span>
      </div>
    </EventCard>
  )
}

function TaskNotificationDetail({ data }: { data: TaskNotification }) {
  const isFailed = data.status === 'failed'
  return (
    <EventCard
      dot={isFailed ? 'red' : 'green'}
      chip="Task"
      chipColor="bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-700 dark:text-indigo-300"
      label={`${data.status}: ${data.summary}`}
      error={isFailed}
      rawData={data}
    />
  )
}

function FilesSavedDetail({ data }: { data: FilesSaved }) {
  return (
    <EventCard
      dot={data.failed.length > 0 ? 'amber' : 'green'}
      chip="Files"
      label={`${data.files.length} saved${data.failed.length > 0 ? `, ${data.failed.length} failed` : ''}`}
      rawData={data}
    />
  )
}

function CommandOutputDetail({ data }: { data: CommandOutput }) {
  return (
    <EventCard
      dot="gray"
      chip="Output"
      chipColor="bg-gray-500/10 dark:bg-gray-500/20 text-gray-700 dark:text-gray-300"
      rawData={data}
    >
      <pre className="text-xs font-mono text-gray-500 dark:text-gray-400 whitespace-pre-wrap max-h-64 overflow-y-auto">
        {data.content}
      </pre>
    </EventCard>
  )
}

function StreamDeltaDetail({ data }: { data: StreamDelta }) {
  return (
    <EventCard
      dot="gray"
      chip="Delta"
      label={`stream_delta [${data.messageId.slice(0, 8)}]`}
      rawData={data}
    />
  )
}

function UnknownDetail({ data }: { data: UnknownSdkEvent }) {
  return <EventCard dot="gray" chip="Unknown" label={data.sdkType} rawData={data} />
}

// ── Shared card wrappers for special system subtypes ────────────────────────

function LocalCommandDetail({ data }: { data: LocalCommand }) {
  return (
    <EventCard dot="gray" chip="Command" label={data.content || data.command || ''} rawData={data}>
      <LocalCommandEventCard content={data.content || data.command || ''} />
    </EventCard>
  )
}

function QueueOperationDetail({ data }: { data: QueueOperation }) {
  return (
    <EventCard
      dot="orange"
      chip="Queue"
      label={`${data.operation}${data.content ? `: ${data.content}` : ''}`}
      rawData={data}
    >
      <MessageQueueEventCard
        operation={data.operation}
        timestamp={data.timestamp}
        content={data.content}
      />
    </EventCard>
  )
}

function FileHistorySnapshotDetail({ data }: { data: FileHistorySnapshot }) {
  const files = data.files ?? Object.keys(data.snapshot?.trackedFileBackups ?? {})
  const fileCount = files.length || data.fileCount || 0
  return (
    <EventCard
      dot="teal"
      chip="Snapshot"
      label={`${fileCount} file(s)${data.isIncremental ? ' (incremental)' : ''}`}
      rawData={data}
    >
      <FileSnapshotCard
        fileCount={fileCount}
        timestamp={data.snapshot?.timestamp ?? ''}
        files={files}
        isIncremental={data.isIncremental ?? false}
        verboseMode
      />
    </EventCard>
  )
}

function AiTitleDetail({ data }: { data: AiTitle }) {
  return (
    <EventCard
      dot="blue"
      chip="Title"
      chipColor="bg-blue-500/10 dark:bg-blue-500/20 text-blue-700 dark:text-blue-300"
      label={data.aiTitle}
      rawData={data}
      meta={
        <span className="text-xs font-mono text-gray-500 dark:text-gray-400 bg-gray-500/10 px-1.5 py-0.5 rounded">
          {data.sessionId?.slice(0, 8)}
        </span>
      }
    />
  )
}

function WorktreeStateDetail({ data }: { data: WorktreeState }) {
  const wt = data.worktreeSession
  return (
    <EventCard
      dot="teal"
      chip="Worktree"
      chipColor="bg-teal-500/10 dark:bg-teal-500/20 text-teal-700 dark:text-teal-300"
      label={`${wt.worktreeName} (${wt.worktreeBranch})`}
      rawData={data}
    >
      <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 text-xs">
        <span className="text-gray-500 dark:text-gray-400">Worktree</span>
        <span className="font-mono text-gray-700 dark:text-gray-300 truncate">
          {wt.worktreeName}
        </span>
        <span className="text-gray-500 dark:text-gray-400">Branch</span>
        <span className="font-mono text-gray-700 dark:text-gray-300 truncate">
          {wt.worktreeBranch}
        </span>
        <span className="text-gray-500 dark:text-gray-400">Original</span>
        <span className="font-mono text-gray-700 dark:text-gray-300 truncate">
          {wt.originalBranch}
        </span>
        <span className="text-gray-500 dark:text-gray-400">Base Commit</span>
        <span className="font-mono text-gray-700 dark:text-gray-300 truncate">
          {wt.originalHeadCommit?.slice(0, 10)}
        </span>
        <span className="text-gray-500 dark:text-gray-400">Path</span>
        <span className="font-mono text-gray-700 dark:text-gray-300 truncate">
          {wt.worktreePath}
        </span>
      </div>
    </EventCard>
  )
}

function LastPromptDetail({ data }: { data: LastPrompt }) {
  return <EventCard dot="gray" chip="Prompt" label={data.lastPrompt} rawData={data} />
}

function InformationalDetail({ data }: { data: Informational }) {
  return (
    <EventCard dot="blue" chip="Info" label={data.content || data.message || ''} rawData={data} />
  )
}

function PrLinkDetail({ data }: { data: Record<string, unknown> }) {
  const prUrl = data.prUrl as string | undefined
  const prNumber = data.prNumber as number | undefined
  const prRepo = data.prRepository as string | undefined
  return (
    <EventCard
      dot="blue"
      chip="PR"
      label={prRepo ? `${prRepo}#${prNumber}` : `PR #${prNumber}`}
      rawData={data}
    >
      {prUrl && (
        <a
          href={prUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-1 text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
        >
          <ExternalLink className="w-3 h-3 flex-shrink-0" />
          <span className="truncate font-mono">{prUrl}</span>
        </a>
      )}
    </EventCard>
  )
}

function CustomTitleDetail({ data }: { data: Record<string, unknown> }) {
  const title = (data.customTitle as string) ?? ''
  return (
    <EventCard
      dot="blue"
      chip="Title"
      chipColor="bg-blue-500/10 dark:bg-blue-500/20 text-blue-700 dark:text-blue-300"
      label={title}
      rawData={data}
      meta={<Tag className="w-3 h-3 text-gray-400" />}
    />
  )
}

function PlanContentDetail({ data }: { data: Record<string, unknown> }) {
  const content = (data.planContent as string) || ''
  return (
    <EventCard
      dot="purple"
      chip="Plan"
      chipColor="bg-violet-500/10 dark:bg-violet-500/20 text-violet-700 dark:text-violet-300"
      rawData={data}
      meta={<FileText className="w-3 h-3 text-violet-400" />}
    >
      <div className="text-xs">
        <Markdown content={content} />
      </div>
    </EventCard>
  )
}

// ── Zero-gap pipeline: attachment, permission_mode_change, scheduled_task_fire ──

function AttachmentDetail({ data }: { data: Record<string, unknown> }) {
  const att = data.attachment as Record<string, unknown> | undefined
  const addedNames = (att?.addedNames ?? []) as string[]
  const removedNames = (att?.removedNames ?? []) as string[]
  const label = `${addedNames.length} added, ${removedNames.length} removed`
  return (
    <EventCard dot="blue" chip="Attach" label={label} rawData={data}>
      {addedNames.length > 0 && (
        <div className="text-xs font-mono text-gray-500 dark:text-gray-400 space-y-0.5">
          {addedNames.map((n) => (
            <div key={n}>+ {n}</div>
          ))}
        </div>
      )}
      {removedNames.length > 0 && (
        <div className="text-xs font-mono text-red-500/70 dark:text-red-400/70 space-y-0.5">
          {removedNames.map((n) => (
            <div key={n}>− {n}</div>
          ))}
        </div>
      )}
    </EventCard>
  )
}

function PermissionModeChangeDetail({ data }: { data: Record<string, unknown> }) {
  const mode = (data.permissionMode as string) ?? 'unknown'
  return (
    <EventCard
      dot="amber"
      chip="Permission"
      chipColor="bg-amber-500/10 dark:bg-amber-500/20 text-amber-700 dark:text-amber-300"
      label={mode}
      rawData={data}
    />
  )
}

function ScheduledTaskFireDetail({ data }: { data: Record<string, unknown> }) {
  return (
    <EventCard
      dot="orange"
      chip="Scheduled"
      chipColor="bg-orange-500/10 dark:bg-orange-500/20 text-orange-700 dark:text-orange-300"
      rawData={data}
    />
  )
}

// ── Main dispatcher ─────────────────────────────────────────────────────────

export function DevSystemBlock({ block }: SystemBlockProps) {
  const variantContent = (() => {
    switch (block.variant) {
      case 'session_init':
        return <SessionInitDetail data={block.data as SessionInit} />
      case 'session_status':
        return <SessionStatusDetail data={block.data as SidecarSessionStatus} />
      case 'elicitation_complete':
        return <ElicitationCompleteDetail data={block.data as ElicitationComplete} />
      case 'hook_event':
        return <HookEventDetail data={block.data as SidecarHookEvent} />
      case 'task_started':
        return <TaskStartedDetail data={block.data as TaskStarted} />
      case 'task_progress':
        return <TaskProgressDetail data={block.data as TaskProgressEvent} />
      case 'task_notification':
        return <TaskNotificationDetail data={block.data as TaskNotification} />
      case 'files_saved':
        return <FilesSavedDetail data={block.data as FilesSaved} />
      case 'command_output':
        return <CommandOutputDetail data={block.data as CommandOutput} />
      case 'stream_delta':
        return <StreamDeltaDetail data={block.data as StreamDelta} />
      case 'unknown':
        return <UnknownDetail data={block.data as UnknownSdkEvent} />
      case 'local_command':
        return <LocalCommandDetail data={block.data as LocalCommand} />
      case 'queue_operation':
        return <QueueOperationDetail data={block.data as QueueOperation} />
      case 'file_history_snapshot':
        return <FileHistorySnapshotDetail data={block.data as FileHistorySnapshot} />
      case 'ai_title':
        return <AiTitleDetail data={block.data as AiTitle} />
      case 'worktree_state':
        return <WorktreeStateDetail data={block.data as WorktreeState} />
      case 'last_prompt':
        return <LastPromptDetail data={block.data as LastPrompt} />
      case 'informational':
        return <InformationalDetail data={block.data as Informational} />
      case 'agent_name':
        return (
          <EventCard
            dot="blue"
            chip="Agent"
            label={(block.data as AgentName).agentName}
            rawData={block.data}
          />
        )
      case 'pr_link':
        return <PrLinkDetail data={block.data as Record<string, unknown>} />
      case 'custom_title':
        return <CustomTitleDetail data={block.data as Record<string, unknown>} />
      case 'plan_content':
        return <PlanContentDetail data={block.data as Record<string, unknown>} />
      case 'attachment':
        return <AttachmentDetail data={block.data as Record<string, unknown>} />
      case 'permission_mode_change':
        return <PermissionModeChangeDetail data={block.data as Record<string, unknown>} />
      case 'scheduled_task_fire':
        return <ScheduledTaskFireDetail data={block.data as Record<string, unknown>} />
      default: {
        // Exhaustiveness check: if a new SystemVariant is added in blocks.ts,
        // TypeScript will error here because `_exhaustive` is not `never`.
        // This makes "dev mode must be a superset of chat mode" structurally
        // enforced at compile time — no runtime test needed.
        const _exhaustive: never = block.variant
        void _exhaustive
        return null
      }
    }
  })()

  return (
    <>
      {variantContent}
      {block.rawJson && (
        <div className="ml-4 pl-3 border-l-2 border-gray-200/30 dark:border-gray-700/30 mt-1 space-y-1">
          {block.rawJson.permissionMode != null && (
            <span className="font-mono text-xs px-1.5 py-0.5 rounded bg-gray-500/10 dark:bg-gray-500/20 text-gray-600 dark:text-gray-300">
              {String(block.rawJson.permissionMode)}
            </span>
          )}
          {block.rawJson.durationMs != null && (
            <DurationBadge ms={Number(block.rawJson.durationMs)} />
          )}
          {/* planContent, prUrl, prNumber, customTitle are rendered via
              first-class switch cases above (plan_content, pr_link,
              custom_title variants). The raw-envelope dump below keeps them
              in SYSTEM_RENDERED_KEYS so they don't duplicate there either. */}
          <RetryDetail rawJson={block.rawJson} />
          <ApiErrorDetail rawJson={block.rawJson} />
          <HookMetadataDetail rawJson={block.rawJson} />
          <StopReasonDetail rawJson={block.rawJson} />
          <MessageLineageDetail rawJson={block.rawJson} />
          <RawEnvelopeDetail rawJson={block.rawJson} renderedKeys={SYSTEM_RENDERED_KEYS} />
        </div>
      )}
    </>
  )
}
