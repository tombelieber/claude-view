import { FileSnapshotCard } from '../../../FileSnapshotCard'
import { LocalCommandEventCard } from '../../../LocalCommandEventCard'
import { MessageQueueEventCard } from '../../../MessageQueueEventCard'
import type { SystemBlock as SystemBlockType } from '../../../../types/blocks'
import type {
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
} from '../../../../types/sidecar-protocol'
import { StopCircle } from 'lucide-react'
import { useConversationActions } from '../../../../contexts/conversation-actions-context'
import { cn } from '../../../../utils/cn'
import { Markdown } from '../shared/Markdown'
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
      <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 text-[10px]">
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
          <span className="text-[9px] font-mono text-gray-400 bg-gray-500/10 px-1.5 py-0.5 rounded">
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
              'text-[9px] font-mono px-1.5 py-0.5 rounded',
              isError ? 'text-red-400 bg-red-500/10' : 'text-green-400 bg-green-500/10',
            )}
          >
            {data.outcome}
          </span>
        ) : undefined
      }
    >
      {(data.stdout || data.stderr) && (
        <pre className="text-[10px] font-mono text-gray-500 dark:text-gray-400 whitespace-pre-wrap max-h-24 overflow-y-auto">
          {(data.stdout || data.stderr || '').slice(0, 200)}
        </pre>
      )}
    </EventCard>
  )
}

function TaskStartedDetail({ data }: { data: TaskStarted }) {
  const convActions = useConversationActions()
  return (
    <EventCard
      dot="blue"
      chip="Task"
      chipColor="bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-700 dark:text-indigo-300"
      label={data.description}
      pulse
      rawData={data}
      meta={
        <div className="flex items-center gap-2 flex-shrink-0">
          <span className="text-[9px] font-mono text-gray-400">{data.taskId.slice(0, 8)}</span>
          {convActions?.stopTask && (
            <button
              type="button"
              onClick={() => convActions.stopTask?.(data.taskId)}
              className="text-red-400 hover:text-red-500 transition-colors duration-200 cursor-pointer"
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
      dot="blue"
      chip="Task"
      chipColor="bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-700 dark:text-indigo-300"
      label={data.summary ?? data.description}
      rawData={data}
    >
      <div className="flex items-center gap-3 text-[10px] font-mono text-gray-500 dark:text-gray-400">
        <span>{data.usage.totalTokens.toLocaleString()} tok</span>
        <span>{data.usage.toolUses} tools</span>
        <span>{(data.usage.durationMs / 1000).toFixed(1)}s</span>
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
      <pre className="text-[10px] font-mono text-gray-500 dark:text-gray-400 whitespace-pre-wrap max-h-24 overflow-y-auto">
        {data.content.slice(0, 500)}
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
  return <EventCard dot="blue" chip="Title" label={data.aiTitle} rawData={data} />
}

function LastPromptDetail({ data }: { data: LastPrompt }) {
  return <EventCard dot="gray" chip="Prompt" label={data.lastPrompt} rawData={data} />
}

function InformationalDetail({ data }: { data: Informational }) {
  return (
    <EventCard dot="blue" chip="Info" label={data.content || data.message || ''} rawData={data} />
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
      case 'last_prompt':
        return <LastPromptDetail data={block.data as LastPrompt} />
      case 'informational':
        return <InformationalDetail data={block.data as Informational} />
      default:
        return null
    }
  })()

  return (
    <>
      {variantContent}
      {block.rawJson && (
        <div className="mt-1 space-y-1">
          {block.rawJson.permissionMode != null && (
            <span className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300">
              {String(block.rawJson.permissionMode)}
            </span>
          )}
          {block.rawJson.durationMs != null && (
            <span
              className={cn(
                'text-[9px] font-mono tabular-nums px-1.5 py-0.5 rounded',
                Number(block.rawJson.durationMs) > 30000
                  ? 'text-red-400 bg-red-500/10'
                  : Number(block.rawJson.durationMs) > 5000
                    ? 'text-amber-400 bg-amber-500/10'
                    : 'text-gray-400 bg-gray-500/10',
              )}
            >
              {Number(block.rawJson.durationMs).toLocaleString()}ms
            </span>
          )}
          {typeof block.rawJson.planContent === 'string' && block.rawJson.planContent && (
            <details className="mt-1">
              <summary className="text-[10px] text-gray-500 dark:text-gray-400 cursor-pointer">
                Plan content
              </summary>
              <Markdown content={block.rawJson.planContent} />
            </details>
          )}
          {block.rawJson.prUrl != null && (
            <a
              href={String(block.rawJson.prUrl)}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center text-[10px] font-mono text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
            >
              PR #{String(block.rawJson.prNumber ?? '')}
            </a>
          )}
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
