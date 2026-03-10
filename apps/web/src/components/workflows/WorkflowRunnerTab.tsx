import { useNavigate } from 'react-router-dom'
import type { WorkflowDefinition } from '../../types/generated/WorkflowDefinition'
import type { StageAttempt, StageStatus } from './WorkflowStageColumn'
import { WorkflowStageColumn } from './WorkflowStageColumn'

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
  stageAttempts,
  elapsedSeconds,
  currentStageIndex,
}: WorkflowRunnerTabProps) {
  const navigate = useNavigate()
  if (!definition)
    return (
      <div className="flex items-center justify-center h-full text-sm text-gray-400 dark:text-gray-600">
        Generate a workflow to see the runner.
      </div>
    )

  const totalStages = definition.stages.length
  const pct = totalStages > 0 ? (currentStageIndex / totalStages) * 100 : 0
  const elapsed = `${Math.floor(elapsedSeconds / 60)}m ${elapsedSeconds % 60}s`

  return (
    <div className="flex flex-col h-full">
      <div className="flex-1 overflow-x-auto p-4">
        <div className="flex gap-4 h-full items-start">
          {definition.stages.map((stage, i) => {
            const status = stageStatuses.get(stage.name) ?? (i === 0 ? 'running' : 'locked')
            const attempts = stageAttempts.get(stage.name) ?? []
            return (
              <div key={stage.name} className="flex items-center gap-2">
                <WorkflowStageColumn
                  stage={stage}
                  status={status}
                  attempts={attempts}
                  onSessionClick={(sessionId) => navigate(`/sessions/${sessionId}`)}
                />
                {i < definition.stages.length - 1 && (
                  <div
                    className={`w-8 h-px transition-colors duration-500 ${status === 'passed' ? 'bg-green-500' : 'bg-gray-300 dark:bg-gray-700'}`}
                  />
                )}
              </div>
            )
          })}
        </div>
      </div>
      <div className="px-4 py-3 border-t border-gray-200 dark:border-gray-800 flex items-center gap-3">
        <div className="flex-1 h-1.5 rounded-full bg-gray-200 dark:bg-gray-800 overflow-hidden">
          <div
            className="h-full bg-green-500 transition-all duration-500"
            style={{ width: `${pct}%` }}
          />
        </div>
        <span className="text-xs text-gray-400 dark:text-gray-500 whitespace-nowrap">
          Stage {currentStageIndex}/{totalStages} &middot; {elapsed}
        </span>
      </div>
    </div>
  )
}
