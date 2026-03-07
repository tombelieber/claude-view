import type { RelaySession } from '../types/relay'

/** Group sessions by whether they need user attention.
 *  NOTE (audit fix B1): Uses agentState.group, NOT status field.
 */
export function groupByStatus(sessions: RelaySession[]): {
  needsYou: RelaySession[]
  autonomous: RelaySession[]
} {
  const needsYou: RelaySession[] = []
  const autonomous: RelaySession[] = []

  for (const s of sessions) {
    if (s.agentState.group === 'needs_you') {
      needsYou.push(s)
    } else {
      autonomous.push(s)
    }
  }

  return { needsYou, autonomous }
}
