export type ActionCategory = 'skill' | 'mcp' | 'builtin' | 'agent' | 'error'

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

export type TimelineItem = ActionItem | TurnSeparator

export function isTurnSeparator(item: TimelineItem): item is TurnSeparator {
  return 'type' in item && (item as TurnSeparator).type === 'turn'
}
