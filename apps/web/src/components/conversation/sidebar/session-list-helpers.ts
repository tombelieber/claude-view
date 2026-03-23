// Pure helper functions for SessionListItem — extracted for testability.
// Aligned with Live Monitor StatusDot colors: needs_you → amber, autonomous → green.
// Mission Control grouping: urgency-first (NEEDS YOU → WORKING → history).

import type { LiveStatus } from '../../../lib/live-status'

export interface SessionLike {
  liveData?: {
    agentState?: { group: string }
    status?: string
    control?: unknown
  } | null
  isActive?: boolean
  liveStatus?: LiveStatus
}

// --- Source type (for icon rendering) ---

export type SessionSource = 'terminal' | 'sdk'

export function getSessionSource(session: SessionLike): SessionSource {
  return session.liveStatus === 'cc_agent_sdk_owned' ? 'sdk' : 'terminal'
}

// --- Urgency grouping ---

export type UrgencyGroup = 'needs_you' | 'working'

export function getUrgencyGroup(session: SessionLike): UrgencyGroup {
  const group = session.liveData?.agentState?.group
  return group === 'needs_you' ? 'needs_you' : 'working'
}

export function groupByUrgency<T extends SessionLike>(
  sessions: T[],
): { needsYou: T[]; working: T[] } {
  const needsYou: T[] = []
  const working: T[] = []
  for (const s of sessions) {
    if (getUrgencyGroup(s) === 'needs_you') needsYou.push(s)
    else working.push(s)
  }
  return { needsYou, working }
}

// --- Status dot color ---

export function getStatusDotColor(session: SessionLike): string {
  if (!session.liveData) return 'bg-gray-300 dark:bg-gray-600'
  const group = session.liveData.agentState?.group
  if (group === 'needs_you') return 'bg-amber-500'
  return 'bg-green-500'
}

// --- Dropdown action visibility ---

export interface DropdownActions {
  resume: boolean
  takeOver: boolean
  fork: boolean
  shutDown: boolean
  openInMonitor: boolean
  archive: boolean
}

export function deriveDropdownActions(session: SessionLike): DropdownActions {
  const ls = session.liveStatus ?? 'inactive'
  const isHistory = ls === 'inactive'

  return {
    resume: isHistory,
    takeOver: false, // TODO: re-enable when fork flow is fixed
    fork: false, // TODO: re-enable when fork flow is fixed
    shutDown: ls === 'cc_agent_sdk_owned',
    openInMonitor: ls !== 'inactive',
    archive: isHistory,
  }
}
