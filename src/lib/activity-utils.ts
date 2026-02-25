import type { SessionInfo } from '../types/generated/SessionInfo'

/** A single day's aggregated activity */
export interface DayActivity {
  /** Date string YYYY-MM-DD */
  date: string
  /** Total seconds spent across all sessions */
  totalSeconds: number
  /** Number of sessions */
  sessionCount: number
  /** Sessions for this day, sorted by start time ascending */
  sessions: SessionInfo[]
}

/** A project's aggregated time */
export interface ProjectActivity {
  /** Project display name (last path segment) */
  name: string
  /** Full project path */
  projectPath: string
  /** Total seconds */
  totalSeconds: number
  /** Number of sessions */
  sessionCount: number
}

export interface ActivitySummary {
  totalSeconds: number
  sessionCount: number
  avgSessionSeconds: number
  longestSession: { seconds: number; project: string; title: string } | null
  busiestDay: { date: string; totalSeconds: number } | null
  /** Total tool calls across all sessions */
  totalToolCalls: number
  /** Total agent spawns across all sessions */
  totalAgentSpawns: number
  /** Total MCP interactions across all sessions */
  totalMcpCalls: number
  /** Unique skill names used across all sessions */
  uniqueSkills: number
}

/** Get session start timestamp — prefer firstMessageAt, fall back to modifiedAt - duration */
export function sessionStartTime(session: SessionInfo): number {
  if (session.firstMessageAt && session.firstMessageAt > 0) {
    return session.firstMessageAt
  }
  // Fallback: modifiedAt (= last_message_at) minus duration. Guard against
  // negative results from corrupted data (CLAUDE.md: guard ts <= 0 at every layer).
  return Math.max(0, session.modifiedAt - session.durationSeconds)
}

/** Get the date string (YYYY-MM-DD) for a Unix timestamp in local timezone */
function dateKey(unixSeconds: number): string {
  if (unixSeconds <= 0) return ''
  const d = new Date(unixSeconds * 1000)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

/** Get project display name from path */
export function projectDisplayName(projectPath: string): string {
  const parts = projectPath.split('/')
  return parts[parts.length - 1] || projectPath
}

/** Aggregate sessions into daily activity, sorted newest-first */
export function aggregateByDay(sessions: SessionInfo[]): DayActivity[] {
  const dayMap = new Map<string, DayActivity>()

  for (const session of sessions) {
    if (session.durationSeconds <= 0) continue
    const start = sessionStartTime(session)
    const key = dateKey(start)
    if (!key) continue

    let day = dayMap.get(key)
    if (!day) {
      day = { date: key, totalSeconds: 0, sessionCount: 0, sessions: [] }
      dayMap.set(key, day)
    }
    day.totalSeconds += session.durationSeconds
    day.sessionCount += 1
    day.sessions.push(session)
  }

  // Sort sessions within each day by start time ascending
  for (const day of dayMap.values()) {
    day.sessions.sort((a, b) => sessionStartTime(a) - sessionStartTime(b))
  }

  // Return days sorted newest-first
  return Array.from(dayMap.values()).sort((a, b) => b.date.localeCompare(a.date))
}

/** Aggregate sessions by project, sorted by total time descending */
export function aggregateByProject(sessions: SessionInfo[]): ProjectActivity[] {
  const projectMap = new Map<string, ProjectActivity>()

  for (const session of sessions) {
    if (session.durationSeconds <= 0) continue
    // Prefer git_root (canonical repo root) for grouping so that monorepo sub-crates
    // roll up into a single project. Falls back to projectPath, then legacy project name.
    const path = (session.gitRoot || null) ?? session.projectPath ?? session.project
    let proj = projectMap.get(path)
    if (!proj) {
      proj = { name: projectDisplayName(path), projectPath: path, totalSeconds: 0, sessionCount: 0 }
      projectMap.set(path, proj)
    }
    proj.totalSeconds += session.durationSeconds
    proj.sessionCount += 1
  }

  return Array.from(projectMap.values()).sort((a, b) => b.totalSeconds - a.totalSeconds)
}

/** Compute summary statistics */
export function computeSummary(sessions: SessionInfo[], days: DayActivity[]): ActivitySummary {
  // Count ALL sessions for the headline number (matches footer/status bar).
  // Only filter to duration>0 for time-based calculations (avg, longest, etc.).
  const timedSessions = sessions.filter(s => s.durationSeconds > 0)
  const totalSeconds = timedSessions.reduce((sum, s) => sum + s.durationSeconds, 0)
  const sessionCount = sessions.length

  let longestSession: ActivitySummary['longestSession'] = null
  let maxDuration = 0
  for (const s of timedSessions) {
    if (s.durationSeconds > maxDuration) {
      maxDuration = s.durationSeconds
      longestSession = {
        seconds: s.durationSeconds,
        project: projectDisplayName((s.gitRoot || null) ?? s.projectPath ?? s.project),
        title: s.summary || s.preview || '(untitled)',
      }
    }
  }

  let busiestDay: ActivitySummary['busiestDay'] = null
  let maxDaySeconds = 0
  for (const day of days) {
    if (day.totalSeconds > maxDaySeconds) {
      maxDaySeconds = day.totalSeconds
      busiestDay = { date: day.date, totalSeconds: day.totalSeconds }
    }
  }

  // Aggregate tool/agent/mcp/skills counts
  let totalToolCalls = 0
  let totalAgentSpawns = 0
  let totalMcpCalls = 0
  const skillSet = new Set<string>()
  for (const s of sessions) {
    totalToolCalls += s.toolCallCount ?? 0
    totalAgentSpawns += s.agentSpawnCount ?? 0
    totalMcpCalls += s.mcpProgressCount ?? 0
    if (s.skillsUsed) {
      for (const skill of s.skillsUsed) {
        if (skill) skillSet.add(skill)
      }
    }
  }

  return {
    totalSeconds,
    sessionCount,
    avgSessionSeconds: timedSessions.length > 0 ? Math.round(totalSeconds / timedSessions.length) : 0,
    longestSession,
    busiestDay,
    totalToolCalls,
    totalAgentSpawns,
    totalMcpCalls,
    uniqueSkills: skillSet.size,
  }
}
