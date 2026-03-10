import type { ChatMessageWithStatus } from '../types/control'
import type { Message } from '../types/generated/Message'

export type ConversationItem =
  | { kind: 'history'; message: Message }
  | { kind: 'live'; message: ChatMessageWithStatus }
  | { kind: 'divider' }

export function historyToItems(messages: Message[]): ConversationItem[] {
  return messages.map((message) => ({ kind: 'history' as const, message }))
}

export function liveToItems(messages: ChatMessageWithStatus[]): ConversationItem[] {
  return messages.map((message) => ({ kind: 'live' as const, message }))
}

export function buildConversationItems(
  history: Message[],
  live: ChatMessageWithStatus[],
): ConversationItem[] {
  const items: ConversationItem[] = historyToItems(history)
  if (live.length > 0) {
    items.push({ kind: 'divider' })
    items.push(...liveToItems(live))
  }
  return items
}
