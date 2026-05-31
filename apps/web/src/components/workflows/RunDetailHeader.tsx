import { ArrowLeft, ExternalLink } from 'lucide-react'
import { Link } from 'react-router-dom'
import type { WorkflowRunSummary } from '../../types/generated/WorkflowRunSummary'
import { formatDuration, formatNumber } from './run-detail-format'

export function RunDetailHeader({ run }: { run: WorkflowRunSummary }) {
  return (
    <div className="shrink-0 border-b border-gray-200 bg-white px-8 py-5 dark:border-gray-800 dark:bg-gray-950">
      <div className="mb-4">
        <Link
          to="/workflows"
          className="inline-flex items-center gap-2 text-sm text-gray-500 hover:text-gray-900 dark:text-gray-400 dark:hover:text-white"
        >
          <ArrowLeft className="h-4 w-4" />
          Workflows
        </Link>
      </div>
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h1 className="truncate text-2xl font-semibold tracking-tight text-gray-950 dark:text-white">
              {run.workflowName}
            </h1>
            <span className="rounded-md border border-gray-200 px-2 py-1 text-xs text-gray-600 dark:border-gray-800 dark:text-gray-300">
              {run.status}
            </span>
          </div>
          <p className="mt-2 max-w-4xl text-sm text-gray-500 dark:text-gray-400">
            {run.summary ?? run.runId}
          </p>
          <div className="mt-3 flex flex-wrap items-center gap-4 text-xs text-gray-500">
            <span>{run.projectDir}</span>
            <span>{run.sessionId}</span>
            <Link
              to={`/sessions/${encodeURIComponent(run.sessionId)}`}
              className="inline-flex items-center gap-1 text-blue-600 hover:underline dark:text-blue-400"
            >
              Parent session
              <ExternalLink className="h-3 w-3" />
            </Link>
          </div>
        </div>
        <div className="grid grid-cols-2 gap-3 text-right sm:grid-cols-4">
          <Stat value={formatNumber(Number(run.totalTokens))} label="tokens" />
          <Stat value={String(run.totalToolCalls)} label="tools" />
          <Stat value={String(run.agentCount)} label="agents" />
          <Stat value={formatDuration(run.durationMs)} label="elapsed" />
        </div>
      </div>
    </div>
  )
}

function Stat({ value, label }: { value: string; label: string }) {
  return (
    <div>
      <div className="text-lg font-semibold text-gray-950 dark:text-white">{value}</div>
      <div className="text-xs text-gray-500">{label}</div>
    </div>
  )
}
