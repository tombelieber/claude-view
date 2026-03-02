import { MessageCircle, Send } from 'lucide-react'
import { useCallback, useState } from 'react'
import { InteractiveCardShell } from './InteractiveCardShell'

export interface ElicitationCardProps {
  requestId: string
  prompt: string
  onSubmit: (requestId: string, response: string) => void
  resolved?: boolean
}

export function ElicitationCard({ requestId, prompt, onSubmit, resolved }: ElicitationCardProps) {
  const [response, setResponse] = useState('')

  const handleSubmit = useCallback(() => {
    if (!response.trim()) return
    onSubmit(requestId, response.trim())
  }, [onSubmit, requestId, response])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        handleSubmit()
      }
    },
    [handleSubmit],
  )

  const resolvedState = resolved ? { label: 'Submitted', variant: 'neutral' as const } : undefined

  return (
    <InteractiveCardShell
      variant="elicitation"
      header="Input Requested"
      icon={<MessageCircle className="w-4 h-4" />}
      resolved={resolvedState}
      actions={
        <button
          type="button"
          onClick={handleSubmit}
          disabled={!response.trim()}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-white bg-gray-700 dark:bg-gray-600 rounded-md hover:bg-gray-800 dark:hover:bg-gray-500 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <Send className="w-3 h-3" />
          Submit
        </button>
      }
    >
      <div className="space-y-2">
        <p className="text-xs text-gray-800 dark:text-gray-200 leading-relaxed">{prompt}</p>
        <input
          type="text"
          value={response}
          onChange={(e) => setResponse(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type your response..."
          className="w-full text-xs px-2 py-1.5 rounded border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-gray-800 dark:text-gray-200 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-gray-500/50"
        />
      </div>
    </InteractiveCardShell>
  )
}
