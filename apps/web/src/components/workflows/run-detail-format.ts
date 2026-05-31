/** Formatting + small derivations shared across the workflow run detail panels. */

/** Above this count a flat list bloats the DOM, so switch to a virtualized region. */
export const VIRTUALIZE_THRESHOLD = 30

export function formatDuration(ms: number | null): string {
  if (!ms) return 'n/a'
  if (ms < 60_000) return `${Math.round(ms / 1000)}s`
  const minutes = Math.floor(ms / 60_000)
  const seconds = Math.round((ms % 60_000) / 1000)
  return `${minutes}m ${seconds}s`
}

export function formatDate(value: number | null): string {
  if (!value) return 'Unknown'
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(Number(value)))
}

export function formatNumber(value: number): string {
  return new Intl.NumberFormat(undefined).format(value)
}

/** A phase is "complete" only when it has agents and all of them finished. */
export function isPhaseComplete(completedAgentCount: number, agentCount: number): boolean {
  return agentCount > 0 && completedAgentCount >= agentCount
}
