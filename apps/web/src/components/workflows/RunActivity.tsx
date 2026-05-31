import { Bot, CheckCircle2, Circle, FileCode2, Gauge, MessageSquareText } from 'lucide-react'
import { useMemo } from 'react'
import { Virtuoso } from 'react-virtuoso'
import { cn } from '../../lib/utils'
import type { WorkflowAgentSummary } from '../../types/generated/WorkflowAgentSummary'
import type { WorkflowRunDetail } from '../../types/generated/WorkflowRunDetail'
import { VIRTUALIZE_THRESHOLD, formatNumber, isPhaseComplete } from './run-detail-format'
import { AGENT_ROW_GRID, WorkflowAgentRow } from './WorkflowAgentRow'

function PhasesSection({ detail }: { detail: WorkflowRunDetail }) {
  const sortedPhases = useMemo(
    () => [...detail.phases].sort((a, b) => a.index - b.index),
    [detail.phases],
  )
  return (
    <section className="rounded-lg border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950">
      <div className="flex items-center gap-2 border-b border-gray-200 px-4 py-3 dark:border-gray-800">
        <Gauge className="h-4 w-4 text-gray-500" />
        <h2 className="text-sm font-semibold text-gray-950 dark:text-white">Phases</h2>
      </div>
      <div className="grid gap-3 p-4 md:grid-cols-2">
        {sortedPhases.length > 0 ? (
          sortedPhases.map((phase) => {
            const complete = isPhaseComplete(phase.completedAgentCount, phase.agentCount)
            return (
              <div
                key={phase.index}
                className="rounded-lg border border-gray-200 p-4 dark:border-gray-800"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium text-gray-950 dark:text-white">
                      {phase.index + 1}. {phase.title}
                    </div>
                    {phase.detail && (
                      <p className="mt-1 line-clamp-2 text-xs text-gray-500">{phase.detail}</p>
                    )}
                  </div>
                  {complete ? (
                    <CheckCircle2 className="h-4 w-4 shrink-0 text-emerald-500" />
                  ) : (
                    <Circle className="h-4 w-4 shrink-0 text-gray-300 dark:text-gray-600" />
                  )}
                </div>
                <div className="mt-3 grid grid-cols-3 gap-2 text-xs text-gray-500">
                  <span>
                    {phase.completedAgentCount}/{phase.agentCount} agents
                  </span>
                  <span>{formatNumber(Number(phase.tokenCount))} tokens</span>
                  <span>{phase.toolCallCount} tools</span>
                </div>
              </div>
            )
          })
        ) : (
          <div className="text-sm text-gray-500">No phase data recorded.</div>
        )}
      </div>
    </section>
  )
}

function AgentsSection({
  detail,
  activeAgentId,
  onSelectAgent,
}: {
  detail: WorkflowRunDetail
  activeAgentId: string | null
  onSelectAgent: (agentId: string) => void
}) {
  const renderRow = (agent: WorkflowAgentSummary) => (
    <WorkflowAgentRow
      agent={agent}
      selected={activeAgentId === agent.agentId}
      onSelect={() => onSelectAgent(agent.agentId)}
    />
  )
  return (
    <section className="rounded-lg border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950">
      <div className="flex items-center gap-2 border-b border-gray-200 px-4 py-3 dark:border-gray-800">
        <Bot className="h-4 w-4 text-gray-500" />
        <h2 className="text-sm font-semibold text-gray-950 dark:text-white">Agents</h2>
        <span className="text-xs text-gray-500">{detail.agents.length}</span>
      </div>
      <div
        className={cn(
          AGENT_ROW_GRID,
          'border-b border-gray-200 px-4 py-2 text-xs font-medium uppercase tracking-wide text-gray-500 dark:border-gray-800',
        )}
      >
        <div>Agent</div>
        <div>Status</div>
        <div>Tokens</div>
        <div>Tools</div>
      </div>
      {detail.agents.length === 0 ? (
        <div className="p-6 text-sm text-gray-500">No workflow-scoped agents recorded.</div>
      ) : detail.agents.length > VIRTUALIZE_THRESHOLD ? (
        <Virtuoso
          style={{ height: 480 }}
          data={detail.agents}
          computeItemKey={(_, agent) => agent.agentId}
          itemContent={(_, agent) => renderRow(agent)}
        />
      ) : (
        detail.agents.map((agent) => <div key={agent.agentId}>{renderRow(agent)}</div>)
      )}
    </section>
  )
}

function TextSection({
  icon,
  title,
  body,
  maxHeight,
}: {
  icon: React.ReactNode
  title: string
  body: string
  maxHeight: string
}) {
  return (
    <section className="rounded-lg border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950">
      <div className="flex items-center gap-2 border-b border-gray-200 px-4 py-3 dark:border-gray-800">
        {icon}
        <h2 className="text-sm font-semibold text-gray-950 dark:text-white">{title}</h2>
      </div>
      <pre
        className={cn(
          'overflow-auto whitespace-pre-wrap p-4 text-xs leading-relaxed text-gray-700 dark:text-gray-300',
          maxHeight,
        )}
      >
        {body}
      </pre>
    </section>
  )
}

export function RunActivity({
  detail,
  activeAgentId,
  onSelectAgent,
}: {
  detail: WorkflowRunDetail
  activeAgentId: string | null
  onSelectAgent: (agentId: string) => void
}) {
  return (
    <div className="flex min-w-0 flex-col gap-5">
      <PhasesSection detail={detail} />
      <AgentsSection detail={detail} activeAgentId={activeAgentId} onSelectAgent={onSelectAgent} />
      <TextSection
        icon={<FileCode2 className="h-4 w-4 text-gray-500" />}
        title="Script"
        body={detail.script ?? 'No saved script text found for this run.'}
        maxHeight="max-h-[420px]"
      />
      <TextSection
        icon={<MessageSquareText className="h-4 w-4 text-gray-500" />}
        title="Result"
        body={detail.result ?? detail.summary.resultPreview ?? 'No result preview recorded.'}
        maxHeight="max-h-[360px]"
      />
    </div>
  )
}
