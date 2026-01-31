import type { Message, ToolCall } from '../hooks/use-session'

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
    if (message.toolCalls && message.toolCalls.length > 0) {
      md += renderToolCalls(message.toolCalls)
    }
    if (message.thinking) {
      md += `\n\n<details>\n<summary>Thinking</summary>\n\n${message.thinking}\n\n</details>`
    }
    md += '\n\n'
  }

  return md
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
