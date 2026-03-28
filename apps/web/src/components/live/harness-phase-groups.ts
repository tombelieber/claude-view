import type { SessionPhase } from '../../types/generated/SessionPhase'
import type { LiveSession } from './use-live-sessions'

/** Displayable phases -- excludes 'working' which is the unclassified fallback */
export type DisplayPhase = Exclude<SessionPhase, 'working'>

export interface PhaseColumn {
  phase: DisplayPhase
  label: string
  emoji: string
  stripe: string
}

export const DESIGN_PHASES: PhaseColumn[] = [
  { phase: 'thinking', label: 'Thinking', emoji: '\u{1F4AD}', stripe: 'bg-purple-500' },
  { phase: 'planning', label: 'Planning', emoji: '\u{1F4CB}', stripe: 'bg-blue-500' },
  { phase: 'reviewing', label: 'Reviewing', emoji: '\u{1F50D}', stripe: 'bg-cyan-500' },
]

export const DELIVERY_PHASES: PhaseColumn[] = [
  { phase: 'building', label: 'Building', emoji: '\u{1F528}', stripe: 'bg-orange-500' },
  { phase: 'testing', label: 'Testing', emoji: '\u{1F9EA}', stripe: 'bg-green-500' },
  { phase: 'shipping', label: 'Shipping', emoji: '\u{1F680}', stripe: 'bg-red-500' },
]

export const PHASE_GROUPS = [
  { id: 'design', label: 'Design', emoji: '\u{1F4A1}', phases: DESIGN_PHASES },
  { id: 'delivery', label: 'Delivery', emoji: '\u{26A1}', phases: DELIVERY_PHASES },
] as const

export function getSessionPhase(session: LiveSession): DisplayPhase {
  const phase = session.phase?.current?.phase
  if (!phase || phase === 'working') return 'building'
  return phase
}

export function isDesignPhase(phase: string): boolean {
  return phase === 'thinking' || phase === 'planning' || phase === 'reviewing'
}

export function sortNeedsYouFirst(sessions: LiveSession[]): LiveSession[] {
  return [...sessions].sort((a, b) => {
    const ag = a.agentState.group === 'needs_you' ? 0 : 1
    const bg = b.agentState.group === 'needs_you' ? 0 : 1
    if (ag !== bg) return ag - bg
    return b.lastActivityAt - a.lastActivityAt
  })
}

export function splitByPhase(
  sessions: LiveSession[],
  phases: readonly PhaseColumn[],
): Record<string, LiveSession[]> {
  const map: Record<string, LiveSession[]> = {}
  for (const p of phases) map[p.phase] = []
  for (const s of sessions) {
    const phase = getSessionPhase(s)
    if (map[phase]) map[phase].push(s)
  }
  for (const arr of Object.values(map)) {
    arr.sort((a, b) => {
      const ag = a.agentState.group === 'needs_you' ? 0 : 1
      const bg = b.agentState.group === 'needs_you' ? 0 : 1
      if (ag !== bg) return ag - bg
      return b.lastActivityAt - a.lastActivityAt
    })
  }
  return map
}
