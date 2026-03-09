import { cn } from '../../lib/utils'
import type { WorkflowMode, WorkflowTab } from '../../pages/WorkflowDetailPage'
import type { WorkflowDetail } from '../../types/generated/WorkflowDetail'
import { WorkflowPreviewTab } from './WorkflowPreviewTab'

interface WorkflowRightPanelProps {
  workflow: WorkflowDetail | null
  generatedYaml: string
  activeTab: WorkflowTab
  onTabChange: (tab: WorkflowTab) => void
  mode: WorkflowMode
  onRun: () => void
}

export function WorkflowRightPanel({
  workflow,
  generatedYaml,
  activeTab,
  onTabChange,
  mode,
  onRun,
}: WorkflowRightPanelProps) {
  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-1 px-3 py-2 border-b border-gray-200 dark:border-gray-800">
        {(['preview', 'runner'] as const).map((tab) => (
          <button
            type="button"
            key={tab}
            onClick={() => onTabChange(tab)}
            className={cn(
              'px-3 py-1.5 rounded-md text-xs font-medium capitalize transition-colors cursor-pointer',
              activeTab === tab
                ? 'bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100'
                : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300',
            )}
          >
            {tab}
          </button>
        ))}
      </div>
      {activeTab === 'preview' ? (
        <WorkflowPreviewTab
          definition={workflow?.definition ?? null}
          yaml={generatedYaml || workflow?.yaml || ''}
          onGenerate={onRun}
          canGenerate={!!workflow && mode === 'design'}
        />
      ) : (
        <div className="flex-1 overflow-hidden p-4 text-sm text-gray-400 dark:text-gray-500">
          Runner tab (Phase 7)
        </div>
      )}
    </div>
  )
}
