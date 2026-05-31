import { Boxes, Search } from 'lucide-react'
import { useMemo, useState } from 'react'
import { Virtuoso } from 'react-virtuoso'
import {
  type DateFilter,
  RUN_ROW_GRID,
  WorkflowRunRow,
  formatCompact,
  matchesDateFilter,
} from '../components/workflows/WorkflowRunRow'
import { useWorkflowRuns } from '../hooks/use-workflows'
import { cn } from '../lib/utils'

export function WorkflowsPage() {
  const [query, setQuery] = useState('')
  const [statusFilter, setStatusFilter] = useState('all')
  const [projectFilter, setProjectFilter] = useState('all')
  const [modelFilter, setModelFilter] = useState('all')
  const [dateFilter, setDateFilter] = useState<DateFilter>('all')

  const { data: runResponse, isLoading: runsLoading, isError } = useWorkflowRuns()
  const runs = useMemo(() => runResponse?.runs ?? [], [runResponse])

  const projects = useMemo(
    () => Array.from(new Set(runs.map((run) => run.projectDir))).sort(),
    [runs],
  )
  const models = useMemo(
    () =>
      Array.from(
        new Set(runs.map((run) => run.defaultModel).filter((model): model is string => !!model)),
      ).sort(),
    [runs],
  )
  const statuses = useMemo(() => Array.from(new Set(runs.map((run) => run.status))).sort(), [runs])

  const filteredRuns = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    return runs.filter((run) => {
      if (statusFilter !== 'all' && run.status !== statusFilter) return false
      if (projectFilter !== 'all' && run.projectDir !== projectFilter) return false
      if (modelFilter !== 'all' && run.defaultModel !== modelFilter) return false
      if (!matchesDateFilter(run, dateFilter)) return false
      if (!normalized) return true
      return [
        run.workflowName,
        run.summary,
        run.runId,
        run.sessionId,
        run.projectDir,
        run.defaultModel,
      ]
        .filter(Boolean)
        .some((value) => String(value).toLowerCase().includes(normalized))
    })
  }, [runs, query, statusFilter, projectFilter, modelFilter, dateFilter])

  const totalTokens = useMemo(
    () => runs.reduce((sum, run) => sum + Number(run.totalTokens), 0),
    [runs],
  )
  const activeRuns = useMemo(() => runs.filter((run) => run.status === 'running').length, [runs])

  return (
    <div className="flex h-full flex-col overflow-hidden bg-gray-50 dark:bg-black">
      <div className="shrink-0 border-b border-gray-200 bg-white px-8 py-6 dark:border-gray-800 dark:bg-gray-950">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight text-gray-950 dark:text-white">
              Workflows
            </h1>
            <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
              Claude Code dynamic workflow runs from ~/.claude.
            </p>
          </div>
          <div className="grid grid-cols-3 gap-3 text-right">
            <Stat value={String(runs.length)} label="runs" />
            <Stat value={String(activeRuns)} label="active" />
            <Stat value={formatCompact(totalTokens)} label="tokens" />
          </div>
        </div>
      </div>

      <div className="shrink-0 px-8 pt-6">
        <div className="flex flex-wrap items-center gap-2">
          <label className="relative min-w-[260px] flex-1">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400" />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search runs, sessions, projects"
              className="h-9 w-full rounded-md border border-gray-200 bg-white pl-9 pr-3 text-sm outline-none focus:border-blue-400 focus:ring-2 focus:ring-blue-100 dark:border-gray-800 dark:bg-gray-950 dark:text-white dark:focus:ring-blue-950"
            />
          </label>
          <FilterSelect
            value={statusFilter}
            onChange={setStatusFilter}
            allLabel="All statuses"
            options={statuses}
          />
          <FilterSelect
            value={projectFilter}
            onChange={setProjectFilter}
            allLabel="All projects"
            options={projects}
            wide
          />
          <FilterSelect
            value={modelFilter}
            onChange={setModelFilter}
            allLabel="All models"
            options={models}
            wide
          />
          <select
            value={dateFilter}
            onChange={(event) => setDateFilter(event.target.value as DateFilter)}
            className="h-9 rounded-md border border-gray-200 bg-white px-3 text-sm dark:border-gray-800 dark:bg-gray-950 dark:text-white"
          >
            <option value="all">Any time</option>
            <option value="day">Last 24h</option>
            <option value="week">Last 7d</option>
            <option value="month">Last 30d</option>
          </select>
        </div>
      </div>

      <div className="flex min-h-0 flex-1 flex-col px-8 pb-6 pt-4">
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950">
          <div
            className={cn(
              RUN_ROW_GRID,
              'shrink-0 border-b border-gray-200 px-4 py-2 text-xs font-medium uppercase tracking-wide text-gray-500 dark:border-gray-800',
            )}
          >
            <div>Workflow</div>
            <div>Project</div>
            <div>Status</div>
            <div>Started</div>
            <div>Usage</div>
            <div />
          </div>
          {runsLoading ? (
            <div className="flex h-40 items-center justify-center text-sm text-gray-500">
              Loading workflow runs...
            </div>
          ) : isError ? (
            <div className="flex h-40 flex-col items-center justify-center gap-2 text-center text-sm text-gray-500">
              <Boxes className="h-6 w-6 text-gray-400" />
              Could not read workflow runs from ~/.claude.
            </div>
          ) : filteredRuns.length > 0 ? (
            <Virtuoso
              className="min-h-0 flex-1"
              data={filteredRuns}
              computeItemKey={(_, run) => `${run.sessionId}:${run.runId}`}
              itemContent={(_, run) => <WorkflowRunRow run={run} />}
            />
          ) : (
            <div className="flex flex-1 flex-col items-center justify-center gap-2 px-4 py-16 text-center">
              <Boxes className="h-6 w-6 text-gray-400" />
              <div className="text-sm font-medium text-gray-700 dark:text-gray-200">
                No workflow runs found
              </div>
              <div className="max-w-md text-xs text-gray-500">
                Runs appear here when Claude Code writes workflow artifacts under
                ~/.claude/projects.
              </div>
            </div>
          )}
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

function FilterSelect({
  value,
  onChange,
  allLabel,
  options,
  wide,
}: {
  value: string
  onChange: (value: string) => void
  allLabel: string
  options: string[]
  wide?: boolean
}) {
  return (
    <select
      value={value}
      onChange={(event) => onChange(event.target.value)}
      className={cn(
        'h-9 rounded-md border border-gray-200 bg-white px-3 text-sm dark:border-gray-800 dark:bg-gray-950 dark:text-white',
        wide && 'max-w-[220px]',
      )}
    >
      <option value="all">{allLabel}</option>
      {options.map((option) => (
        <option key={option} value={option}>
          {option}
        </option>
      ))}
    </select>
  )
}
