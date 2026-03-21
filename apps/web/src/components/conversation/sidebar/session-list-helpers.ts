// Pure helper functions for SessionListItem — extracted for testability.
// Aligned with Live Monitor StatusDot colors: needs_you → amber, autonomous → green.

import type { LiveStatus } from '../../../lib/live-status'

interface SessionLike {
  liveData?: {
    agentState?: { group: string }
    status?: string
    control?: unknown
  } | null
  isActive?: boolean
  liveStatus?: LiveStatus
}

// --- Status dot color ---

export function getStatusDotColor(session: SessionLike): string {
  if (!session.liveData) return 'bg-gray-300 dark:bg-gray-600'
  const group = session.liveData.agentState?.group
  if (group === 'needs_you') return 'bg-amber-500'
  return 'bg-green-500'
}

// --- Status badge ---

export function getStatusBadge(session: SessionLike): { text: string; className: string } | null {
  if (!session.liveData) return null
  const group = session.liveData.agentState?.group
  const isNeedsYou = group === 'needs_you'

  const label = session.liveStatus === 'cc_agent_sdk_owned' ? 'Live' : 'Watching'
  return isNeedsYou
    ? {
        text: label,
        className: 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400',
      }
    : {
        text: label,
        className: 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400',
      }
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
    takeOver: ls === 'cc_owned',
    fork: true,
    shutDown: ls === 'cc_agent_sdk_owned',
    openInMonitor: ls !== 'inactive',
    archive: isHistory,
  }
}
