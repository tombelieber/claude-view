import { useState } from 'react'
import { useParams, useSearchParams } from 'react-router-dom'
import { toast } from 'sonner'
import { WorkflowRightPanel } from '../components/workflows/WorkflowRightPanel'
import type { StageAttempt, StageStatus } from '../components/workflows/WorkflowStageColumn'
import { useWorkflow } from '../hooks/use-workflows'

export type WorkflowTab = 'preview' | 'runner'

export function WorkflowDetailPage() {
  const { id } = useParams<{ id: string }>()
  const isNew = id === 'new'
  const { data: workflow } = useWorkflow(isNew ? '' : (id ?? ''))

  const [searchParams] = useSearchParams()
  const initialTab: WorkflowTab = searchParams.get('tab') === 'runner' ? 'runner' : 'preview'

  const [activeTab, setActiveTab] = useState<WorkflowTab>(initialTab)

  // Runner state — deferred until execution engine is wired
  const stageStatuses: Map<string, StageStatus> = new Map()
  const stageAttempts: Map<string, StageAttempt[]> = new Map()

  function handleRun() {
    setActiveTab('runner')
    toast.info('Workflow execution coming soon', { description: 'Runner wiring is in progress.' })
  }

  return (
    <div className="h-full flex overflow-hidden">
      <WorkflowRightPanel
        workflow={workflow ?? null}
        activeTab={activeTab}
        onTabChange={setActiveTab}
        onRun={handleRun}
        definition={workflow?.definition ?? null}
        stageStatuses={stageStatuses}
        stageAttempts={stageAttempts}
        elapsedSeconds={0}
        currentStageIndex={0}
      />
    </div>
  )
}
