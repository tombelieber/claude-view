import { readdir, stat, open } from 'fs/promises'
import { existsSync } from 'fs'
import { join, basename } from 'path'
import { homedir } from 'os'

export interface SessionInfo {
  id: string
  project: string
  projectPath: string
  filePath: string
  modifiedAt: Date
  sizeBytes: number
  preview: string
  // Rich metadata fields
  lastMessage: string          // Last user message
  filesTouched: string[]       // Files edited/written (max 5)
  skillsUsed: string[]         // Slash commands detected
  toolCounts: {
    edit: number
    read: number
    bash: number
    write: number
  }
}

export interface ProjectInfo {
  name: string
  displayName: string  // Just the project folder name (e.g., "claude-view")
  path: string
  sessions: SessionInfo[]
  activeCount: number  // Number of sessions active in the last hour
}

const CLAUDE_PROJECTS_DIR = join(homedir(), '.claude', 'projects')

interface ResolvedProject {
  fullPath: string      // e.g., "Users/TBGor/dev/@vicky-ai/claude-view"
  displayName: string   // e.g., "claude-view"
}

/**
 * Resolve an encoded directory name to its actual filesystem path.
 * Uses filesystem verification to correctly handle directory names with hyphens.
 *
 * Claude encodes paths like: /Users/TBGor/dev/@vicky-ai/claude-view
 * As directory names like: -Users-TBGor-dev--vicky-ai-claude-view
 *
 * The challenge is that hyphens in directory names (e.g., "claude-view")
 * look the same as path separators. We verify against the filesystem to
 * find the correct interpretation.
 */
function resolveProjectPath(encodedName: string): ResolvedProject {
  // Step 1: Basic decode (this may be wrong if dir names contain hyphens)
  const basicDecode = '/' + encodedName
    .replace(/^-/, '')      // Remove leading dash
    .replace(/--/g, '/@')   // Double dash represents @
    .replace(/-/g, '/')     // Single dash represents /

  // Step 2: Check if the basic decode exists on filesystem
  if (existsSync(basicDecode)) {
    return {
      fullPath: basicDecode.slice(1), // Remove leading /
      displayName: basename(basicDecode)
    }
  }

  // Step 3: Use recursive resolution to find the correct path
  // by trying different hyphen combinations at each level
  const segments = basicDecode.slice(1).split('/')
  const resolvedPath = resolveSegments(segments, '')

  if (resolvedPath) {
    return {
      fullPath: resolvedPath.slice(1), // Remove leading /
      displayName: basename(resolvedPath)
    }
  }

  // Fallback: return the basic decode even if it doesn't exist
  // (project directory may have been deleted)
  return {
    fullPath: basicDecode.slice(1),
    displayName: basename(basicDecode)
  }
}

/**
 * Recursively resolve path segments, trying different separator combinations
 * at each level to find the actual filesystem path.
 *
 * Claude's encoding uses `-` for multiple characters: `/`, `.`, and literal `-`
 * So we try joining segments with both `-` and `.` to find the actual directory.
 */
function resolveSegments(segments: string[], currentPath: string): string | null {
  if (segments.length === 0) {
    return currentPath || null
  }

  // Try joining progressively more segments
  // Start with most segments joined (longest possible directory name)
  // and work down to single segments
  for (let joinCount = segments.length; joinCount >= 1; joinCount--) {
    const toJoin = segments.slice(0, joinCount)

    // Try different separator combinations: all hyphens, all dots, or mixed
    const joinVariants = getJoinVariants(toJoin)

    for (const joined of joinVariants) {
      const testPath = currentPath ? `${currentPath}/${joined}` : `/${joined}`

      if (existsSync(testPath)) {
        // This path exists, recursively resolve remaining segments
        const remaining = segments.slice(joinCount)

        if (remaining.length === 0) {
          // No more segments, we found the full path
          return testPath
        }

        // Try to resolve remaining segments
        const result = resolveSegments(remaining, testPath)
        if (result) {
          return result
        }
        // If remaining segments couldn't be resolved, try other join variants
      }
    }
  }

  return null
}

/**
 * Generate different ways to join segments using `-` and `.` separators.
 * For efficiency, we prioritize common patterns:
 * 1. All hyphens (most common for multi-word names like "claude-view")
 * 2. Single dot before last segment (common for domains like "famatch.io")
 * 3. All dots (less common but possible)
 */
function getJoinVariants(segments: string[]): string[] {
  if (segments.length === 1) {
    return [segments[0]]
  }

  const variants: string[] = []

  // All hyphens (e.g., "claude-view", "my-cool-project")
  variants.push(segments.join('-'))

  // Dot before last segment (e.g., "famatch.io", "example.com")
  if (segments.length === 2) {
    variants.push(segments.join('.'))
  } else if (segments.length > 2) {
    // e.g., ["my", "app", "io"] -> "my-app.io"
    const beforeLast = segments.slice(0, -1).join('-')
    variants.push(`${beforeLast}.${segments[segments.length - 1]}`)
  }

  return variants
}

interface SessionMetadata {
  preview: string
  lastMessage: string
  filesTouched: string[]
  skillsUsed: string[]
  toolCounts: {
    edit: number
    read: number
    bash: number
    write: number
  }
}

/**
 * Extract rich metadata from a JSONL session file
 * Scans the entire file but processes efficiently line-by-line
 */
async function getSessionMetadata(filePath: string): Promise<SessionMetadata> {
  const result: SessionMetadata = {
    preview: '(no user message found)',
    lastMessage: '',
    filesTouched: [],
    skillsUsed: [],
    toolCounts: { edit: 0, read: 0, bash: 0, write: 0 }
  }

  const filesSet = new Set<string>()
  const skillsSet = new Set<string>()
  let firstUserMessage = ''
  let lastUserMessage = ''

  let handle
  try {
    handle = await open(filePath, 'r')
    const content = await handle.readFile({ encoding: 'utf-8' })
    const lines = content.split('\n')

    for (const line of lines) {
      if (!line.trim()) continue

      try {
        const entry = JSON.parse(line)

        // Extract user messages
        if (entry.type === 'user' && entry.message?.content) {
          let text = typeof entry.message.content === 'string'
            ? entry.message.content
            : (Array.isArray(entry.message.content)
                ? entry.message.content.find((b: { type: string; text?: string }) => b.type === 'text')?.text
                : '') || ''

          // Clean command tags
          text = text.replace(/<command-[^>]*>[^<]*<\/command-[^>]*>/g, '').trim()

          // Skip system/tool messages
          if (text && !text.startsWith('<') && text.length > 10) {
            if (!firstUserMessage) {
              firstUserMessage = text.length > 200 ? text.substring(0, 200) + '…' : text
            }
            lastUserMessage = text.length > 200 ? text.substring(0, 200) + '…' : text

            // Detect skills (slash commands)
            const skillMatches = text.match(/\/[\w:-]+/g)
            if (skillMatches) {
              skillMatches.forEach((s: string) => skillsSet.add(s))
            }
          }
        }

        // Count tool usage and extract files
        if (entry.type === 'assistant' && entry.message?.content) {
          const content = entry.message.content
          if (Array.isArray(content)) {
            for (const block of content) {
              if (block.type === 'tool_use') {
                const toolName = block.name?.toLowerCase() || ''
                if (toolName === 'edit') {
                  result.toolCounts.edit++
                  if (block.input?.file_path) filesSet.add(block.input.file_path)
                } else if (toolName === 'write') {
                  result.toolCounts.write++
                  if (block.input?.file_path) filesSet.add(block.input.file_path)
                } else if (toolName === 'read') {
                  result.toolCounts.read++
                } else if (toolName === 'bash') {
                  result.toolCounts.bash++
                }
              }
            }
          }
        }
      } catch {
        // Skip malformed lines
        continue
      }
    }

    result.preview = firstUserMessage || '(no user message found)'
    result.lastMessage = lastUserMessage
    result.filesTouched = Array.from(filesSet).slice(0, 5).map(f => {
      // Shorten to just filename for display
      const parts = f.split('/')
      return parts[parts.length - 1]
    })
    result.skillsUsed = Array.from(skillsSet).slice(0, 5)

    return result
  } catch {
    return result
  } finally {
    await handle?.close()
  }
}

/**
 * Discover all Claude Code projects and their sessions
 */
export async function getProjects(): Promise<ProjectInfo[]> {
  const projects: ProjectInfo[] = []

  try {
    const entries = await readdir(CLAUDE_PROJECTS_DIR, { withFileTypes: true })

    for (const entry of entries) {
      // Skip non-directories and hidden directories
      if (!entry.isDirectory() || entry.name.startsWith('.')) {
        continue
      }

      const projectDirPath = join(CLAUDE_PROJECTS_DIR, entry.name)
      const resolved = resolveProjectPath(entry.name)
      const sessions: SessionInfo[] = []

      try {
        const projectFiles = await readdir(projectDirPath)

        for (const file of projectFiles) {
          // Only process .jsonl files (sessions)
          if (!file.endsWith('.jsonl')) {
            continue
          }

          const filePath = join(projectDirPath, file)

          try {
            const fileStat = await stat(filePath)

            // Skip directories (shouldn't happen but be safe)
            if (!fileStat.isFile()) {
              continue
            }

            // Extract session ID from filename (remove .jsonl extension)
            const sessionId = file.replace('.jsonl', '')

            // Get rich metadata from session
            const metadata = await getSessionMetadata(filePath)

            sessions.push({
              id: sessionId,
              project: entry.name,
              projectPath: resolved.fullPath,
              filePath,
              modifiedAt: fileStat.mtime,
              sizeBytes: fileStat.size,
              preview: metadata.preview,
              lastMessage: metadata.lastMessage,
              filesTouched: metadata.filesTouched,
              skillsUsed: metadata.skillsUsed,
              toolCounts: metadata.toolCounts
            })
          } catch {
            // Skip files we can't stat
            continue
          }
        }
      } catch {
        // Skip projects we can't read
        continue
      }

      // Only include projects that have sessions and valid displayName
      if (sessions.length > 0 && resolved.displayName) {
        // Sort sessions by modified date (newest first)
        sessions.sort((a, b) => b.modifiedAt.getTime() - a.modifiedAt.getTime())

        // Count sessions active in the last 5 minutes (likely still in use)
        const fiveMinutesAgo = Date.now() - 5 * 60 * 1000
        const activeCount = sessions.filter(s => s.modifiedAt.getTime() > fiveMinutesAgo).length

        projects.push({
          name: resolved.fullPath,
          displayName: resolved.displayName,
          path: projectDirPath,
          sessions,
          activeCount
        })
      }
    }

    // Sort projects by most recent session (newest first)
    projects.sort((a, b) => {
      const aLatest = a.sessions[0]?.modifiedAt.getTime() || 0
      const bLatest = b.sessions[0]?.modifiedAt.getTime() || 0
      return bLatest - aLatest
    })

    return projects
  } catch (error) {
    // If we can't access the Claude projects directory, return empty array
    console.error('Failed to access Claude projects directory:', error)
    return []
  }
}
