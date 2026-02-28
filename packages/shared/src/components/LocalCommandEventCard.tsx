import { Terminal } from 'lucide-react'

interface LocalCommandEventCardProps {
  content: string
}

export function LocalCommandEventCard({ content }: LocalCommandEventCardProps) {
  if (!content || !content.trim()) {
    return null
  }

  return (
    <div className="py-0.5 border-l-2 border-l-gray-400 pl-1 my-1">
      <div className="flex items-center gap-1.5">
        <Terminal className="w-3 h-3 text-gray-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400">{content}</span>
      </div>
    </div>
  )
}
