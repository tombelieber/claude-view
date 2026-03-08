import type { LiveSession } from '../types/generated'

/** Group sessions by whether they need user attention.
 *  NOTE (audit fix B1): Uses agentState.group, NOT status field.
 */
export function groupByStatus(sessions: LiveSession[]): {
  needsYou: LiveSession[]
  autonomous: LiveSession[]
} {
  const needsYou: LiveSession[] = []
  const autonomous: LiveSession[] = []

  for (const s of sessions) {
    if (s.agentState.group === 'needs_you') {
      needsYou.push(s)
    } else {
      autonomous.push(s)
    }
  }

  return { needsYou, autonomous }
}
