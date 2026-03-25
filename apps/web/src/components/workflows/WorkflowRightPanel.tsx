import { ArrowLeft } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { cn } from '../../lib/utils'
import type { WorkflowTab } from '../../pages/WorkflowDetailPage'
import type { WorkflowDefinition } from '../../types/generated/WorkflowDefinition'
import type { WorkflowDetail } from '../../types/generated/WorkflowDetail'
import { WorkflowPreviewTab } from './WorkflowPreviewTab'
import { WorkflowRunnerTab } from './WorkflowRunnerTab'
import type { StageAttempt, StageStatus } from './WorkflowStageColumn'

interface WorkflowRightPanelProps {
  workflow: WorkflowDetail | null
  activeTab: WorkflowTab
  onTabChange: (tab: WorkflowTab) => void
  onRun: () => void
  definition: WorkflowDefinition | null
  stageStatuses: Map<string, StageStatus>
  stageAttempts: Map<string, StageAttempt[]>
  elapsedSeconds: number
  currentStageIndex: number
}

const TAB_LABELS: Record<WorkflowTab, string> = {
  preview: 'Preview',
  runner: 'Runner',
}

export function WorkflowRightPanel({
  workflow,
  activeTab,
  onTabChange,
  onRun,
  definition,
  stageStatuses,
  stageAttempts,
  elapsedSeconds,
  currentStageIndex,
}: WorkflowRightPanelProps) {
  const navigate = useNavigate()

  return (
    <div className="flex flex-col h-full w-full bg-[#F5F5F7] dark:bg-[#000000] overflow-hidden">
      {/* Header */}
      <div className="shrink-0 bg-[#F5F5F7]/80 dark:bg-[#1C1C1E]/80 backdrop-blur-sm border-b border-[#D1D1D6] dark:border-[#3A3A3C]">
        {/* Back + title */}
        <div className="flex items-center gap-3 px-5 pt-4 pb-3">
          <button
            type="button"
            onClick={() => navigate('/workflows')}
            className="p-1.5 -ml-1.5 rounded-lg text-[#6E6E73] dark:text-[#98989D]
                       hover:bg-black/[0.06] dark:hover:bg-white/[0.08]
                       transition-colors duration-150 cursor-pointer"
            aria-label="Back to workflows"
          >
            <ArrowLeft className="w-4 h-4" />
          </button>

          <div className="flex-1 min-w-0">
            <h2 className="text-base font-semibold text-[#1D1D1F] dark:text-white truncate leading-tight">
              {workflow?.definition.name ?? 'Workflow'}
            </h2>
            {workflow && (
              <p className="text-xs text-[#6E6E73] dark:text-[#98989D] truncate mt-0.5">
                {workflow.definition.author} · v{workflow.definition.version} ·{' '}
                {workflow.definition.category}
              </p>
            )}
          </div>
        </div>

        {/* Tab bar */}
        <div className="flex items-center px-5 gap-0">
          {(['preview', 'runner'] as const).map((tab) => (
            <button
              key={tab}
              type="button"
              onClick={() => onTabChange(tab)}
              className={cn(
                'relative px-1 mr-5 pb-2.5 pt-0.5 text-xs font-medium',
                'transition-colors duration-150 cursor-pointer',
                'after:absolute after:bottom-0 after:left-0 after:right-0 after:h-[2px] after:rounded-full after:transition-all after:duration-150',
                activeTab === tab
                  ? 'text-[#1D1D1F] dark:text-white after:bg-[#1D1D1F] dark:after:bg-white'
                  : 'text-[#6E6E73] dark:text-[#98989D] after:bg-transparent hover:text-[#1D1D1F] dark:hover:text-white',
              )}
            >
              {TAB_LABELS[tab]}
            </button>
          ))}
        </div>
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-hidden">
        {activeTab === 'preview' ? (
          <WorkflowPreviewTab
            definition={workflow?.definition ?? null}
            yaml={workflow?.yaml ?? ''}
            onGenerate={onRun}
            canGenerate={!!workflow}
          />
        ) : (
          <WorkflowRunnerTab
            definition={definition}
            stageStatuses={stageStatuses}
            stageAttempts={stageAttempts}
            elapsedSeconds={elapsedSeconds}
            currentStageIndex={currentStageIndex}
          />
        )}
      </div>
    </div>
  )
}
