import { User, Bot } from 'lucide-react'

interface TurnSeparatorRowProps {
  role: 'user' | 'assistant'
  content: string
}

export function TurnSeparatorRow({ role, content }: TurnSeparatorRowProps) {
  const Icon = role === 'user' ? User : Bot
  const label = role === 'user' ? 'User' : 'Assistant'

  return (
    <div className="flex items-center gap-2 px-3 py-1.5">
      <div className="h-px flex-1 bg-gray-800" />
      <Icon className="w-3 h-3 text-gray-600 flex-shrink-0" />
      <span className="text-[10px] text-gray-600 font-medium flex-shrink-0">{label}:</span>
      <span className="text-[10px] text-gray-600 truncate max-w-[250px]" title={content}>
        {content}
      </span>
      <div className="h-px flex-1 bg-gray-800" />
    </div>
  )
}
