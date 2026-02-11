import type { CategoryBreakdown } from '../../types/generated/CategoryBreakdown'
import { Code2, FileText, Brain, HelpCircle } from 'lucide-react'

interface Props {
  breakdown: CategoryBreakdown
  onCategoryClick: (categoryId: string) => void
}

const CATEGORY_CONFIG = {
  codeWork: {
    id: 'code_work',
    label: 'Code Work',
    icon: Code2,
    color: 'text-blue-600 dark:text-blue-400',
    bgColor: 'bg-blue-100 dark:bg-blue-900/30',
  },
  supportWork: {
    id: 'support_work',
    label: 'Support Work',
    icon: FileText,
    color: 'text-green-600 dark:text-green-400',
    bgColor: 'bg-green-100 dark:bg-green-900/30',
  },
  thinkingWork: {
    id: 'thinking_work',
    label: 'Thinking Work',
    icon: Brain,
    color: 'text-purple-600 dark:text-purple-400',
    bgColor: 'bg-purple-100 dark:bg-purple-900/30',
  },
  uncategorized: {
    id: 'uncategorized',
    label: 'Uncategorized',
    icon: HelpCircle,
    color: 'text-gray-600 dark:text-gray-400',
    bgColor: 'bg-gray-100 dark:bg-gray-800',
  },
} as const

export function CategoryStatsSummary({ breakdown, onCategoryClick }: Props) {
  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
      {Object.entries(CATEGORY_CONFIG).map(([key, config]) => {
        const data = breakdown[key as keyof CategoryBreakdown]
        const Icon = config.icon
        const isClickable = config.id !== 'uncategorized' && data.count > 0

        return (
          <button
            key={key}
            onClick={() => isClickable && onCategoryClick(config.id)}
            disabled={!isClickable}
            className={`
              p-4 rounded-lg border border-gray-200 dark:border-gray-700
              ${isClickable ? 'hover:border-gray-300 dark:hover:border-gray-600 cursor-pointer' : 'cursor-default'}
              transition-colors text-left
            `}
          >
            <div className={`inline-flex p-2 rounded-lg ${config.bgColor} mb-3`}>
              <Icon className={`w-5 h-5 ${config.color}`} />
            </div>
            <div className="text-2xl font-bold text-gray-900 dark:text-gray-100">
              {data.percentage.toFixed(0)}%
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400">
              {config.label}
            </div>
            <div className="text-xs text-gray-400 dark:text-gray-500 mt-1">
              {data.count} session{data.count !== 1 ? 's' : ''}
            </div>
          </button>
        )
      })}
    </div>
  )
}
