import { CheckCircle2, Circle, HelpCircle, Send } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { isAskUserQuestionInput } from '../../live/AskUserQuestionDisplay'
import { InteractiveCardShell } from './InteractiveCardShell'

export interface AskUserQuestionCardProps {
  inputData: unknown
  requestId: string
  onAnswer: (requestId: string, answers: Record<string, string>) => void
  answered?: boolean
  selectedAnswers?: Record<string, string>
}

export function AskUserQuestionCard({
  inputData,
  requestId,
  onAnswer,
  answered,
  selectedAnswers,
}: AskUserQuestionCardProps) {
  // Validate the input shape
  const parsed = useMemo(() => {
    if (!isAskUserQuestionInput(inputData)) return null
    return inputData
  }, [inputData])

  // Track single-select per question: questionIndex -> optionIndex
  const [singleSelections, setSingleSelections] = useState<Record<number, number>>({})
  // Track multi-select per question: questionIndex -> Set of optionIndices
  const [multiSelections, setMultiSelections] = useState<Record<number, Set<number>>>({})
  // Track "Other" text per question
  const [otherTexts, setOtherTexts] = useState<Record<number, string>>({})
  // Track whether "Other" is selected per question
  const [otherSelected, setOtherSelected] = useState<Record<number, boolean>>({})

  const handleSingleSelect = useCallback((qi: number, oi: number) => {
    setSingleSelections((prev) => ({ ...prev, [qi]: oi }))
    // Deselect "Other" when picking a regular option
    setOtherSelected((prev) => ({ ...prev, [qi]: false }))
  }, [])

  const handleMultiToggle = useCallback((qi: number, oi: number) => {
    setMultiSelections((prev) => {
      const existing = prev[qi] ?? new Set<number>()
      const next = new Set(existing)
      if (next.has(oi)) {
        next.delete(oi)
      } else {
        next.add(oi)
      }
      return { ...prev, [qi]: next }
    })
  }, [])

  const handleOtherToggle = useCallback((qi: number, isMultiple: boolean) => {
    if (isMultiple) {
      setOtherSelected((prev) => ({ ...prev, [qi]: !prev[qi] }))
    } else {
      // Single-select: deselect any option, select "Other"
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

  const handleSubmit = useCallback(() => {
    if (!parsed) return

    const answers: Record<string, string> = {}
    for (let qi = 0; qi < parsed.questions.length; qi++) {
      const q = parsed.questions[qi]
      const parts: string[] = []

      if (q.multiple) {
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
  }, [parsed, multiSelections, singleSelections, otherSelected, otherTexts, onAnswer, requestId])

  if (!parsed) return null

  const resolvedState = answered ? { label: 'Answered', variant: 'success' as const } : undefined

  return (
    <InteractiveCardShell
      variant="question"
      header="Question"
      icon={<HelpCircle className="w-4 h-4" />}
      resolved={resolvedState}
      actions={
        <button
          type="button"
          onClick={handleSubmit}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-white bg-purple-600 rounded-md hover:bg-purple-700 transition-colors"
        >
          <Send className="w-3 h-3" />
          Submit
        </button>
      }
    >
      <div className="space-y-3">
        {parsed.questions.map((q, qi) => (
          <div key={q.question}>
            {/* Question header */}
            {q.header && (
              <div className="text-xs font-mono text-purple-600 dark:text-purple-400 uppercase tracking-wide mb-0.5">
                {q.header}
              </div>
            )}
            <div className="text-xs text-gray-800 dark:text-gray-200 leading-relaxed mb-2">
              {q.question}
            </div>

            {/* Options */}
            {q.options && q.options.length > 0 && (
              <div className="space-y-1.5">
                {q.options.map((opt, oi) => {
                  const isSelected = q.multiple
                    ? (multiSelections[qi]?.has(oi) ?? false)
                    : singleSelections[qi] === oi

                  return (
                    <button
                      key={opt.label}
                      type="button"
                      onClick={() =>
                        q.multiple ? handleMultiToggle(qi, oi) : handleSingleSelect(qi, oi)
                      }
                      className={`w-full flex items-start gap-2 px-2 py-1.5 rounded text-left border transition-colors ${
                        isSelected
                          ? 'bg-purple-50 dark:bg-purple-900/20 border-purple-300 dark:border-purple-500/40'
                          : 'bg-white/50 dark:bg-gray-800/30 border-gray-200/50 dark:border-gray-700/30 hover:border-purple-300/50 dark:hover:border-purple-500/20'
                      }`}
                    >
                      {q.multiple ? (
                        <CheckCircle2
                          className={`w-3.5 h-3.5 flex-shrink-0 mt-0.5 ${
                            isSelected
                              ? 'text-purple-500 dark:text-purple-400'
                              : 'text-gray-400 dark:text-gray-500'
                          }`}
                        />
                      ) : (
                        <Circle
                          className={`w-3.5 h-3.5 flex-shrink-0 mt-0.5 ${
                            isSelected
                              ? 'text-purple-500 dark:text-purple-400'
                              : 'text-gray-400 dark:text-gray-500'
                          }`}
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

                {/* "Other" option */}
                <button
                  type="button"
                  onClick={() => handleOtherToggle(qi, !!q.multiple)}
                  className={`w-full flex items-start gap-2 px-2 py-1.5 rounded text-left border transition-colors ${
                    otherSelected[qi]
                      ? 'bg-purple-50 dark:bg-purple-900/20 border-purple-300 dark:border-purple-500/40'
                      : 'bg-white/50 dark:bg-gray-800/30 border-gray-200/50 dark:border-gray-700/30 hover:border-purple-300/50 dark:hover:border-purple-500/20'
                  }`}
                >
                  <Circle
                    className={`w-3.5 h-3.5 flex-shrink-0 mt-0.5 ${
                      otherSelected[qi]
                        ? 'text-purple-500 dark:text-purple-400'
                        : 'text-gray-400 dark:text-gray-500'
                    }`}
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
                    className="w-full text-xs px-2 py-1.5 rounded border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-gray-800 dark:text-gray-200 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-purple-500/50"
                  />
                )}

                <div className="text-xs text-gray-400 dark:text-gray-500 italic px-2 pt-1">
                  {q.multiple ? 'Multiple selections allowed' : 'Single selection only'}
                </div>
              </div>
            )}

            {/* Display selected answers when resolved */}
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
