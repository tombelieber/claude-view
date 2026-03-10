import type { RichMessage } from './RichPane'

/**
 * Returns true if a queue-operation enqueue message contains user-typed content
 * (as opposed to task/system payloads).
 */
export function isUserQueueContent(content: string | undefined): boolean {
  if (!content || !content.trim()) return false
  const trimmed = content.trim()
  // Task dispatches: JSON with task_id
  if (trimmed.startsWith('{') && trimmed.includes('"task_id"')) return false
  // Task notifications: XML-like
  if (trimmed.startsWith('<task-notification>')) return false
  return true
}

/**
 * Two-pass filter: identifies active (un-dequeued) user enqueue messages
 * and injects them as pending user messages into the display list.
 *
 * Returns indices of messages that should be shown as pending user messages.
 */
export function findActiveUserEnqueues(messages: RichMessage[]): Set<number> {
  // Track ALL enqueues (including task dispatches) to maintain correct FIFO ordering.
  const enqueueStack: { index: number; isUser: boolean }[] = []
  const active = new Set<number>()

  for (let i = 0; i < messages.length; i++) {
    const m = messages[i]
    if (m.category !== 'queue' || !m.metadata) continue

    const op = m.metadata.operation as string | undefined
    if (op === 'enqueue') {
      enqueueStack.push({
        index: i,
        isUser: isUserQueueContent(m.metadata.content as string | undefined),
      })
    } else if (op === 'dequeue' || op === 'remove') {
      enqueueStack.shift() // consume oldest, regardless of type
    } else if (op === 'popAll') {
      enqueueStack.length = 0 // clear all
    }
  }

  // Only user enqueues get pending display
  for (const entry of enqueueStack) {
    if (entry.isUser) {
      active.add(entry.index)
    }
  }
  return active
}
