import type { WorkflowMode } from '../../pages/WorkflowDetailPage'

const MODE_LABELS: Record<WorkflowMode, string> = {
  design: 'Designing',
  control: 'Running',
  review: 'Review',
}

const MODE_PLACEHOLDERS: Record<WorkflowMode, string> = {
  design: 'Describe your workflow...',
  control: 'Pause, skip stage, abort...',
  review: 'Re-run, patch, or ask what failed',
}

interface WorkflowChatRailProps {
  workflowId: string | null
  mode: WorkflowMode
  onModeChange: (mode: WorkflowMode) => void
  onYamlUpdate: (yaml: string) => void
  onWorkflowGenerated: () => void
  runId: string | null
  autoMessage: string | null
  generatedYaml: string
}

export function WorkflowChatRail({ mode, workflowId }: WorkflowChatRailProps) {
  return (
    <div className="flex flex-col h-full">
      <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-800 flex items-center justify-between">
        <span className="text-xs text-gray-500 dark:text-gray-400">{MODE_LABELS[mode]}</span>
        <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
          {workflowId ?? 'New Workflow'}
        </span>
      </div>
      <div className="flex-1 overflow-y-auto p-3 text-xs text-gray-400 dark:text-gray-500">
        <p className="text-center py-8">
          {mode === 'design' ? 'Describe the workflow you want to create.' : ''}
        </p>
      </div>
      <div className="p-3 border-t border-gray-200 dark:border-gray-800">
        <textarea
          placeholder={MODE_PLACEHOLDERS[mode]}
          disabled={mode !== 'design'}
          rows={3}
          className="w-full resize-none rounded-md border border-gray-200 dark:border-gray-700
                     bg-transparent text-sm px-3 py-2
                     placeholder:text-gray-400 dark:placeholder:text-gray-600
                     focus:outline-none focus:ring-1 focus:ring-gray-400
                     disabled:opacity-50"
        />
      </div>
    </div>
  )
}
