import type { SessionInfo, ProjectInfo } from '../hooks/use-projects'

export interface ParsedQuery {
  text: string[]           // Free text terms
  project?: string         // project:name filter
  path?: string            // path:*.tsx filter (glob pattern)
  skill?: string           // skill:brainstorm filter
  after?: Date             // after:2026-01-20 filter
  before?: Date            // before:2026-01-25 filter
  regex?: RegExp           // /pattern/flags auto-detected
}

/**
 * Parse search query with syntax support:
 * - project:name
 * - path:*.tsx
 * - skill:brainstorm
 * - after:2026-01-20
 * - before:2026-01-25
 * - /regex/flags
 * - "exact phrase"
 * - plain text
 */
export function parseQuery(input: string): ParsedQuery {
  const result: ParsedQuery = { text: [] }

  if (!input.trim()) return result

  // Extract regex patterns first (e.g., /error.*fix/i)
  const regexMatch = input.match(/\/([^/]+)\/([gimsuy]*)/)
  if (regexMatch) {
    try {
      result.regex = new RegExp(regexMatch[1], regexMatch[2])
      input = input.replace(regexMatch[0], '')
    } catch {
      // Invalid regex, treat as text
    }
  }

  // Extract quoted phrases
  const phrases: string[] = []
  input = input.replace(/"([^"]+)"/g, (_, phrase) => {
    phrases.push(phrase.toLowerCase())
    return ''
  })

  // Parse tokens
  const tokens = input.trim().split(/\s+/).filter(Boolean)

  for (const token of tokens) {
    const [key, ...valueParts] = token.split(':')
    const value = valueParts.join(':')

    if (value) {
      switch (key.toLowerCase()) {
        case 'project':
          result.project = value.toLowerCase()
          break
        case 'path':
          result.path = value.toLowerCase()
          break
        case 'skill':
          result.skill = value.toLowerCase()
          break
        case 'after':
          result.after = parseDate(value)
          break
        case 'before':
          result.before = parseDate(value)
          break
        default:
          // Unknown filter, treat as text
          result.text.push(token.toLowerCase())
      }
    } else {
      result.text.push(token.toLowerCase())
    }
  }

  // Add quoted phrases to text
  result.text.push(...phrases)

  return result
}

function parseDate(value: string): Date | undefined {
  // Support formats: 2026-01-20, jan-20, yesterday, today
  const now = new Date()

  if (value === 'today') {
    return new Date(now.getFullYear(), now.getMonth(), now.getDate())
  }
  if (value === 'yesterday') {
    const d = new Date(now)
    d.setDate(d.getDate() - 1)
    return new Date(d.getFullYear(), d.getMonth(), d.getDate())
  }

  // Try ISO format
  const parsed = new Date(value)
  if (!isNaN(parsed.getTime())) {
    return parsed
  }

  // Try month-day format (e.g., jan-20)
  const monthMatch = value.match(/^([a-z]+)-(\d+)$/i)
  if (monthMatch) {
    const months = ['jan','feb','mar','apr','may','jun','jul','aug','sep','oct','nov','dec']
    const monthIndex = months.indexOf(monthMatch[1].toLowerCase())
    if (monthIndex >= 0) {
      return new Date(now.getFullYear(), monthIndex, parseInt(monthMatch[2]))
    }
  }

  return undefined
}

/**
 * Match a glob pattern against a string
 * Supports * (any chars) and ? (single char)
 */
function globMatch(pattern: string, str: string): boolean {
  const regex = pattern
    .replace(/[.+^${}()|[\]\\]/g, '\\$&')
    .replace(/\*/g, '.*')
    .replace(/\?/g, '.')
  return new RegExp(`^${regex}$`, 'i').test(str)
}

/**
 * Filter sessions based on parsed query
 */
export function filterSessions(
  sessions: SessionInfo[],
  projects: ProjectInfo[],
  query: ParsedQuery
): SessionInfo[] {
  return sessions.filter(session => {
    // Project filter
    if (query.project) {
      const project = projects.find(p => p.sessions.includes(session))
      if (!project) return false
      const projectName = project.displayName.toLowerCase()
      if (!projectName.includes(query.project)) return false
    }

    // Path filter (glob match against files touched)
    if (query.path) {
      const hasMatch = (session.filesTouched ?? []).some(f =>
        globMatch(query.path!, f.toLowerCase())
      )
      if (!hasMatch) return false
    }

    // Skill filter
    if (query.skill) {
      const hasSkill = (session.skillsUsed ?? []).some(s =>
        s.toLowerCase().includes(query.skill!)
      )
      if (!hasSkill) return false
    }

    // Date filters
    const sessionDate = new Date(session.modifiedAt)
    if (query.after && sessionDate < query.after) return false
    if (query.before && sessionDate > query.before) return false

    // Regex match against preview and lastMessage
    if (query.regex) {
      const text = `${session.preview} ${session.lastMessage}`
      if (!query.regex.test(text)) return false
    }

    // Text search (all terms must match somewhere)
    if (query.text.length > 0) {
      const searchable = `${session.preview} ${session.lastMessage} ${(session.filesTouched ?? []).join(' ')} ${(session.skillsUsed ?? []).join(' ')}`.toLowerCase()
      const allMatch = query.text.every(term => searchable.includes(term))
      if (!allMatch) return false
    }

    return true
  })
}
