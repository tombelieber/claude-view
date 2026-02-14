// Phase B: View mode types and display status mapping

export type LiveViewMode = 'grid' | 'list' | 'kanban' | 'monitor'

/** Display status for UI grouping (maps from raw session status) */
export type LiveDisplayStatus = 'working' | 'waiting' | 'idle' | 'done'

export const LIVE_VIEW_MODES = [
  { id: 'grid' as const, label: 'Grid', icon: 'LayoutGrid', shortcut: '1' },
  { id: 'list' as const, label: 'List', icon: 'List', shortcut: '2' },
  { id: 'kanban' as const, label: 'Board', icon: 'Columns3', shortcut: '3' },
  { id: 'monitor' as const, label: 'Monitor', icon: 'Monitor', shortcut: '4' },
] as const

/** Map raw backend session status to display status for UI grouping */
export function toDisplayStatus(status: string): LiveDisplayStatus {
  switch (status) {
    case 'streaming':
    case 'tool_use':
      return 'working'
    case 'waiting_for_user':
      return 'waiting'
    case 'complete':
      return 'done'
    case 'idle':
    default:
      return 'idle'
  }
}

/** Custom sort order for display statuses */
export const DISPLAY_STATUS_ORDER: Record<LiveDisplayStatus, number> = {
  working: 0,
  waiting: 1,
  idle: 2,
  done: 3,
}

export const LIVE_VIEW_STORAGE_KEY = 'claude-view:live-view-mode'
