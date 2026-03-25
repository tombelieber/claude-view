import { useNavigate } from 'react-router-dom'
import { WorkflowCard } from '../components/workflows/WorkflowCard'
import { useDeleteWorkflow, useWorkflows } from '../hooks/use-workflows'

export function WorkflowsPage() {
  const { data: workflows = [], isLoading } = useWorkflows()
  const { mutate: deleteWorkflow } = useDeleteWorkflow()
  const navigate = useNavigate()

  const official = workflows.filter((w) => w.source === 'official')
  const user = workflows.filter((w) => w.source === 'user')

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full bg-[#F5F5F7] dark:bg-[#000000]">
        <div className="flex flex-col items-center gap-3">
          <div className="w-6 h-6 rounded-full border-2 border-[#1D1D1F]/20 dark:border-white/20 border-t-[#1D1D1F] dark:border-t-white animate-spin" />
          <span className="text-xs text-[#6E6E73] dark:text-[#98989D]">Loading…</span>
        </div>
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full overflow-auto bg-[#F5F5F7] dark:bg-[#000000]">
      {/* WIP banner */}
      <div className="mx-8 mt-6 px-4 py-3 rounded-xl bg-[#FFF7ED] dark:bg-[#2C1A00] border border-[#FED7AA] dark:border-[#92400E] flex items-center gap-3">
        <span className="text-[#EA580C] dark:text-[#FB923C] shrink-0">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path
              d="M8 1.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Zm8-3.5a.75.75 0 0 1 .75.75v3.5a.75.75 0 0 1-1.5 0v-3.5A.75.75 0 0 1 8 4.5ZM8 11a1 1 0 1 1 0 2 1 1 0 0 1 0-2Z"
              fill="currentColor"
            />
          </svg>
        </span>
        <p className="text-xs text-[#9A3412] dark:text-[#FB923C] leading-snug">
          <span className="font-semibold">Work in progress.</span> Workflow execution is not yet
          available — you can browse and preview workflows, but running them requires the execution
          engine (coming in a future release).
        </p>
      </div>

      {/* Page header */}
      <div className="px-8 pt-6 pb-6">
        <h1 className="text-3xl font-bold text-[#1D1D1F] dark:text-white tracking-tight">
          Workflows
        </h1>
        <p className="mt-1 text-base text-[#6E6E73] dark:text-[#98989D]">
          Automated pipelines for your Claude sessions.
        </p>
      </div>

      <div className="flex flex-col gap-10 px-8 pb-10">
        {/* Official section */}
        {official.length > 0 && (
          <section>
            <div className="flex items-baseline gap-2 mb-4">
              <h2 className="text-xs font-semibold text-[#1D1D1F] dark:text-white tracking-wide uppercase">
                Official
              </h2>
              <span className="text-xs text-[#AEAEB2] dark:text-[#636366]">{official.length}</span>
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
              {official.map((wf) => (
                <WorkflowCard
                  key={wf.id}
                  workflow={wf}
                  onRun={(id) => navigate(`/workflows/${id}?tab=runner`)}
                  onView={(id) => navigate(`/workflows/${id}`)}
                />
              ))}
            </div>
          </section>
        )}

        {/* User section */}
        {user.length > 0 && (
          <section>
            <div className="flex items-baseline gap-2 mb-4">
              <h2 className="text-xs font-semibold text-[#1D1D1F] dark:text-white tracking-wide uppercase">
                Custom
              </h2>
              <span className="text-xs text-[#AEAEB2] dark:text-[#636366]">{user.length}</span>
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
              {user.map((wf) => (
                <WorkflowCard
                  key={wf.id}
                  workflow={wf}
                  onRun={(id) => navigate(`/workflows/${id}?tab=runner`)}
                  onView={(id) => navigate(`/workflows/${id}`)}
                  onDelete={(id) => deleteWorkflow(id)}
                />
              ))}
            </div>
          </section>
        )}

        {/* Coming soon — only if no custom workflows yet */}
        {user.length === 0 && (
          <section>
            <div className="flex items-baseline gap-2 mb-4">
              <h2 className="text-xs font-semibold text-[#AEAEB2] dark:text-[#636366] tracking-wide uppercase">
                Custom
              </h2>
            </div>
            <div className="rounded-2xl border border-dashed border-[#D1D1D6] dark:border-[#3A3A3C] p-8 flex flex-col items-center gap-2 text-center">
              <p className="text-sm font-medium text-[#AEAEB2] dark:text-[#636366]">
                Custom Workflows
              </p>
              <p className="text-xs text-[#C7C7CC] dark:text-[#48484A]">Coming soon</p>
            </div>
          </section>
        )}
      </div>
    </div>
  )
}
