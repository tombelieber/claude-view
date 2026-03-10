import { useCallback, useEffect, useRef, useState } from 'react'
import { useParams } from 'react-router-dom'
import { WorkflowChatRail } from '../components/workflows/WorkflowChatRail'
import { WorkflowRightPanel } from '../components/workflows/WorkflowRightPanel'
import type { StageAttempt, StageStatus } from '../components/workflows/WorkflowStageColumn'
import { useWorkflow } from '../hooks/use-workflows'

const MIN_RAIL_PCT = 20
const MAX_RAIL_PCT = 40
const DEFAULT_RAIL_PCT = 28

export type WorkflowMode = 'design' | 'control' | 'review'
export type WorkflowTab = 'preview' | 'runner'

export function WorkflowDetailPage() {
  const { id } = useParams<{ id: string }>()
  const isNew = id === 'new'
  const { data: workflow } = useWorkflow(isNew ? '' : (id ?? ''))

  const [railPct, setRailPct] = useState(DEFAULT_RAIL_PCT)
  const [mode, setMode] = useState<WorkflowMode>('design')
  const [activeTab, setActiveTab] = useState<WorkflowTab>('preview')
  const [generatedYaml, setGeneratedYaml] = useState<string>('')
  const [stageStatuses, setStageStatuses] = useState<Map<string, StageStatus>>(new Map())
  const [stageAttempts, setStageAttempts] = useState<Map<string, StageAttempt[]>>(new Map())
  const [elapsedSeconds, setElapsedSeconds] = useState(0)
  const [currentStageIndex, setCurrentStageIndex] = useState(0)
  const [runId, setRunId] = useState<string | null>(null)
  const [autoMessage, setAutoMessage] = useState<string | null>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  // Suppress unused-variable warnings — these setters will be wired in the run-engine task
  void setStageStatuses
  void setStageAttempts
  void setElapsedSeconds
  void setCurrentStageIndex

  // Cmd+B: toggle rail
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'b') {
        e.preventDefault()
        setRailPct((p) => (p > 0 ? 0 : DEFAULT_RAIL_PCT))
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  const handleDragStart = useCallback((e: React.PointerEvent) => {
    e.preventDefault()
    const onMove = (ev: PointerEvent) => {
      if (!containerRef.current) return
      const rect = containerRef.current.getBoundingClientRect()
      const pct = ((ev.clientX - rect.left) / rect.width) * 100
      setRailPct(Math.round(Math.max(MIN_RAIL_PCT, Math.min(MAX_RAIL_PCT, pct))))
    }
    const onUp = () => {
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
    }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
  }, [])

  const handleRun = useCallback(() => {
    setMode('control')
    setRunId(crypto.randomUUID()) // Placeholder — Task 15 returns real run ID
    setTimeout(() => setActiveTab('runner'), 350)
  }, [])

  const handleRunComplete = useCallback((summary: string) => {
    setMode('review')
    setAutoMessage(summary)
  }, [])

  // Suppress unused-variable warning — wired in run-engine task
  void handleRunComplete

  return (
    <div ref={containerRef} className="h-full flex overflow-hidden">
      <div
        className="flex flex-col border-r border-gray-200 dark:border-gray-800 overflow-hidden transition-all duration-200"
        style={{ width: `${railPct}%` }}
      >
        <WorkflowChatRail
          workflowId={isNew ? null : (id ?? null)}
          mode={mode}
          onModeChange={setMode}
          onYamlUpdate={setGeneratedYaml}
          onWorkflowGenerated={handleRun}
          runId={runId}
          autoMessage={autoMessage}
          generatedYaml={generatedYaml}
        />
      </div>

      <div
        onPointerDown={handleDragStart}
        className="w-1 cursor-col-resize flex-shrink-0 bg-transparent hover:bg-blue-500/30 transition-colors duration-150"
      />

      <div className="flex-1 overflow-hidden">
        <WorkflowRightPanel
          workflow={workflow ?? null}
          generatedYaml={generatedYaml}
          activeTab={activeTab}
          onTabChange={setActiveTab}
          mode={mode}
          onRun={handleRun}
          definition={workflow?.definition ?? null}
          stageStatuses={stageStatuses}
          stageAttempts={stageAttempts}
          elapsedSeconds={elapsedSeconds}
          currentStageIndex={currentStageIndex}
        />
      </div>
    </div>
  )
}
