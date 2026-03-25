import { useTheme } from '../../hooks/use-theme'
import { cn } from '../../lib/utils'
import type { WorkflowDefinition } from '../../types/generated/WorkflowDefinition'
import { WorkflowDiagram } from './WorkflowDiagram'
import type { StageAttempt, StageStatus } from './WorkflowStageColumn'

type RunStatus = 'ready' | 'running' | 'complete' | 'failed'

function deriveRunStatus(
  def: WorkflowDefinition,
  stageStatuses: Map<string, StageStatus>,
): RunStatus {
  if (stageStatuses.size === 0) return 'ready'
  const statuses = def.stages.map((s) => stageStatuses.get(s.name) ?? 'locked')
  if (statuses.some((s) => s === 'failed')) return 'failed'
  if (statuses.some((s) => s === 'running')) return 'running'
  if (statuses.every((s) => s === 'passed')) return 'complete'
  return 'running'
}

const STATUS_CONFIG: Record<RunStatus, { label: string; dot: string; text: string; bar: string }> =
  {
    ready: {
      label: 'Ready',
      dot: 'bg-[#C7C7CC]',
      text: 'text-[#AEAEB2] dark:text-[#636366]',
      bar: 'bg-[#E5E5EA] dark:bg-[#3A3A3C]',
    },
    running: {
      label: 'Running',
      dot: 'bg-[#3B82F6] animate-pulse',
      text: 'text-[#3B82F6]',
      bar: 'bg-[#3B82F6]',
    },
    complete: {
      label: 'Complete',
      dot: 'bg-[#22C55E]',
      text: 'text-[#22C55E]',
      bar: 'bg-[#22C55E]',
    },
    failed: {
      label: 'Failed',
      dot: 'bg-[#EF4444]',
      text: 'text-[#EF4444]',
      bar: 'bg-[#EF4444]',
    },
  }

interface WorkflowRunnerTabProps {
  definition: WorkflowDefinition | null
  stageStatuses: Map<string, StageStatus>
  stageAttempts: Map<string, StageAttempt[]>
  elapsedSeconds: number
  currentStageIndex: number
}

export function WorkflowRunnerTab({
  definition,
  stageStatuses,
  elapsedSeconds,
  currentStageIndex,
}: WorkflowRunnerTabProps) {
  const { resolvedTheme } = useTheme()

  if (!definition) {
    return (
      <div className="flex items-center justify-center h-full bg-[#F5F5F7] dark:bg-[#000000]">
        <div className="text-center">
          <p className="text-base font-medium text-[#1D1D1F] dark:text-white mb-1">
            No workflow selected
          </p>
          <p className="text-xs text-[#6E6E73] dark:text-[#98989D]">
            Go back and click Run on a workflow.
          </p>
        </div>
      </div>
    )
  }

  const totalStages = definition.stages.length
  const pct = totalStages > 0 ? Math.round((currentStageIndex / totalStages) * 100) : 0
  const mins = Math.floor(elapsedSeconds / 60)
  const secs = elapsedSeconds % 60
  const elapsed = mins > 0 ? `${mins}m ${secs}s` : `${secs}s`
  const runStatus = deriveRunStatus(definition, stageStatuses)
  const cfg = STATUS_CONFIG[runStatus]

  return (
    <div className="flex flex-col h-full overflow-hidden bg-[#F5F5F7] dark:bg-[#000000]">
      {/* Status bar */}
      <div className="shrink-0 px-5 py-4 bg-white dark:bg-[#1C1C1E] border-b border-[#D1D1D6] dark:border-[#3A3A3C]">
        <div className="flex items-center justify-between mb-2.5">
          <div className="flex items-center gap-2">
            <div className={cn('w-2 h-2 rounded-full shrink-0', cfg.dot)} />
            <span className={cn('text-xs font-medium', cfg.text)}>{cfg.label}</span>
          </div>
          <div className="flex items-center gap-3 text-xs text-[#AEAEB2] dark:text-[#636366]">
            <span>
              {currentStageIndex} / {totalStages}
            </span>
            <span>{elapsed}</span>
            <span>{pct}%</span>
          </div>
        </div>
        <div className="h-1 rounded-full bg-[#E5E5EA] dark:bg-[#2C2C2E] overflow-hidden">
          <div
            className={cn('h-full rounded-full transition-all duration-700 ease-out', cfg.bar)}
            style={{ width: `${pct}%` }}
          />
        </div>
      </div>

      {/* Diagram — ReactFlow with status colors */}
      <div className="flex-1 overflow-hidden bg-white dark:bg-[#1C1C1E]">
        <WorkflowDiagram
          definition={definition}
          stageStatuses={stageStatuses}
          isDark={resolvedTheme === 'dark'}
        />
      </div>

      {/* Legend */}
      <div className="shrink-0 px-5 py-3 border-t border-[#D1D1D6] dark:border-[#3A3A3C] bg-white dark:bg-[#1C1C1E] flex items-center gap-5">
        {(
          [
            { label: 'Locked', color: 'bg-[#E5E5EA] dark:bg-[#3A3A3C]' },
            { label: 'Running', color: 'bg-[#3B82F6]' },
            { label: 'Passed', color: 'bg-[#22C55E]' },
            { label: 'Failed', color: 'bg-[#EF4444]' },
          ] as const
        ).map(({ label, color }) => (
          <div key={label} className="flex items-center gap-1.5">
            <div className={cn('w-2 h-2 rounded-sm shrink-0', color)} />
            <span className="text-xs text-[#AEAEB2] dark:text-[#636366]">{label}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
