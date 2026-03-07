import type { Message } from '../types/message'
import { MessageBubble } from './MessageBubble'

interface MessageListProps {
  messages: Message[]
}

/** Roles visible in the shared conversation renderer. */
const VISIBLE_ROLES = new Set(['user', 'assistant', 'tool_use', 'tool_result'])

export function MessageList({ messages }: MessageListProps) {
  const visible = messages.filter((m) => VISIBLE_ROLES.has(m.role) && m.content?.trim())

  if (visible.length === 0) {
    return (
      <div className="text-center text-gray-400 dark:text-gray-500 py-12 text-sm">
        No messages to display.
      </div>
    )
  }

  return (
    <div className="space-y-1">
      {visible.map((message, i) => (
        <MessageBubble key={message.uuid || i} message={message} />
      ))}
    </div>
  )
}
