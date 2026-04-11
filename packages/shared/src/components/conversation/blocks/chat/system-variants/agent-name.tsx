import { Bot } from 'lucide-react'

interface Props {
  data: Record<string, unknown>
}

export function AgentNamePill({ data }: Props) {
  const name = data.agentName as string | undefined
  if (!name) return null
  return (
    <div className="flex items-center gap-2 px-3 py-1.5 text-xs text-sky-600 dark:text-sky-400">
      <Bot className="w-3 h-3 flex-shrink-0" />
      <span className="font-medium">Subagent: {name}</span>
    </div>
  )
}
