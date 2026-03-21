import type { LiveSession } from '@claude-view/shared/types/generated'

export type LiveStatus = 'cc_owned' | 'cc_agent_sdk_owned' | 'inactive'

export function deriveLiveStatus(live: LiveSession | null | undefined): LiveStatus {
  if (live == null) return 'inactive'
  const isActive = live.status === 'working' || live.status === 'paused' || live.control != null
  if (!isActive) return 'inactive'
  return live.control != null ? 'cc_agent_sdk_owned' : 'cc_owned'
}
