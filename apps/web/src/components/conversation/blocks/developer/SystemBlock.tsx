import type { SystemBlock as SystemBlockType } from '@claude-view/shared/types/blocks'
import type {
  CommandOutput,
  ElicitationComplete,
  FilesSaved,
  SessionInit,
  HookEvent as SidecarHookEvent,
  SessionStatus as SidecarSessionStatus,
  StreamDelta,
  TaskNotification,
  TaskProgressEvent,
  TaskStarted,
  UnknownSdkEvent,
} from '@claude-view/shared/types/sidecar-protocol'
import {
  Activity,
  Bell,
  Code,
  FileText,
  GitBranch,
  Play,
  Settings,
  Terminal,
  Zap,
} from 'lucide-react'

interface SystemBlockProps {
  block: SystemBlockType
}

function SessionInitDetail({ data }: { data: SessionInit }) {
  return (
    <div className="rounded border border-gray-200/50 dark:border-gray-700/50 overflow-hidden">
      <div className="flex items-center gap-2 px-2.5 py-1.5 bg-gray-50 dark:bg-gray-800/40 border-b border-gray-200/50 dark:border-gray-700/50">
        <Settings className="w-3 h-3 text-gray-500 dark:text-gray-400" />
        <span className="text-[10px] font-medium text-gray-600 dark:text-gray-300">
          Session Init
        </span>
      </div>
      <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 px-2.5 py-1.5 text-[10px]">
        <div className="text-gray-500 dark:text-gray-400">Model</div>
        <div className="font-mono text-gray-700 dark:text-gray-300 truncate">{data.model}</div>
        <div className="text-gray-500 dark:text-gray-400">Mode</div>
        <div className="font-mono text-gray-700 dark:text-gray-300">{data.permissionMode}</div>
        <div className="text-gray-500 dark:text-gray-400">Tools</div>
        <div className="font-mono text-gray-700 dark:text-gray-300">{data.tools.length}</div>
        <div className="text-gray-500 dark:text-gray-400">CWD</div>
        <div className="font-mono text-gray-700 dark:text-gray-300 truncate">{data.cwd}</div>
        {data.agents.length > 0 && (
          <>
            <div className="text-gray-500 dark:text-gray-400">Agents</div>
            <div className="font-mono text-gray-700 dark:text-gray-300">
              {data.agents.join(', ')}
            </div>
          </>
        )}
      </div>
    </div>
  )
}

function SessionStatusDetail({ data }: { data: SidecarSessionStatus }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-[10px] text-gray-500 dark:text-gray-400">
      <Activity className="w-3 h-3" />
      <span>Status: {data.status ?? 'idle'}</span>
      {data.permissionMode && <span className="font-mono">({data.permissionMode})</span>}
    </div>
  )
}

function ElicitationCompleteDetail({ data }: { data: ElicitationComplete }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-[10px] text-gray-500 dark:text-gray-400">
      <Code className="w-3 h-3" />
      <span>
        Elicitation complete: {data.mcpServerName} / {data.elicitationId}
      </span>
    </div>
  )
}

function HookEventDetail({ data }: { data: SidecarHookEvent }) {
  return (
    <div className="flex items-start gap-2 px-3 py-1.5 text-[10px]">
      <GitBranch className="w-3 h-3 text-gray-500 dark:text-gray-400 flex-shrink-0 mt-0.5" />
      <div>
        <div className="text-gray-600 dark:text-gray-300">
          Hook: {data.hookName} ({data.phase})
        </div>
        {data.outcome && (
          <div className="text-gray-500 dark:text-gray-400">Outcome: {data.outcome}</div>
        )}
        {data.stdout && (
          <pre className="font-mono text-gray-500 dark:text-gray-400 whitespace-pre-wrap mt-0.5">
            {data.stdout.slice(0, 200)}
          </pre>
        )}
      </div>
    </div>
  )
}

function TaskStartedDetail({ data }: { data: TaskStarted }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-[10px] text-gray-500 dark:text-gray-400">
      <Play className="w-3 h-3" />
      <span>Task started: {data.description}</span>
      <span className="font-mono">[{data.taskId.slice(0, 8)}]</span>
    </div>
  )
}

function TaskProgressDetail({ data }: { data: TaskProgressEvent }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-[10px] text-gray-500 dark:text-gray-400">
      <Activity className="w-3 h-3" />
      <span className="truncate">{data.summary ?? data.description}</span>
      <span className="font-mono flex-shrink-0">
        {data.usage.totalTokens.toLocaleString()}tok / {data.usage.toolUses} tools
      </span>
    </div>
  )
}

function TaskNotificationDetail({ data }: { data: TaskNotification }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-[10px] text-gray-500 dark:text-gray-400">
      <Bell className="w-3 h-3" />
      <span className={data.status === 'failed' ? 'text-red-500 dark:text-red-400' : ''}>
        Task {data.status}: {data.summary}
      </span>
    </div>
  )
}

function FilesSavedDetail({ data }: { data: FilesSaved }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-[10px] text-gray-500 dark:text-gray-400">
      <FileText className="w-3 h-3" />
      <span>
        {data.files.length} file(s) saved
        {data.failed.length > 0 && `, ${data.failed.length} failed`}
      </span>
    </div>
  )
}

function CommandOutputDetail({ data }: { data: CommandOutput }) {
  return (
    <div className="flex items-start gap-2 px-3 py-1.5 text-[10px]">
      <Terminal className="w-3 h-3 text-gray-500 dark:text-gray-400 flex-shrink-0 mt-0.5" />
      <pre className="font-mono text-gray-600 dark:text-gray-400 whitespace-pre-wrap max-h-24 overflow-y-auto">
        {data.content.slice(0, 500)}
      </pre>
    </div>
  )
}

function StreamDeltaDetail({ data }: { data: StreamDelta }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-[10px] text-gray-500 dark:text-gray-400">
      <Zap className="w-3 h-3" />
      <span className="font-mono">stream_delta [{data.messageId.slice(0, 8)}]</span>
    </div>
  )
}

function UnknownDetail({ data }: { data: UnknownSdkEvent }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-[10px] text-gray-500 dark:text-gray-400">
      <Code className="w-3 h-3" />
      <span className="font-mono">Unknown: {data.sdkType}</span>
    </div>
  )
}

export function DevSystemBlock({ block }: SystemBlockProps) {
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
    default:
      return null
  }
}
