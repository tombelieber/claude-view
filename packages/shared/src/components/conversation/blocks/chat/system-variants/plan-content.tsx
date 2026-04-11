import { FileText } from 'lucide-react'
import { Markdown } from '../../shared/Markdown'

interface Props {
  data: Record<string, unknown>
}

export function PlanContentCard({ data }: Props) {
  const content = (data.planContent as string) || ''
  return (
    <div className="px-3 py-2 rounded-lg bg-violet-50 dark:bg-violet-900/15 border border-violet-200 dark:border-violet-800/40">
      <div className="flex items-center gap-1.5 mb-1.5">
        <FileText className="w-3 h-3 text-violet-500 dark:text-violet-400" />
        <span className="text-xs font-medium uppercase tracking-wider text-violet-500 dark:text-violet-400">
          Plan
        </span>
      </div>
      <div className="text-xs">
        <Markdown content={content} />
      </div>
    </div>
  )
}
