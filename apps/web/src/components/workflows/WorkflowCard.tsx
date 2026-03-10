import { Eye, Play, Star, Trash2 } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { WorkflowSummary } from '../../types/generated/WorkflowSummary'

interface WorkflowCardProps {
  workflow: WorkflowSummary
  onRun: (id: string) => void
  onView: (id: string) => void
  onDelete?: (id: string) => void
}

export function WorkflowCard({ workflow, onRun, onView, onDelete }: WorkflowCardProps) {
  const isOfficial = workflow.source === 'official'
  return (
    <div
      className={cn(
        'relative rounded-lg border p-4 flex flex-col gap-3',
        'bg-white dark:bg-gray-900 transition-colors duration-150',
        isOfficial
          ? 'border-amber-300 dark:border-amber-700 hover:border-amber-400'
          : 'border-gray-200 dark:border-gray-700 hover:border-gray-400 dark:hover:border-gray-500',
      )}
    >
      {isOfficial && (
        <div className="absolute top-3 right-3 flex items-center gap-1 text-amber-600 dark:text-amber-400">
          <Star className="w-3 h-3 fill-current" />
          <span className="text-xs font-medium">Official</span>
        </div>
      )}
      <div className="flex flex-col gap-1 pr-16">
        <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">{workflow.name}</h3>
        <p className="text-xs text-gray-500 dark:text-gray-400 line-clamp-2">
          {workflow.description}
        </p>
      </div>
      <div className="flex items-center gap-2 text-xs text-gray-400 dark:text-gray-500">
        <span>
          {workflow.stageCount} stage{workflow.stageCount !== 1 ? 's' : ''}
        </span>
        <span>·</span>
        <span>{workflow.category}</span>
      </div>
      <div className="flex items-center gap-2 mt-auto">
        <button
          type="button"
          onClick={() => onRun(workflow.id)}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium
                     bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900
                     hover:bg-gray-700 dark:hover:bg-gray-300 transition-colors cursor-pointer"
        >
          <Play className="w-3 h-3" />
          Run
        </button>
        <button
          type="button"
          onClick={() => onView(workflow.id)}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium
                     border border-gray-200 dark:border-gray-700
                     text-gray-600 dark:text-gray-400
                     hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors cursor-pointer"
        >
          <Eye className="w-3 h-3" />
          View
        </button>
        {!isOfficial && onDelete && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation()
              onDelete(workflow.id)
            }}
            className="p-1.5 rounded-md text-gray-400 hover:text-red-500
                       hover:bg-red-50 dark:hover:bg-red-950 transition-colors cursor-pointer"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        )}
      </div>
    </div>
  )
}
