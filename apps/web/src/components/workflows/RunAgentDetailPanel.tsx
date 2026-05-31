import { Bot, Clock3 } from 'lucide-react'
import { Virtuoso } from 'react-virtuoso'
import type { WorkflowAgentDetail } from '../../types/generated/WorkflowAgentDetail'
import type { WorkflowAgentEvent } from '../../types/generated/WorkflowAgentEvent'
import type { WorkflowRunDetail } from '../../types/generated/WorkflowRunDetail'
import { VIRTUALIZE_THRESHOLD, formatDate } from './run-detail-format'

function EventCard({ event }: { event: WorkflowAgentEvent }) {
  return (
    <div className="rounded-md border border-gray-200 p-3 dark:border-gray-800">
      <div className="mb-1 flex items-center justify-between gap-2 text-xs text-gray-500">
        <span>{event.role ?? event.kind}</span>
        <span>{event.timestamp ? formatDate(event.timestamp) : ''}</span>
      </div>
      <div className="whitespace-pre-wrap text-xs text-gray-700 dark:text-gray-300">
        {event.preview}
      </div>
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
  return (
    <div className="flex flex-col gap-4 p-4">
      <div>
        <div className="text-sm font-medium text-gray-950 dark:text-white">
          {agentDetail.summary.label ?? agentDetail.summary.agentId}
        </div>
        <div className="mt-1 text-xs text-gray-500">
          {agentDetail.summary.model ?? 'No model recorded'}
        </div>
      </div>
      <PreviewBlock label="Prompt" body={agentDetail.promptPreview ?? 'No prompt preview.'} />
      <PreviewBlock label="Result" body={agentDetail.resultPreview ?? 'No result preview.'} />
      <div>
        <div className="mb-2 text-xs font-medium uppercase tracking-wide text-gray-500">Events</div>
        {events.length === 0 ? (
          <div className="text-xs text-gray-500">No bounded JSONL events available.</div>
        ) : events.length > VIRTUALIZE_THRESHOLD ? (
          <Virtuoso
            style={{ height: 420 }}
            data={events}
            computeItemKey={(index, event) => `${event.kind}-${index}`}
            itemContent={(_, event) => (
              <div className="pb-2">
                <EventCard event={event} />
              </div>
            )}
          />
        ) : (
          <div className="flex max-h-[420px] flex-col gap-2 overflow-auto">
            {events.map((event, index) => (
              <EventCard key={`${event.kind}-${index}`} event={event} />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

function PreviewBlock({ label, body }: { label: string; body: string }) {
  return (
    <div>
      <div className="mb-1 text-xs font-medium uppercase tracking-wide text-gray-500">{label}</div>
      <pre className="max-h-40 overflow-auto whitespace-pre-wrap rounded-md bg-gray-50 p-3 text-xs text-gray-700 dark:bg-gray-900 dark:text-gray-300">
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
    <aside className="flex min-w-0 flex-col gap-5">
      <RunMetadata detail={detail} />
      <section className="rounded-lg border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950">
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
