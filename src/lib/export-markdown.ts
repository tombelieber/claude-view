import type { Message, ToolCall } from '../hooks/use-session'
import type { SessionDetail } from '../types/generated'

function formatTimestamp(timestamp?: string | null): string {
  if (!timestamp) return ''
  const date = new Date(timestamp)
  return date.toLocaleString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })
}

function renderToolCalls(toolCalls?: ToolCall[] | null): string {
  if (!toolCalls || toolCalls.length === 0) return ''
  const items = toolCalls.map((tc) => `- **${tc.name}** (x${tc.count})`).join('\n')
  return `\n\n**Tools Used:**\n${items}`
}

export function generateMarkdown(
  messages: Message[],
  projectName?: string,
  sessionId?: string,
): string {
  const exportDate = formatTimestamp(new Date().toISOString())
  const userCount = messages.filter((m) => m.role === 'user').length
  const assistantCount = messages.filter((m) => m.role === 'assistant').length

  let md = '# Conversation Export\n\n'
  if (projectName) md += `**Project:** ${projectName}  \n`
  if (sessionId) md += `**Session:** ${sessionId}  \n`
  md += `**Exported:** ${exportDate}  \n`
  md += `**Messages:** ${messages.length} (${userCount} user, ${assistantCount} assistant)\n\n`
  md += '---\n\n'

  let turnNumber = 0
  for (let i = 0; i < messages.length; i++) {
    const message = messages[i]
    const isUser = message.role === 'user'

    if (isUser || i === 0) {
      turnNumber++
      md += `## Turn ${turnNumber}\n\n`
    }

    const roleLabel = isUser ? 'User' : 'Assistant'
    const ts = formatTimestamp(message.timestamp)
    md += `**${roleLabel}:**${ts ? ` *${ts}*` : ''}\n\n`
    md += message.content
    if (message.tool_calls && message.tool_calls.length > 0) {
      md += renderToolCalls(message.tool_calls)
    }
    if (message.thinking) {
      md += `\n\n<details>\n<summary>Thinking</summary>\n\n${message.thinking}\n\n</details>`
    }
    md += '\n\n'
  }

  return md
}

/** Truncate text at a word boundary, avoiding breaks inside code blocks. */
function truncateAtSafePoint(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text

  // If a code block starts before maxLen but doesn't close, truncate before it
  const codeBlockStart = text.indexOf('```')
  if (codeBlockStart !== -1 && codeBlockStart < maxLen) {
    const codeBlockEnd = text.indexOf('```', codeBlockStart + 3)
    if (codeBlockEnd === -1 || codeBlockEnd > maxLen) {
      const before = text.slice(0, codeBlockStart).trimEnd()
      return before.length > 0 ? before + '...' : text.slice(0, maxLen) + '...'
    }
  }

  // Break at word boundary
  const truncated = text.slice(0, maxLen)
  const lastSpace = truncated.lastIndexOf(' ')
  return (lastSpace > maxLen * 0.5 ? truncated.slice(0, lastSpace) : truncated) + '...'
}

/**
 * Generates a condensed context prompt optimized for pasting into a new
 * Claude Code session to "continue" an old conversation.
 *
 * Design constraints:
 * - Target ~200-500 tokens (not 50K like full markdown export)
 * - Structured for LLM consumption, not human reading
 * - Graceful fallbacks when fields are null
 */
export function generateResumeContext(
  messages: Message[],
  detail: SessionDetail,
): string {
  const sections: string[] = []

  // Header
  sections.push('I want to continue a previous conversation. Here is the context:\n')

  // Project context
  const projectLine = `**Project:** \`${detail.projectPath}\``
  const branchLine = detail.gitBranch ? ` (branch: \`${detail.gitBranch}\`)` : ''
  sections.push(projectLine + branchLine)

  // Summary — use Claude Code's auto-summary if present (may be null), else fall back to preview
  const summaryText = detail.summary || detail.preview || ''
  if (summaryText) {
    sections.push(`**What I was doing:** ${summaryText}`)
  }

  // Category if available
  if (detail.categoryL1) {
    const cats = [detail.categoryL1, detail.categoryL2, detail.categoryL3]
      .filter(Boolean)
      .join(' > ')
    sections.push(`**Task type:** ${cats}`)
  }

  // Files modified (deduplicated, max 15)
  const uniqueEdited = [...new Set(detail.filesEdited)]
  if (uniqueEdited.length > 0) {
    const fileList = uniqueEdited.slice(0, 15).map(f => `- \`${f}\``).join('\n')
    const suffix = uniqueEdited.length > 15
      ? `\n- ... and ${uniqueEdited.length - 15} more`
      : ''
    sections.push(`**Files modified:**\n${fileList}${suffix}`)
  }

  // Files read (top 10, excluding already-listed edited files)
  const editedSet = new Set(uniqueEdited)
  const readOnly = [...new Set(detail.filesRead)].filter(f => !editedSet.has(f))
  if (readOnly.length > 0) {
    const fileList = readOnly.slice(0, 10).map(f => `- \`${f}\``).join('\n')
    const suffix = readOnly.length > 10
      ? `\n- ... and ${readOnly.length - 10} more`
      : ''
    sections.push(`**Files referenced:**\n${fileList}${suffix}`)
  }

  // Last few conversation turns (user + assistant only, max 5 exchanges)
  const conversational = messages.filter(
    m => m.role === 'user' || m.role === 'assistant'
  )
  // Take last 10 messages (up to 5 exchanges)
  const recentMessages = conversational.slice(-10)
  if (recentMessages.length > 0) {
    const turnLines = recentMessages.map(m => {
      const role = m.role === 'user' ? 'User' : 'Assistant'
      // Truncate long messages at word boundary, avoid breaking code blocks
      const content = truncateAtSafePoint(m.content, 200)
      return `**${role}:** ${content}`
    })
    sections.push(`**Recent conversation:**\n${turnLines.join('\n\n')}`)
  }

  sections.push(
    recentMessages.length > 0
      ? '---\nPlease continue from where we left off.'
      : '---\nPlease help me with this project based on the context above.'
  )

  return sections.join('\n\n')
}

export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    return false
  }
}

export function downloadMarkdown(markdown: string, filename: string): void {
  const blob = new Blob([markdown], { type: 'text/markdown;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  URL.revokeObjectURL(url)
}
