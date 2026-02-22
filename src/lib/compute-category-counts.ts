import type { RichMessage } from '../components/live/RichPane'
import type { ActionCategory } from '../components/live/action-log/types'

export type CategoryCounts = Record<ActionCategory, number>

const EMPTY: CategoryCounts = { skill: 0, mcp: 0, builtin: 0, agent: 0, hook: 0, hook_progress: 0, error: 0, system: 0, snapshot: 0, queue: 0, context: 0, result: 0, summary: 0 }

export function computeCategoryCounts(messages: RichMessage[]): CategoryCounts {
  const counts = { ...EMPTY }
  for (const m of messages) {
    if (m.category) {
      counts[m.category] = (counts[m.category] || 0) + 1
    }
  }
  return counts
}
