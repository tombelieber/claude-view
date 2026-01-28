import type { SessionInfo } from '../hooks/use-projects'

export interface DateGroup {
  label: string
  sessions: SessionInfo[]
}

function startOfDay(date: Date): Date {
  const d = new Date(date)
  d.setHours(0, 0, 0, 0)
  return d
}

function isSameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
}

/** ISO week number (Mon=1) */
function getISOWeek(date: Date): number {
  const d = new Date(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()))
  d.setUTCDate(d.getUTCDate() + 4 - (d.getUTCDay() || 7))
  const yearStart = new Date(Date.UTC(d.getUTCFullYear(), 0, 1))
  return Math.ceil(((d.getTime() - yearStart.getTime()) / 86400000 + 1) / 7)
}

function getTierLabel(sessionDate: Date, now: Date): string {
  const today = startOfDay(now)
  const yesterday = new Date(today)
  yesterday.setDate(yesterday.getDate() - 1)

  if (isSameDay(sessionDate, today)) return 'Today'
  if (isSameDay(sessionDate, yesterday)) return 'Yesterday'

  const sessionWeek = getISOWeek(sessionDate)
  const sessionYear = sessionDate.getFullYear()
  const todayWeek = getISOWeek(now)
  const todayYear = now.getFullYear()

  if (sessionWeek === todayWeek && sessionYear === todayYear) return 'This Week'
  if (
    (sessionWeek === todayWeek - 1 && sessionYear === todayYear) ||
    (todayWeek === 1 && sessionYear === todayYear - 1)
  ) return 'Last Week'

  if (sessionDate.getMonth() === now.getMonth() && sessionYear === todayYear) return 'This Month'

  const prevMonth = new Date(now.getFullYear(), now.getMonth() - 1, 1)
  if (sessionDate.getMonth() === prevMonth.getMonth() && sessionDate.getFullYear() === prevMonth.getFullYear()) {
    return 'Last Month'
  }

  return sessionDate.toLocaleDateString('en-US', { month: 'long', year: 'numeric' })
}

/**
 * Group sessions by recency tiers (Today, Yesterday, This Week, etc.)
 * Sessions must already be sorted by modifiedAt DESC.
 */
export function groupSessionsByDate(sessions: SessionInfo[]): DateGroup[] {
  const now = new Date()
  const groups: DateGroup[] = []
  let currentLabel = ''
  let currentGroup: SessionInfo[] = []

  for (const session of sessions) {
    const date = new Date(session.modifiedAt * 1000)
    const label = getTierLabel(date, now)

    if (label !== currentLabel) {
      if (currentGroup.length > 0) {
        groups.push({ label: currentLabel, sessions: currentGroup })
      }
      currentLabel = label
      currentGroup = [session]
    } else {
      currentGroup.push(session)
    }
  }

  if (currentGroup.length > 0) {
    groups.push({ label: currentLabel, sessions: currentGroup })
  }

  return groups
}

/**
 * Count sessions per day for heatmap data.
 * Returns a Map of "YYYY-MM-DD" → count.
 */
export function countSessionsByDay(sessions: SessionInfo[]): Map<string, number> {
  const counts = new Map<string, number>()
  for (const session of sessions) {
    const date = new Date(session.modifiedAt * 1000)
    const key = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
    counts.set(key, (counts.get(key) ?? 0) + 1)
  }
  return counts
}

/** Format a date key "YYYY-MM-DD" */
export function toDateKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}
