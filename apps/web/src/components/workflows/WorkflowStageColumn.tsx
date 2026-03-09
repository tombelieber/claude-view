import { CheckCircle, Lock, XCircle } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { WorkflowStage } from '../../types/generated/WorkflowStage'

export type StageStatus = 'locked' | 'running' | 'passed' | 'failed'

export interface StageAttempt {
  sessionId: string
  status: 'running' | 'passed' | 'failed'
  attempt: number
}

interface WorkflowStageColumnProps {
  stage: WorkflowStage
  status: StageStatus
  attempts: StageAttempt[]
  onSessionClick: (sessionId: string) => void
}

export function WorkflowStageColumn({
  stage,
  status,
  attempts,
  onSessionClick,
}: WorkflowStageColumnProps) {
  const isLocked = status === 'locked'
  return (
    <div
      className={cn(
        'flex flex-col rounded-lg border min-w-[220px] max-w-[280px] transition-all duration-300',
        isLocked
          ? 'opacity-40 border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900/50'
          : 'border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-900',
      )}
    >
      <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-800">
        <div className="flex items-center justify-between">
          <span className="text-xs font-semibold text-gray-800 dark:text-gray-200">
            {stage.name}
          </span>
          {isLocked && <Lock className="w-3 h-3 text-gray-400" />}
          {status === 'passed' && <CheckCircle className="w-3 h-3 text-green-500" />}
          {status === 'failed' && <XCircle className="w-3 h-3 text-red-500" />}
        </div>
        {stage.skills.map((skill) => (
          <span
            key={skill}
            className="block text-xs text-gray-400 dark:text-gray-500 font-mono mt-0.5"
          >
            {skill}
          </span>
        ))}
        {stage.gate && (
          <div className="mt-1.5 flex items-center gap-1">
            <Lock className="w-2.5 h-2.5 text-gray-400" />
            <span className="text-xs text-gray-400 dark:text-gray-500 truncate">
              {stage.gate.condition}
            </span>
          </div>
        )}
      </div>
      <div className="flex flex-col gap-2 p-2 flex-1 min-h-[80px]">
        {attempts.map((attempt) => (
          <button
            type="button"
            key={attempt.sessionId}
            onClick={() => onSessionClick(attempt.sessionId)}
            className={cn(
              'text-left px-2 py-1.5 rounded border text-xs transition-colors cursor-pointer',
              attempt.status === 'running'
                ? 'border-blue-300 dark:border-blue-700 bg-blue-50 dark:bg-blue-950/30'
                : attempt.status === 'passed'
                  ? 'border-green-300 dark:border-green-800 bg-green-50 dark:bg-green-950/30'
                  : 'border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950/30',
            )}
          >
            <div className="flex items-center justify-between">
              <span className="text-gray-600 dark:text-gray-400">Attempt {attempt.attempt}</span>
              <span
                className={cn(
                  attempt.status === 'running' && 'text-blue-500',
                  attempt.status === 'passed' && 'text-green-500',
                  attempt.status === 'failed' && 'text-red-500',
                )}
              >
                {attempt.status === 'running'
                  ? '\u21bb'
                  : attempt.status === 'passed'
                    ? '\u2713'
                    : '\u2717'}
              </span>
            </div>
          </button>
        ))}
        {attempts.length === 0 && !isLocked && (
          <p className="text-xs text-gray-400 dark:text-gray-600 text-center py-2">Waiting...</p>
        )}
      </div>
    </div>
  )
}
