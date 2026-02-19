import { MessageCircle, Keyboard } from 'lucide-react'

interface QuestionOption {
  label: string
  description: string
}

interface Question {
  question: string
  header: string
  options: QuestionOption[]
  multiSelect: boolean
}

interface QuestionCardProps {
  context?: Record<string, unknown>
}

export function QuestionCard({ context }: QuestionCardProps) {
  if (!context) return null

  const questions = context.questions as Question[] | undefined
  if (!questions || questions.length === 0) return null

  const q = questions[0]
  const displayOptions = q.options.slice(0, 4)
  const isMulti = questions.length > 1

  return (
    <div
      className="mb-2 rounded-md border-l-4 border-amber-400 dark:border-amber-500 bg-amber-50/50 dark:bg-amber-900/10 p-3"
      data-testid="question-card"
    >
      {/* Header */}
      <div className="flex items-center gap-1.5 mb-1.5">
        <MessageCircle className="h-3.5 w-3.5 text-amber-500 dark:text-amber-400" />
        <span className="text-[11px] font-semibold text-amber-600 dark:text-amber-400 uppercase tracking-wide">
          Question
        </span>
        {isMulti && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-200/60 dark:bg-amber-800/40 text-amber-700 dark:text-amber-300 font-medium">
            1 of {questions.length}
          </span>
        )}
      </div>

      {/* Question text */}
      <p className="text-sm font-medium text-gray-900 dark:text-gray-100 line-clamp-2 mb-2">
        {q.question}
      </p>

      {/* Options grid */}
      {displayOptions.length > 0 && (
        <div className="grid grid-cols-2 gap-1.5 mb-2">
          {displayOptions.map((opt) => (
            <span
              key={opt.label}
              className="inline-flex items-center gap-1 px-2 py-1 rounded text-xs bg-white/80 dark:bg-gray-800/60 text-gray-600 dark:text-gray-400 border border-gray-200 dark:border-gray-700"
              title={opt.description}
            >
              <span className="w-2 h-2 rounded-full border border-gray-300 dark:border-gray-600 flex-shrink-0" />
              <span className="truncate">{opt.label}</span>
            </span>
          ))}
        </div>
      )}

      {/* Footer */}
      <div className="flex items-center gap-1 text-[10px] text-gray-400 dark:text-gray-500">
        <Keyboard className="h-2.5 w-2.5" />
        <span>Answer in terminal</span>
      </div>
    </div>
  )
}
