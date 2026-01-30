import { Terminal } from 'lucide-react'
import { cn } from '../lib/utils'

interface LocalCommandEventCardProps {
  content: string
}

export function LocalCommandEventCard({ content }: LocalCommandEventCardProps) {
  if (!content || !content.trim()) {
    return null
  }

  return (
    <div
      className={cn(
        'flex items-center gap-2 my-1 px-3 py-1.5 text-gray-500'
      )}
    >
      <Terminal className="w-3.5 h-3.5 flex-shrink-0" aria-hidden="true" />
      <span className="text-sm">{content}</span>
    </div>
  )
}
