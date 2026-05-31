import { useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { RunActivity } from '../components/workflows/RunActivity'
import { RunAgentDetailPanel } from '../components/workflows/RunAgentDetailPanel'
import { RunDetailHeader } from '../components/workflows/RunDetailHeader'
import { useWorkflowAgent, useWorkflowRun } from '../hooks/use-workflows'

function CenteredMessage({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 bg-gray-50 text-sm text-gray-500 dark:bg-black">
      {children}
    </div>
  )
}

export function WorkflowRunDetailPage() {
  const { sessionId = '', runId = '' } = useParams<{ sessionId: string; runId: string }>()
  const { data: detail, isLoading, isError } = useWorkflowRun(sessionId, runId)
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null)
  const activeAgentId = selectedAgentId ?? detail?.agents[0]?.agentId ?? null
  const { data: agentDetail } = useWorkflowAgent(sessionId, runId, activeAgentId)

  if (isLoading) {
    return <CenteredMessage>Loading workflow run...</CenteredMessage>
  }

  if (isError || !detail) {
    return (
      <CenteredMessage>
        <div>{isError ? 'Could not load this workflow run.' : 'Workflow run not found.'}</div>
        <Link to="/workflows" className="text-blue-600 hover:underline dark:text-blue-400">
          Back to workflows
        </Link>
      </CenteredMessage>
    )
  }

  return (
    <div className="flex h-full flex-col overflow-auto bg-gray-50 dark:bg-black">
      <RunDetailHeader run={detail.summary} />
      <div className="grid gap-5 px-8 py-6 xl:grid-cols-[minmax(0,1fr)_380px]">
        <RunActivity
          detail={detail}
          activeAgentId={activeAgentId}
          onSelectAgent={setSelectedAgentId}
        />
        <RunAgentDetailPanel detail={detail} agentDetail={agentDetail} />
      </div>
    </div>
  )
}
