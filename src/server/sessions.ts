import { readdir, stat, open } from 'fs/promises'
import { join } from 'path'
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
  path: string
  sessions: SessionInfo[]
}

const CLAUDE_PROJECTS_DIR = join(homedir(), '.claude', 'projects')

/**
 * Convert project directory name to readable path format
 * e.g., "-Users-name-project" -> "Users/name/project"
 * e.g., "-Users-TBGor-dev--vicky-ai" -> "Users/TBGor/dev/@vicky-ai"
 *
 * LIMITATION: This function cannot perfectly decode paths containing literal hyphens.
 * For example, "@vicky-ai" encodes the same as "@vicky/ai" would. Claude's encoding
 * scheme uses dashes for path separators, making it impossible to distinguish between
 * literal hyphens and path separators without checking the filesystem. We keep the
 * current behavior (treating all single dashes as path separators) as it's the best
 * we can do without filesystem lookups, and most paths don't contain literal hyphens.
 */
function formatProjectName(dirName: string): string {
  // Remove leading dash and replace remaining dashes with slashes
  // Handle double dashes which represent @ symbol in directory encoding
  const formatted = dirName
    .replace(/^-/, '')  // Remove leading dash
    .replace(/--/g, '/@')  // Double dash represents @ (e.g., dev--vicky -> dev/@vicky)
    .replace(/-/g, '/')  // Replace single dashes with slashes

  // Handle edge case of empty string (directory named just "-")
  return formatted || '(unknown)'
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
              projectPath: formatProjectName(entry.name),
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

      // Only include projects that have sessions
      if (sessions.length > 0) {
        // Sort sessions by modified date (newest first)
        sessions.sort((a, b) => b.modifiedAt.getTime() - a.modifiedAt.getTime())

        projects.push({
          name: formatProjectName(entry.name),
          path: projectDirPath,
          sessions
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
