import { Plus } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { WorkflowCard } from '../components/workflows/WorkflowCard'
import { useDeleteWorkflow, useWorkflows } from '../hooks/use-workflows'

export function WorkflowsPage() {
  const { data: workflows = [], isLoading } = useWorkflows()
  const { mutate: deleteWorkflow } = useDeleteWorkflow()
  const navigate = useNavigate()

  if (isLoading) return <div className="p-6 text-sm text-gray-400">Loading workflows...</div>

  return (
    <div className="flex flex-col h-full overflow-auto p-6 gap-6">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Workflows</h1>
      </div>
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {workflows.map((wf) => (
          <WorkflowCard
            key={wf.id}
            workflow={wf}
            onRun={(id) => navigate(`/workflows/${id}`)}
            onView={(id) => navigate(`/workflows/${id}?mode=preview`)}
            onDelete={wf.source === 'user' ? (id) => deleteWorkflow(id) : undefined}
          />
        ))}
        <button
          type="button"
          onClick={() => navigate('/workflows/new')}
          className="rounded-lg border-2 border-dashed border-gray-200 dark:border-gray-700
                     p-6 flex flex-col items-center justify-center gap-2
                     text-gray-400 dark:text-gray-500 hover:border-gray-400
                     hover:text-gray-600 dark:hover:text-gray-300 transition-colors cursor-pointer"
        >
          <Plus className="w-5 h-5" />
          <span className="text-sm font-medium">New Workflow</span>
          <span className="text-xs">Start from chat</span>
        </button>
      </div>
    </div>
  )
}
