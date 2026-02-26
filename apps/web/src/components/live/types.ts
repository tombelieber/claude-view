// Phase B2: Agent state types and display status mapping

export type LiveViewMode = 'grid' | 'list' | 'kanban' | 'monitor'

/** Agent state group — the operator's mental model */
export type AgentStateGroup = 'needs_you' | 'autonomous'

/** The universal agent state — core protocol */
export interface AgentState {
  group: AgentStateGroup
  state: string           // open string — v1 states listed above, more added over time
  label: string
  context?: Record<string, unknown>
}

// v1 known states (for icon/color mapping, but unknown states render with generic style)
export const KNOWN_STATES: Record<string, { icon: string; color: string }> = {
  // Needs You
  awaiting_input: { icon: 'MessageCircle', color: 'amber' },
  awaiting_approval: { icon: 'FileCheck', color: 'amber' },
  needs_permission: { icon: 'Shield', color: 'red' },
  error: { icon: 'AlertTriangle', color: 'red' },
  interrupted: { icon: 'CirclePause', color: 'amber' },
  idle: { icon: 'Clock', color: 'gray' },
  // Autonomous
  thinking: { icon: 'Sparkles', color: 'green' },
  acting: { icon: 'Terminal', color: 'green' },
  delegating: { icon: 'GitBranch', color: 'green' },
  // Terminal
  task_complete: { icon: 'CheckCircle2', color: 'gray' },
  session_ended: { icon: 'LogOut', color: 'gray' },
  work_delivered: { icon: 'Package', color: 'gray' },
}

// Unknown states get a generic icon/color for their group
export const GROUP_DEFAULTS: Record<AgentStateGroup, { icon: string; color: string }> = {
  needs_you: { icon: 'Bell', color: 'amber' },
  autonomous: { icon: 'Loader', color: 'green' },
}

/** Custom sort order for agent state groups */
export const GROUP_ORDER: Record<AgentStateGroup, number> = {
  needs_you: 0,
  autonomous: 1,
}

export const LIVE_VIEW_MODES = [
  { id: 'kanban' as const, label: 'Board', icon: 'Columns3', shortcut: '1' },
  { id: 'grid' as const, label: 'Grid', icon: 'LayoutGrid', shortcut: '2' },
  { id: 'list' as const, label: 'List', icon: 'List', shortcut: '3' },
  { id: 'monitor' as const, label: 'Monitor', icon: 'Monitor', shortcut: '4' },
] as const

export const LIVE_VIEW_STORAGE_KEY = 'claude-view:live-view-mode'
