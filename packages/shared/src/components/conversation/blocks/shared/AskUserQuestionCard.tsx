import { CheckCircle2, Circle, HelpCircle, Send } from 'lucide-react'
import { useCallback, useState } from 'react'
import type { AskQuestion } from '../../../../types/sidecar-protocol'
import { InteractiveCardShell } from './InteractiveCardShell'

export interface AskUserQuestionCardProps {
  question: AskQuestion
  onAnswer?: (requestId: string, answers: Record<string, string>) => void
  answered?: boolean
  selectedAnswers?: Record<string, string>
}

export function AskUserQuestionCard({
  question,
  onAnswer,
  answered,
  selectedAnswers,
}: AskUserQuestionCardProps) {
  const questions = question.questions

  const [singleSelections, setSingleSelections] = useState<Record<number, number>>({})
  const [multiSelections, setMultiSelections] = useState<Record<number, Set<number>>>({})
  const [otherTexts, setOtherTexts] = useState<Record<number, string>>({})
  const [otherSelected, setOtherSelected] = useState<Record<number, boolean>>({})

  const handleSingleSelect = useCallback((qi: number, oi: number) => {
    setSingleSelections((prev) => ({ ...prev, [qi]: oi }))
    setOtherSelected((prev) => ({ ...prev, [qi]: false }))
  }, [])

  const handleMultiToggle = useCallback((qi: number, oi: number) => {
    setMultiSelections((prev) => {
      const existing = prev[qi] ?? new Set<number>()
      const next = new Set(existing)
      if (next.has(oi)) next.delete(oi)
      else next.add(oi)
      return { ...prev, [qi]: next }
    })
  }, [])

  const handleOtherToggle = useCallback((qi: number, isMultiple: boolean) => {
    if (isMultiple) {
      setOtherSelected((prev) => ({ ...prev, [qi]: !prev[qi] }))
    } else {
      setSingleSelections((prev) => {
        const next = { ...prev }
        delete next[qi]
        return next
      })
      setOtherSelected((prev) => ({ ...prev, [qi]: true }))
    }
  }, [])

  const handleOtherText = useCallback((qi: number, text: string) => {
    setOtherTexts((prev) => ({ ...prev, [qi]: text }))
  }, [])

  const requestId = question.requestId

  const handleSubmit = useCallback(() => {
    if (!onAnswer) return
    const answers: Record<string, string> = {}
    for (let qi = 0; qi < questions.length; qi++) {
      const q = questions[qi]
      const parts: string[] = []

      if (q.multiSelect) {
        const selected = multiSelections[qi]
        if (selected && q.options) {
          for (const oi of selected) {
            if (q.options[oi]) parts.push(q.options[oi].label)
          }
        }
      } else {
        const oi = singleSelections[qi]
        if (oi !== undefined && q.options?.[oi]) {
          parts.push(q.options[oi].label)
        }
      }

      if (otherSelected[qi] && otherTexts[qi]?.trim()) {
        parts.push(otherTexts[qi].trim())
      }

      answers[q.question] = parts.join(', ')
    }
    onAnswer(requestId, answers)
  }, [questions, multiSelections, singleSelections, otherSelected, otherTexts, onAnswer, requestId])

  const resolvedState = answered ? { label: 'Answered', variant: 'success' as const } : undefined

  return (
    <InteractiveCardShell
      variant="question"
      header="Question"
      icon={<HelpCircle className="w-4 h-4" />}
      resolved={resolvedState}
      actions={
        onAnswer ? (
          <button
            type="button"
            onClick={handleSubmit}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-white bg-amber-600 rounded-md hover:bg-amber-700 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/50"
          >
            <Send className="w-3 h-3" />
            Submit
          </button>
        ) : undefined
      }
    >
      <div className="space-y-3">
        {questions.map((q, qi) => (
          <div key={q.question}>
            {q.header && (
              <div className="text-xs font-mono text-amber-600 dark:text-amber-400 uppercase tracking-wide mb-0.5">
                {q.header}
              </div>
            )}
            <div className="text-xs text-gray-800 dark:text-gray-200 leading-relaxed mb-2">
              {q.question}
            </div>

            {q.options && q.options.length > 0 && (
              <div className="space-y-1.5">
                {q.options.map((opt, oi) => {
                  const isSelected = q.multiSelect
                    ? (multiSelections[qi]?.has(oi) ?? false)
                    : singleSelections[qi] === oi

                  return (
                    <button
                      key={opt.label}
                      type="button"
                      onClick={() =>
                        q.multiSelect ? handleMultiToggle(qi, oi) : handleSingleSelect(qi, oi)
                      }
                      className={`w-full flex items-start gap-2 px-2 py-1.5 rounded text-left border transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/50 ${
                        isSelected
                          ? 'bg-amber-50 dark:bg-amber-900/20 border-amber-300 dark:border-amber-500/40'
                          : 'bg-white/50 dark:bg-gray-800/30 border-gray-200/50 dark:border-gray-700/30 hover:border-amber-300/50 dark:hover:border-amber-500/20'
                      }`}
                    >
                      {q.multiSelect ? (
                        <CheckCircle2
                          className={`w-3.5 h-3.5 flex-shrink-0 mt-0.5 ${
                            isSelected
                              ? 'text-amber-500 dark:text-amber-400'
                              : 'text-gray-400 dark:text-gray-500'
                          }`}
                        />
                      ) : (
                        <Circle
                          className={`w-3.5 h-3.5 flex-shrink-0 mt-0.5 ${
                            isSelected
                              ? 'text-amber-500 dark:text-amber-400'
                              : 'text-gray-400 dark:text-gray-500'
                          }`}
                          {...(isSelected ? { fill: 'currentColor' } : {})}
                        />
                      )}
                      <div className="min-w-0 flex-1">
                        <div className="text-xs font-medium text-gray-700 dark:text-gray-300">
                          {opt.label}
                        </div>
                        {opt.description && (
                          <div className="text-xs text-gray-500 dark:text-gray-500 mt-0.5 leading-relaxed">
                            {opt.description}
                          </div>
                        )}
                      </div>
                    </button>
                  )
                })}

                <button
                  type="button"
                  onClick={() => handleOtherToggle(qi, !!q.multiSelect)}
                  className={`w-full flex items-start gap-2 px-2 py-1.5 rounded text-left border transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/50 ${
                    otherSelected[qi]
                      ? 'bg-amber-50 dark:bg-amber-900/20 border-amber-300 dark:border-amber-500/40'
                      : 'bg-white/50 dark:bg-gray-800/30 border-gray-200/50 dark:border-gray-700/30 hover:border-amber-300/50 dark:hover:border-amber-500/20'
                  }`}
                >
                  <Circle
                    className={`w-3.5 h-3.5 flex-shrink-0 mt-0.5 ${
                      otherSelected[qi]
                        ? 'text-amber-500 dark:text-amber-400'
                        : 'text-gray-400 dark:text-gray-500'
                    }`}
                    {...(otherSelected[qi] ? { fill: 'currentColor' } : {})}
                  />
                  <span className="text-xs font-medium text-gray-500 dark:text-gray-400 italic">
                    Other...
                  </span>
                </button>

                {otherSelected[qi] && (
                  <input
                    type="text"
                    value={otherTexts[qi] ?? ''}
                    onChange={(e) => handleOtherText(qi, e.target.value)}
                    placeholder="Type your answer..."
                    className="w-full text-xs px-2 py-1.5 rounded border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-gray-800 dark:text-gray-200 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-amber-500/50"
                  />
                )}

                <div className="text-xs text-gray-400 dark:text-gray-500 italic px-2 pt-1">
                  {q.multiSelect ? 'Multiple selections allowed' : 'Single selection only'}
                </div>
              </div>
            )}

            {answered && selectedAnswers?.[q.question] && (
              <div className="mt-1 text-xs text-green-600 dark:text-green-400 font-medium">
                Answer: {selectedAnswers[q.question]}
              </div>
            )}
          </div>
        ))}
      </div>
    </InteractiveCardShell>
  )
}
