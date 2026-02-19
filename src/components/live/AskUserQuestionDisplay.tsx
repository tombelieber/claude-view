import { HelpCircle, MessageCircle, CheckCircle2, Circle } from 'lucide-react'

interface QuestionOption {
  label: string
  description?: string
}

interface QuestionItem {
  question: string
  header?: string
  options?: QuestionOption[]
  multiple?: boolean
}

interface AskUserQuestionInput {
  questions: QuestionItem[]
}

function isAskUserQuestionInput(data: unknown): data is AskUserQuestionInput {
  if (!data || typeof data !== 'object') return false
  const d = data as Record<string, unknown>
  return Array.isArray(d.questions)
}

const COLORS = {
  purple: {
    border: 'border-purple-200/50 dark:border-purple-500/20',
    bg: 'bg-purple-50/30 dark:bg-purple-900/10',
    headerBorder: 'border-purple-200/30 dark:border-purple-500/10',
    headerBg: 'bg-purple-100/50 dark:bg-purple-900/20',
    icon: 'text-purple-500 dark:text-purple-400',
    headerText: 'text-purple-600 dark:text-purple-400',
  },
  amber: {
    border: 'border-amber-200/50 dark:border-amber-500/20',
    bg: 'bg-amber-50/30 dark:bg-amber-900/10',
    headerBorder: 'border-amber-200/30 dark:border-amber-500/10',
    headerBg: 'bg-amber-100/50 dark:bg-amber-900/20',
    icon: 'text-amber-500 dark:text-amber-400',
    headerText: 'text-amber-600 dark:text-amber-400',
  },
} as const

export function AskUserQuestionDisplay({ inputData, variant = 'purple' }: { inputData: unknown; variant?: 'purple' | 'amber' }) {
  if (!isAskUserQuestionInput(inputData)) return null

  const { questions } = inputData
  const c = COLORS[variant]
  const Icon = variant === 'amber' ? MessageCircle : HelpCircle

  return (
    <div className="mt-2 space-y-3">
      {questions.map((q, qi) => (
        <div
          key={qi}
          className={`rounded-lg border ${c.border} ${c.bg} overflow-hidden`}
        >
          <div className={`px-3 py-2 border-b ${c.headerBorder} ${c.headerBg}`}>
            <div className="flex items-start gap-2">
              <Icon className={`w-4 h-4 ${c.icon} flex-shrink-0 mt-0.5`} />
              <div className="min-w-0 flex-1">
                {q.header && (
                  <div className={`text-[10px] font-mono ${c.headerText} uppercase tracking-wide mb-0.5`}>
                    {q.header}
                  </div>
                )}
                <div className="text-xs text-gray-800 dark:text-gray-200 leading-relaxed">
                  {q.question}
                </div>
              </div>
            </div>
          </div>

          {q.options && q.options.length > 0 && (
            <div className="p-2 space-y-1.5">
              {q.options.map((opt, oi) => (
                <div
                  key={oi}
                  className="flex items-start gap-2 px-2 py-1.5 rounded bg-white/50 dark:bg-gray-800/30 border border-gray-200/50 dark:border-gray-700/30"
                >
                  {q.multiple ? (
                    <CheckCircle2 className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 flex-shrink-0 mt-0.5" />
                  ) : (
                    <Circle className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 flex-shrink-0 mt-0.5" />
                  )}
                  <div className="min-w-0 flex-1">
                    <div className="text-[11px] font-medium text-gray-700 dark:text-gray-300">
                      {opt.label}
                    </div>
                    {opt.description && (
                      <div className="text-[10px] text-gray-500 dark:text-gray-500 mt-0.5 leading-relaxed">
                        {opt.description}
                      </div>
                    )}
                  </div>
                </div>
              ))}
              <div className="text-[9px] text-gray-400 dark:text-gray-500 italic px-2 pt-1">
                {q.multiple ? 'Multiple selections allowed' : 'Single selection only'}
              </div>
            </div>
          )}
        </div>
      ))}
    </div>
  )
}

export { isAskUserQuestionInput }
