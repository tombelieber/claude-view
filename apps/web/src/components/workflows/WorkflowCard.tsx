import { Play } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { WorkflowSummary } from '../../types/generated/WorkflowSummary'

interface WorkflowCardProps {
  workflow: WorkflowSummary
  onRun: (id: string) => void
  onView: (id: string) => void
  onDelete?: (id: string) => void
}

export function WorkflowCard({ workflow, onRun, onView }: WorkflowCardProps) {
  const isOfficial = workflow.source === 'official'

  return (
    <div
      className={cn(
        'group relative flex flex-col rounded-2xl p-5',
        // iOS glass card — translucent white with subtle inner highlight
        'bg-white/90 dark:bg-[#1C1C1E]/95 backdrop-blur-sm',
        'shadow-[0_1px_3px_rgba(0,0,0,0.06),0_4px_16px_rgba(0,0,0,0.06),inset_0_1px_0_rgba(255,255,255,0.8)] dark:shadow-[0_1px_3px_rgba(0,0,0,0.3),0_4px_16px_rgba(0,0,0,0.3),inset_0_1px_0_rgba(255,255,255,0.05)]',
        'hover:shadow-[0_2px_6px_rgba(0,0,0,0.08),0_8px_28px_rgba(0,0,0,0.10),inset_0_1px_0_rgba(255,255,255,0.9)] dark:hover:shadow-[0_2px_6px_rgba(0,0,0,0.4),0_8px_28px_rgba(0,0,0,0.4),inset_0_1px_0_rgba(255,255,255,0.07)]',
        'transition-all duration-200 ease-out hover:-translate-y-0.5',
        'border border-black/[0.06] dark:border-white/[0.08]',
      )}
    >
      {/* Category tag */}
      <div className="flex items-center justify-between mb-4">
        <span className="text-[11px] font-medium tracking-wide text-[#6E6E73] dark:text-[#98989D] uppercase">
          {workflow.category}
        </span>
        {isOfficial && (
          <span className="text-[10px] font-semibold tracking-wider text-[#22C55E] uppercase">
            Official
          </span>
        )}
      </div>

      {/* Title */}
      <h3 className="text-[15px] font-semibold text-[#1D1D1F] dark:text-white leading-snug mb-1.5">
        {workflow.name}
      </h3>

      {/* Description */}
      <p className="text-[13px] text-[#6E6E73] dark:text-[#98989D] leading-relaxed line-clamp-2 mb-5">
        {workflow.description}
      </p>

      {/* Stage pills */}
      <div className="flex items-center gap-1.5 mb-5">
        {Array.from({ length: Math.min(workflow.stageCount, 6) }, (_, i) => (
          <div
            key={`${workflow.id}-dot-${i}`}
            className={cn(
              'rounded-full transition-all duration-200',
              i === 0 ? 'w-4 h-1.5' : 'w-1.5 h-1.5',
              isOfficial
                ? 'bg-[#22C55E] opacity-60 group-hover:opacity-100'
                : 'bg-[#1D1D1F]/20 dark:bg-white/20 group-hover:bg-[#1D1D1F]/40 dark:group-hover:bg-white/40',
            )}
          />
        ))}
        {workflow.stageCount > 6 && (
          <span className="text-[11px] text-[#AEAEB2] dark:text-[#636366] ml-0.5">
            +{workflow.stageCount - 6}
          </span>
        )}
        <span className="ml-auto text-[12px] text-[#AEAEB2] dark:text-[#636366]">
          {workflow.stageCount} {workflow.stageCount === 1 ? 'stage' : 'stages'}
        </span>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2 mt-auto">
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation()
            onRun(workflow.id)
          }}
          className={cn(
            'flex items-center gap-1.5 px-4 py-1.5 rounded-full text-[13px] font-semibold',
            'transition-all duration-150 cursor-pointer',
            isOfficial
              ? 'bg-[#22C55E] text-white hover:bg-[#16A34A] active:scale-95'
              : 'bg-[#1D1D1F] dark:bg-white text-white dark:text-[#1D1D1F] hover:bg-[#3D3D3F] dark:hover:bg-[#E5E5EA] active:scale-95',
          )}
        >
          <Play className="w-3 h-3" fill="currentColor" />
          Run
        </button>
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation()
            onView(workflow.id)
          }}
          className="px-4 py-1.5 rounded-full text-[13px] font-medium
                     text-[#6E6E73] dark:text-[#98989D]
                     hover:bg-black/[0.06] dark:hover:bg-white/[0.08]
                     transition-all duration-150 cursor-pointer active:scale-95"
        >
          View
        </button>
      </div>
    </div>
  )
}
