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

/**
 * Extract the first user message from a JSONL session file for preview
 * Only reads the first 8KB of the file to avoid memory issues with large sessions
 */
async function getSessionPreview(filePath: string): Promise<string> {
  let handle
  try {
    handle = await open(filePath, 'r')
    const buffer = Buffer.alloc(8192)  // Read first 8KB
    await handle.read(buffer, 0, 8192, 0)
    const content = buffer.toString('utf-8')
    const lines = content.split('\n')

    for (const line of lines) {
      if (!line.trim()) continue

      try {
        const entry = JSON.parse(line)
        if (entry.type === 'user' && entry.message?.content) {
          // Get the content, handling various formats
          // Extract text from content blocks if it's an array
          let preview = typeof entry.message.content === 'string'
            ? entry.message.content
            : (Array.isArray(entry.message.content)
                ? entry.message.content.find((b: { type: string; text?: string }) => b.type === 'text')?.text
                : '') || ''

          // Clean up command tags if present
          preview = preview.replace(/<command-[^>]*>[^<]*<\/command-[^>]*>/g, '').trim()

          // Truncate to reasonable preview length
          if (preview.length > 200) {
            preview = preview.substring(0, 200) + '...'
          }

          return preview || '(empty message)'
        }
      } catch {
        // Skip malformed JSON lines (may be truncated at buffer boundary)
        continue
      }
    }

    return '(no user message found)'
  } catch {
    return '(unable to read session)'
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

            // Get preview from first user message
            const preview = await getSessionPreview(filePath)

            sessions.push({
              id: sessionId,
              project: entry.name,
              projectPath: resolved.fullPath,
              filePath,
              modifiedAt: fileStat.mtime,
              sizeBytes: fileStat.size,
              preview
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

        // Count sessions active in the last hour
        const oneHourAgo = Date.now() - 60 * 60 * 1000
        const activeCount = sessions.filter(s => s.modifiedAt.getTime() > oneHourAgo).length

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
