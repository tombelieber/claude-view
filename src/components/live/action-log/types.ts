export type ActionCategory = 'skill' | 'mcp' | 'builtin' | 'agent' | 'hook' | 'error' | 'system' | 'snapshot' | 'queue'

export interface ActionItem {
  id: string
  timestamp?: number
  duration?: number
  category: ActionCategory
  toolName: string
  label: string
  status: 'success' | 'error' | 'pending'
  input?: string
  output?: string
}

export interface TurnSeparator {
  id: string
  type: 'turn'
  role: 'user' | 'assistant'
  content: string
  timestamp?: number
}

export interface HookEventItem {
  id: string
  timestamp: number
  type: 'hook_event'
  eventName: string
  toolName?: string
  label: string
  group: 'autonomous' | 'needs_you' | 'delivered'
  context?: string
}

export type TimelineItem = ActionItem | TurnSeparator | HookEventItem

export function isTurnSeparator(item: TimelineItem): item is TurnSeparator {
  return 'type' in item && (item as TurnSeparator).type === 'turn'
}

export function isHookEvent(item: TimelineItem): item is HookEventItem {
  return 'type' in item && (item as HookEventItem).type === 'hook_event'
}
