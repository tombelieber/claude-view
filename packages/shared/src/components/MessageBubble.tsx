import { Bot, User, Wrench } from 'lucide-react'
import type { Message } from '../types/message'
import { cn } from '../utils/cn'
import { MarkdownContent } from './MarkdownContent'
import { ThinkingBlock } from './ThinkingBlock'

interface MessageBubbleProps {
  message: Message
}

const ROLE_CONFIG: Record<
  string,
  { label: string; borderColor: string; badgeBg: string; badgeText: string; icon: typeof User }
> = {
  user: {
    label: 'User',
    borderColor: 'border-l-blue-400',
    badgeBg: 'bg-blue-50 dark:bg-blue-950/40',
    badgeText: 'text-blue-600 dark:text-blue-400',
    icon: User,
  },
  assistant: {
    label: 'Assistant',
    borderColor: 'border-l-orange-400',
    badgeBg: 'bg-orange-50 dark:bg-orange-950/40',
    badgeText: 'text-orange-600 dark:text-orange-400',
    icon: Bot,
  },
  tool_use: {
    label: 'Tool Use',
    borderColor: 'border-l-purple-400',
    badgeBg: 'bg-purple-50 dark:bg-purple-950/40',
    badgeText: 'text-purple-600 dark:text-purple-400',
    icon: Wrench,
  },
  tool_result: {
    label: 'Tool Result',
    borderColor: 'border-l-green-400',
    badgeBg: 'bg-green-50 dark:bg-green-950/40',
    badgeText: 'text-green-600 dark:text-green-400',
    icon: Wrench,
  },
}

function formatTimestamp(ts: string | null | undefined): string | null {
  if (!ts) return null
  try {
    const date = new Date(ts)
    if (Number.isNaN(date.getTime())) return null
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  } catch {
    return null
  }
}

export function MessageBubble({ message }: MessageBubbleProps) {
  const config = ROLE_CONFIG[message.role]

  // Skip system/progress/summary messages in shared view
  if (!config) return null

  const Icon = config.icon
  const time = formatTimestamp(message.timestamp)

  // Tool call summary
  const toolSummary =
    message.tool_calls && message.tool_calls.length > 0
      ? message.tool_calls
          .map((tc) => `${tc.name}${tc.count > 1 ? ` x${tc.count}` : ''}`)
          .join(', ')
      : null

  return (
    <div className={cn('border-l-2 pl-4 py-3', config.borderColor)}>
      <div className="flex items-center gap-2 mb-2">
        <span
          className={cn(
            'inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium',
            config.badgeBg,
            config.badgeText,
          )}
        >
          <Icon className="w-3 h-3" />
          {config.label}
        </span>
        {time && <span className="text-xs text-gray-400 dark:text-gray-500">{time}</span>}
        {toolSummary && (
          <span className="text-xs text-gray-400 dark:text-gray-500 italic">{toolSummary}</span>
        )}
      </div>

      {message.thinking && <ThinkingBlock thinking={message.thinking} />}

      {message.content && <MarkdownContent content={message.content} />}
    </div>
  )
}
