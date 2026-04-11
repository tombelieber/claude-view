import type { LastPrompt } from '../../../../../types/sidecar-protocol'
import { MessageSquare } from 'lucide-react'

interface Props {
  data: LastPrompt
}

export function LastPromptPill({ data }: Props) {
  const prompt = data?.lastPrompt ?? ''
  const display = prompt.length > 100 ? `${prompt.slice(0, 100)}\u2026` : prompt
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <MessageSquare className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">{display}</span>
    </div>
  )
}
