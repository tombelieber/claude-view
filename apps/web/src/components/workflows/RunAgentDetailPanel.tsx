import { Bot, Clock3, Wrench } from 'lucide-react'
import { Virtuoso } from 'react-virtuoso'
import { cn } from '../../lib/utils'
import type { WorkflowAgentDetail } from '../../types/generated/WorkflowAgentDetail'
import type { WorkflowAgentEvent } from '../../types/generated/WorkflowAgentEvent'
import type { WorkflowRunDetail } from '../../types/generated/WorkflowRunDetail'
import { EVENT_VIRTUALIZE_THRESHOLD, formatDate } from './run-detail-format'

const TOOL_EVENT_RE = /^\s*(tool_use|tool_result)\s+([A-Za-z][\w.-]*)\s*$/

type EventDisplay = {
  event: WorkflowAgentEvent
  toolNames: string[]
  label: string
  body: string
  input?: string
  output?: string | null
}

type ToolCallDisplay = EventDisplay & {
  input: string
  output: string | null
}

type LegacyToolResult = {
  content: string
  isError: boolean
  toolUseId: string | null
}

type PendingToolUse = {
  display: EventDisplay
  index: number
}

function legacyToolName(preview: string): string | null {
  const match = preview.match(/(?:^|\s)(?:tool_use|tool_result)\s+([A-Za-z][\w.-]*)/)
  return match?.[1] ?? null
}

function eventToolNames(event: WorkflowAgentEvent): string[] {
  const names = event.toolNames ?? []
  if (names.length > 0) return names
  const legacy = legacyToolName(event.preview)
  return legacy ? [legacy] : []
}

function parseLegacyToolResult(preview: string): LegacyToolResult | null {
  const trimmed = preview.trim()
  if (!trimmed.startsWith('{')) return null

  try {
    const parsed = JSON.parse(trimmed) as unknown
    if (!parsed || typeof parsed !== 'object') return null
    const record = parsed as Record<string, unknown>
    if (record.type !== 'tool_result' && !('tool_use_id' in record)) return null

    const content = record.content
    const isError = record.is_error === true
    const toolUseId = typeof record.tool_use_id === 'string' ? record.tool_use_id : null
    if (typeof content === 'string') return { content, isError, toolUseId }
    if (Array.isArray(content)) {
      const text = content
        .map((item) => {
          if (typeof item === 'string') return item
          if (item && typeof item === 'object' && 'text' in item) {
            const text = (item as Record<string, unknown>).text
            return typeof text === 'string' ? text : ''
          }
          return ''
        })
        .filter(Boolean)
        .join('\n')
      return text ? { content: text, isError, toolUseId } : null
    }
  } catch {
    return null
  }

  return null
}

function formatJsonScalar(value: unknown): string {
  if (value === null) return 'null'
  if (typeof value === 'string') return value
  if (typeof value === 'number' || typeof value === 'boolean') return String(value)
  return JSON.stringify(value)
}

function formatLegacyContent(content: string): string {
  const trimmed = content.trim()
  if (!trimmed.startsWith('{') && !trimmed.startsWith('[')) return content

  try {
    const parsed = JSON.parse(trimmed) as unknown
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return Object.entries(parsed as Record<string, unknown>)
        .map(([key, value]) => `${key}: ${formatJsonScalar(value)}`)
        .join('\n')
    }
    return JSON.stringify(parsed, null, 2)
  } catch {
    return content
  }
}

function legacyResultToolName(result: LegacyToolResult, fallback: string | null): string | null {
  if (result.content.startsWith('Launching skill:')) return 'Skill'
  if (result.content.startsWith('Structured output provided')) return 'StructuredOutput'
  return fallback
}

function legacyResultStatus(result: LegacyToolResult): 'completed' | 'failed' {
  return result.isError ? 'failed' : 'completed'
}

function eventLabel(
  event: WorkflowAgentEvent,
  toolNames: string[],
  legacyResult: LegacyToolResult | null,
): string {
  const toolName = toolNames[0] ?? null
  if (legacyResult) {
    const status = legacyResultStatus(legacyResult)
    return [toolName ?? 'Tool', status].filter(Boolean).join(' · ')
  }

  const legacyEvent = event.preview.match(TOOL_EVENT_RE)
  if (legacyEvent?.[1] === 'tool_use' && toolName) return `${toolName} · started`
  if (legacyEvent?.[1] === 'tool_result' && toolName) return `${toolName} · result`
  if (event.kind === 'tool_use' && toolName) return `${toolNames.join(', ')} · started`
  if (event.kind === 'tool_result' && toolName) return `${toolNames.join(', ')} · result`

  return [event.role, event.kind].filter(Boolean).join(' · ') || 'event'
}

function readableEventBody(
  event: WorkflowAgentEvent,
  toolNames: string[],
  legacyResult = parseLegacyToolResult(event.preview),
): string {
  const body = event.toolResultPreview ?? event.toolInputPreview
  if (body) return body

  if (legacyResult) {
    return formatLegacyContent(legacyResult.content)
  }

  const legacyEvent = event.preview.match(TOOL_EVENT_RE)
  if (legacyEvent?.[1] === 'tool_use' && toolNames[0]) {
    return `${toolNames[0]} input details are not available from this server response.`
  }
  if (legacyEvent?.[1] === 'tool_result' && toolNames[0]) {
    return `${toolNames[0]} returned a result`
  }
  return event.preview
}

function isToolUseDisplay(display: EventDisplay): boolean {
  return (
    display.event.kind === 'tool_use' ||
    display.event.preview.match(TOOL_EVENT_RE)?.[1] === 'tool_use'
  )
}

function isToolResultDisplay(display: EventDisplay): boolean {
  return (
    display.event.kind === 'tool_result' ||
    display.event.preview.match(TOOL_EVENT_RE)?.[1] === 'tool_result' ||
    parseLegacyToolResult(display.event.preview) !== null
  )
}

function toolCallLabel(toolName: string | null, result: LegacyToolResult | null): string {
  if (result) {
    const status = legacyResultStatus(result)
    return [toolName ?? 'Tool', status].filter(Boolean).join(' · ')
  }
  return toolName ? `${toolName} · completed` : 'Tool · completed'
}

function buildEventDisplays(events: WorkflowAgentEvent[]): EventDisplay[] {
  let previousToolName: string | null = null
  const pendingToolUsesById = new Map<string, PendingToolUse>()
  const pendingToolUseQueue: PendingToolUse[] = []
  const displays: EventDisplay[] = []

  const forgetPendingToolUse = (pending: PendingToolUse) => {
    if (pending.display.event.toolUseId) {
      pendingToolUsesById.delete(pending.display.event.toolUseId)
    }
    const index = pendingToolUseQueue.indexOf(pending)
    if (index >= 0) {
      pendingToolUseQueue.splice(index, 1)
    }
  }

  for (let index = 0; index < events.length; index += 1) {
    const event = events[index]
    const legacyResult = parseLegacyToolResult(event.preview)
    let toolNames = eventToolNames(event)

    if (legacyResult && toolNames.length === 0) {
      const inferredName = legacyResultToolName(legacyResult, previousToolName)
      if (inferredName) {
        toolNames = [inferredName]
      }
    }

    if (event.kind === 'tool_use' && toolNames[0]) {
      previousToolName = toolNames[0]
    }

    const display = {
      event,
      toolNames,
      label: eventLabel(event, toolNames, legacyResult),
      body: readableEventBody(event, toolNames, legacyResult),
    }

    if (isToolUseDisplay(display)) {
      const pending = { display, index: displays.length }
      if (event.toolUseId) {
        pendingToolUsesById.set(event.toolUseId, pending)
      }
      pendingToolUseQueue.push(pending)
      displays.push(display)
      continue
    }

    if (!isToolResultDisplay(display)) {
      displays.push(display)
      continue
    }

    const resultToolUseId = event.toolUseId ?? legacyResult?.toolUseId ?? null
    const hasIdBearingPendingToolUse = pendingToolUseQueue.some(
      (pending) => pending.display.event.toolUseId,
    )
    const pendingToolUse =
      (resultToolUseId ? pendingToolUsesById.get(resultToolUseId) : undefined) ??
      (hasIdBearingPendingToolUse ? undefined : pendingToolUseQueue[0])

    if (!pendingToolUse) {
      displays.push(display)
      continue
    }

    forgetPendingToolUse(pendingToolUse)
    const resultToolName = display.toolNames[0] ?? pendingToolUse.display.toolNames[0] ?? null
    const resultToolNames =
      display.toolNames.length > 0 ? display.toolNames : pendingToolUse.display.toolNames

    displays[pendingToolUse.index] = {
      ...display,
      input: pendingToolUse.display.body,
      label: toolCallLabel(resultToolName, legacyResult),
      output: display.body,
      toolNames: resultToolNames,
    } satisfies ToolCallDisplay
  }

  return displays
}

function isPairedToolCall(display: EventDisplay): display is ToolCallDisplay {
  return 'input' in display && 'output' in display
}

function EventCard({ display }: { display: EventDisplay }) {
  const { event, toolNames, label, body } = display
  const pairedToolCall = isPairedToolCall(display)

  return (
    <div
      className="rounded-md border border-gray-200 p-3 dark:border-gray-800"
      data-testid="workflow-event-card"
    >
      <div className="mb-1 flex items-center justify-between gap-2 text-xs text-gray-500">
        <span className="min-w-0 truncate">{label || 'event'}</span>
        <span>{event.timestamp ? formatDate(event.timestamp) : ''}</span>
      </div>
      {toolNames.length > 0 && (
        <div className="mb-2 flex flex-wrap gap-1">
          {toolNames.map((toolName) => (
            <span
              className={cn(
                'inline-flex max-w-full items-center gap-1 rounded border px-1.5 py-0.5 text-[11px] font-medium',
                'border-blue-200 bg-blue-50 text-blue-700 dark:border-blue-900/60 dark:bg-blue-950/40 dark:text-blue-300',
              )}
              key={toolName}
            >
              <Wrench className="h-3 w-3 shrink-0" />
              <span className="truncate">{toolName}</span>
            </span>
          ))}
        </div>
      )}
      {pairedToolCall ? (
        <div className="grid gap-2 text-xs">
          <div>
            <div className="mb-1 font-medium uppercase tracking-wide text-gray-500">Input</div>
            <pre className="max-h-28 overflow-auto whitespace-pre-wrap break-words rounded bg-gray-50 p-2 text-gray-700 dark:bg-gray-900 dark:text-gray-300">
              {display.input}
            </pre>
          </div>
          <div>
            <div className="mb-1 font-medium uppercase tracking-wide text-gray-500">Output</div>
            <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-words rounded bg-gray-50 p-2 text-gray-700 dark:bg-gray-900 dark:text-gray-300">
              {display.output ?? 'No output recorded yet.'}
            </pre>
          </div>
        </div>
      ) : (
        <div className="whitespace-pre-wrap break-words text-xs text-gray-700 dark:text-gray-300">
          {body}
        </div>
      )}
    </div>
  )
}
function RunMetadata({ detail }: { detail: WorkflowRunDetail }) {
  const run = detail.summary
  return (
    <section className="rounded-lg border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950">
      <div className="flex items-center gap-2 border-b border-gray-200 px-4 py-3 dark:border-gray-800">
        <Clock3 className="h-4 w-4 text-gray-500" />
        <h2 className="text-sm font-semibold text-gray-950 dark:text-white">Run metadata</h2>
      </div>
      <dl className="grid grid-cols-[120px_1fr] gap-x-3 gap-y-2 p-4 text-xs">
        <dt className="text-gray-500">Started</dt>
        <dd className="text-gray-800 dark:text-gray-200">{formatDate(run.startTime)}</dd>
        <dt className="text-gray-500">Updated</dt>
        <dd className="text-gray-800 dark:text-gray-200">{formatDate(run.updatedAt)}</dd>
        <dt className="text-gray-500">Model</dt>
        <dd className="break-words text-gray-800 dark:text-gray-200">
          {run.defaultModel ?? 'Unknown'}
        </dd>
        <dt className="text-gray-500">Artifact</dt>
        <dd className="break-words text-gray-800 dark:text-gray-200">
          {detail.artifactRelativePath ?? 'Unknown'}
        </dd>
      </dl>
    </section>
  )
}

function AgentDetail({ agentDetail }: { agentDetail: WorkflowAgentDetail }) {
  const events = agentDetail.events
  const eventDisplays = buildEventDisplays(events)
  const lastToolName = agentDetail.summary.lastToolName
  const lastToolSummary = agentDetail.summary.lastToolSummary
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 p-4">
      <div className="shrink-0 rounded-md border border-gray-200 p-3 dark:border-gray-800">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="truncate text-sm font-medium text-gray-950 dark:text-white">
              {agentDetail.summary.label ?? agentDetail.summary.agentId}
            </div>
            <div className="mt-1 truncate text-xs text-gray-500">
              {agentDetail.summary.model ?? 'No model recorded'}
            </div>
          </div>
          <div className="shrink-0 rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-600 dark:bg-gray-900 dark:text-gray-300">
            {agentDetail.summary.state}
          </div>
        </div>
        {(lastToolName || lastToolSummary) && (
          <div className="mt-3 border-t border-gray-200 pt-3 dark:border-gray-800">
            <div className="mb-1 text-xs font-medium uppercase tracking-wide text-gray-500">
              Last tool
            </div>
            <div className="flex flex-wrap items-start gap-2 text-xs">
              {lastToolName && (
                <span
                  className={cn(
                    'inline-flex max-w-full items-center gap-1 rounded border px-1.5 py-0.5 font-medium',
                    'border-blue-200 bg-blue-50 text-blue-700 dark:border-blue-900/60 dark:bg-blue-950/40 dark:text-blue-300',
                  )}
                >
                  <Wrench className="h-3 w-3 shrink-0" />
                  <span className="truncate">{lastToolName}</span>
                </span>
              )}
              {lastToolSummary && (
                <span className="min-w-0 flex-1 break-words font-mono text-gray-700 dark:text-gray-300">
                  {lastToolSummary}
                </span>
              )}
            </div>
          </div>
        )}
      </div>
      <div className="grid shrink-0 gap-3 xl:grid-cols-2">
        <PreviewBlock label="Prompt" body={agentDetail.promptPreview ?? 'No prompt preview.'} />
        <PreviewBlock label="Result" body={agentDetail.resultPreview ?? 'No result preview.'} />
      </div>
      <div className="min-h-0 flex-1">
        <div className="mb-2 text-xs font-medium uppercase tracking-wide text-gray-500">Events</div>
        {events.length === 0 ? (
          <div className="text-xs text-gray-500">No bounded JSONL events available.</div>
        ) : events.length > EVENT_VIRTUALIZE_THRESHOLD ? (
          <Virtuoso
            className="h-full min-h-80"
            data={eventDisplays}
            computeItemKey={(index, display) => `${display.event.kind}-${index}`}
            itemContent={(_, display) => (
              <div className="pb-2">
                <EventCard display={display} />
              </div>
            )}
          />
        ) : (
          <div className="flex h-full min-h-80 flex-col gap-2 overflow-auto">
            {eventDisplays.map((display, index) => (
              <EventCard key={`${display.event.kind}-${index}`} display={display} />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

function PreviewBlock({ label, body }: { label: string; body: string }) {
  return (
    <div className="min-w-0 shrink-0">
      <div className="mb-1 text-xs font-medium uppercase tracking-wide text-gray-500">{label}</div>
      <pre className="max-h-28 overflow-auto whitespace-pre-wrap break-words rounded-md bg-gray-50 p-3 text-xs text-gray-700 dark:bg-gray-900 dark:text-gray-300">
        {body}
      </pre>
    </div>
  )
}

export function RunAgentDetailPanel({
  detail,
  agentDetail,
}: {
  detail: WorkflowRunDetail
  agentDetail: WorkflowAgentDetail | undefined
}) {
  return (
    <aside className="flex h-full min-h-[calc(100vh-156px)] min-w-0 flex-col gap-5">
      <RunMetadata detail={detail} />
      <section className="flex min-h-0 flex-1 flex-col rounded-lg border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950">
        <div className="flex items-center gap-2 border-b border-gray-200 px-4 py-3 dark:border-gray-800">
          <Bot className="h-4 w-4 text-gray-500" />
          <h2 className="text-sm font-semibold text-gray-950 dark:text-white">Agent detail</h2>
        </div>
        {agentDetail ? (
          <AgentDetail agentDetail={agentDetail} />
        ) : (
          <div className="p-4 text-sm text-gray-500">Select an agent with JSONL events.</div>
        )}
      </section>
    </aside>
  )
}
