import type { Message, ToolCall } from '../hooks/use-session'

/**
 * Tool icons for display in HTML export
 */
const TOOL_ICONS: Record<string, string> = {
  Read: '&#128196;', // 📄
  Write: '&#9999;', // ✏️
  Edit: '&#128295;', // 🔧
  Bash: '&#128187;', // 💻
  Glob: '&#128269;', // 🔍
  Grep: '&#128270;', // 🔎
}

function getToolIcon(toolName: string): string {
  return TOOL_ICONS[toolName] || '&#128295;' // 🔧
}

/**
 * Escapes HTML special characters to prevent XSS
 */
function escapeHtml(text: string): string {
  const escapeMap: Record<string, string> = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;',
  }
  return text.replace(/[&<>"']/g, (char) => escapeMap[char])
}

/**
 * Converts basic markdown to HTML
 * Handles: headers, code blocks, inline code, bold, italic, lists, links, blockquotes
 */
function markdownToHtml(markdown: string): string {
  let html = escapeHtml(markdown)

  // Code blocks (must be processed before other formatting)
  html = html.replace(
    /```(\w*)\n([\s\S]*?)```/g,
    (_, lang, code) =>
      `<pre class="code-block"><code class="language-${lang || 'text'}">${code.trim()}</code></pre>`
  )

  // Inline code (must be before bold/italic to avoid conflicts)
  html = html.replace(/`([^`]+)`/g, '<code class="inline-code">$1</code>')

  // Headers (must be at start of line)
  html = html.replace(/^### (.+)$/gm, '<h3>$1</h3>')
  html = html.replace(/^## (.+)$/gm, '<h2>$1</h2>')
  html = html.replace(/^# (.+)$/gm, '<h1>$1</h1>')

  // Bold and italic
  html = html.replace(/\*\*\*(.+?)\*\*\*/g, '<strong><em>$1</em></strong>')
  html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
  html = html.replace(/\*(.+?)\*/g, '<em>$1</em>')
  html = html.replace(/___(.+?)___/g, '<strong><em>$1</em></strong>')
  html = html.replace(/__(.+?)__/g, '<strong>$1</strong>')
  html = html.replace(/_(.+?)_/g, '<em>$1</em>')

  // Blockquotes
  html = html.replace(/^&gt; (.+)$/gm, '<blockquote>$1</blockquote>')

  // Links (escape quotes in URL for href attribute safety)
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, text, url) => {
    const safeUrl = url.replace(/"/g, '&quot;')
    return `<a href="${safeUrl}" target="_blank" rel="noopener noreferrer">${text}</a>`
  })

  // Unordered lists - mark with data attribute first, then wrap
  html = html.replace(/^- (.+)$/gm, '<li data-list="ul">$1</li>')
  html = html.replace(/^\* (.+)$/gm, '<li data-list="ul">$1</li>')
  html = html.replace(
    /(<li data-list="ul">[\s\S]*?<\/li>\n?)+/g,
    (match) => `<ul>${match.replace(/ data-list="ul"/g, '')}</ul>`
  )

  // Ordered lists - mark with data attribute first, then wrap
  html = html.replace(/^\d+\. (.+)$/gm, '<li data-list="ol">$1</li>')
  html = html.replace(
    /(<li data-list="ol">[\s\S]*?<\/li>\n?)+/g,
    (match) => `<ol>${match.replace(/ data-list="ol"/g, '')}</ol>`
  )

  // Paragraphs (convert double newlines to paragraph breaks)
  html = html
    .split('\n\n')
    .map((block) => {
      // Don't wrap blocks that are already HTML elements
      if (
        block.startsWith('<h') ||
        block.startsWith('<ul') ||
        block.startsWith('<ol') ||
        block.startsWith('<pre') ||
        block.startsWith('<blockquote')
      ) {
        return block
      }
      // Wrap text blocks in paragraphs
      if (block.trim()) {
        return `<p>${block.replace(/\n/g, '<br>')}</p>`
      }
      return ''
    })
    .join('\n')

  return html
}

/**
 * Renders tool calls as a collapsible details element
 */
function renderToolCalls(toolCalls: ToolCall[]): string {
  if (!toolCalls || toolCalls.length === 0) return ''

  const totalCount = toolCalls.reduce((sum, tc) => sum + tc.count, 0)
  const summaryParts = toolCalls.map((tc) => `${getToolIcon(tc.name)} ${tc.name}`)
  const summaryText = summaryParts.join(', ')

  const toolDetails = toolCalls
    .map(
      (tc) =>
        `<div class="tool-item">${getToolIcon(tc.name)} ${escapeHtml(tc.name)} <span class="tool-count">x ${tc.count}</span></div>`
    )
    .join('\n')

  return `
    <details class="tool-calls">
      <summary>${summaryText} <span class="tool-count">(${totalCount} ${totalCount === 1 ? 'call' : 'calls'})</span></summary>
      <div class="tool-details">
        ${toolDetails}
      </div>
    </details>
  `
}

/**
 * Formats a timestamp for display
 */
function formatTime(timestamp?: string): string {
  if (!timestamp) return ''
  const date = new Date(timestamp)
  return date.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })
}

/**
 * Generates a standalone HTML document from conversation messages
 */
export function generateStandaloneHtml(messages: Message[]): string {
  const exportTimestamp = new Date().toLocaleString()

  const messagesHtml = messages
    .map((message) => {
      const isUser = message.role === 'user'
      const roleClass = isUser ? 'user' : 'assistant'
      const avatarClass = isUser ? 'avatar-user' : 'avatar-assistant'
      const avatarLetter = isUser ? 'U' : 'C'
      const displayName = isUser ? 'You' : 'Claude'
      const time = formatTime(message.timestamp)

      return `
      <div class="message ${roleClass}">
        <div class="message-header">
          <div class="${avatarClass}">${avatarLetter}</div>
          <div class="message-info">
            <div class="message-name-row">
              <span class="message-name">${displayName}</span>
              ${time ? `<span class="message-time">${time}</span>` : ''}
            </div>
          </div>
        </div>
        <div class="message-content">
          ${markdownToHtml(message.content)}
          ${renderToolCalls(message.toolCalls || [])}
        </div>
      </div>
    `
    })
    .join('\n')

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Claude Conversation</title>
  <style>
    * {
      box-sizing: border-box;
      margin: 0;
      padding: 0;
    }

    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
      line-height: 1.6;
      color: #1f2937;
      background-color: #f9fafb;
      padding: 20px;
    }

    .container {
      max-width: 800px;
      margin: 0 auto;
    }

    /* Header */
    .header {
      text-align: center;
      padding: 20px 0;
      margin-bottom: 24px;
      border-bottom: 1px solid #e5e7eb;
    }

    .header h1 {
      font-size: 24px;
      font-weight: 600;
      color: #111827;
      margin-bottom: 8px;
    }

    .header .timestamp {
      font-size: 14px;
      color: #6b7280;
    }

    /* Messages */
    .messages {
      display: flex;
      flex-direction: column;
      gap: 16px;
    }

    .message {
      padding: 16px;
      border-radius: 8px;
    }

    .message.user {
      background-color: #ffffff;
      border: 1px solid #e5e7eb;
    }

    .message.assistant {
      background-color: #f9fafb;
    }

    .message-header {
      display: flex;
      align-items: flex-start;
      gap: 12px;
      margin-bottom: 12px;
    }

    .avatar-user,
    .avatar-assistant {
      width: 32px;
      height: 32px;
      border-radius: 4px;
      display: flex;
      align-items: center;
      justify-content: center;
      color: white;
      font-weight: 600;
      font-size: 14px;
      flex-shrink: 0;
    }

    .avatar-user {
      background-color: #3b82f6;
    }

    .avatar-assistant {
      background-color: #f97316;
    }

    .message-info {
      flex: 1;
      min-width: 0;
    }

    .message-name-row {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 8px;
    }

    .message-name {
      font-weight: 500;
      color: #111827;
    }

    .message-time {
      font-size: 12px;
      color: #9ca3af;
    }

    .message-content {
      padding-left: 44px;
    }

    /* Typography */
    .message-content p {
      margin-bottom: 8px;
    }

    .message-content p:last-child {
      margin-bottom: 0;
    }

    .message-content h1 {
      font-size: 20px;
      font-weight: 700;
      margin: 16px 0 8px 0;
    }

    .message-content h2 {
      font-size: 18px;
      font-weight: 700;
      margin: 12px 0 8px 0;
    }

    .message-content h3 {
      font-size: 16px;
      font-weight: 700;
      margin: 8px 0 4px 0;
    }

    .message-content ul,
    .message-content ol {
      margin-left: 20px;
      margin-bottom: 8px;
    }

    .message-content li {
      margin-bottom: 4px;
    }

    .message-content a {
      color: #3b82f6;
      text-decoration: underline;
    }

    .message-content a:hover {
      color: #1d4ed8;
    }

    .message-content blockquote {
      border-left: 4px solid #d1d5db;
      padding-left: 16px;
      font-style: italic;
      color: #4b5563;
      margin: 8px 0;
    }

    /* Code */
    .inline-code {
      background-color: #f3f4f6;
      padding: 2px 6px;
      border-radius: 4px;
      font-family: 'SF Mono', Monaco, 'Courier New', monospace;
      font-size: 0.875em;
    }

    .code-block {
      background-color: #1f2937;
      color: #f9fafb;
      padding: 16px;
      border-radius: 8px;
      overflow-x: auto;
      margin: 8px 0;
    }

    .code-block code {
      font-family: 'SF Mono', Monaco, 'Courier New', monospace;
      font-size: 13px;
      line-height: 1.5;
      white-space: pre;
    }

    /* Tool calls */
    .tool-calls {
      margin-top: 12px;
      background-color: #f3f4f6;
      border: 1px solid #e5e7eb;
      border-radius: 8px;
      font-size: 14px;
    }

    .tool-calls summary {
      padding: 8px 12px;
      cursor: pointer;
      color: #4b5563;
      user-select: none;
    }

    .tool-calls summary:hover {
      background-color: #e5e7eb;
      border-radius: 8px;
    }

    .tool-details {
      padding: 8px 12px 12px 24px;
      border-top: 1px solid #e5e7eb;
    }

    .tool-item {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 4px 0;
      color: #4b5563;
    }

    .tool-count {
      color: #9ca3af;
    }

    /* Print styles */
    @media print {
      body {
        background-color: white;
        padding: 0;
      }

      .message {
        break-inside: avoid;
        page-break-inside: avoid;
      }

      .message.user,
      .message.assistant {
        border: 1px solid #e5e7eb;
      }

      .code-block {
        background-color: #f3f4f6 !important;
        color: #1f2937 !important;
        border: 1px solid #e5e7eb;
      }

      .tool-calls {
        break-inside: avoid;
      }

      .tool-calls[open] .tool-details {
        display: block;
      }
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="header">
      <h1>Claude Conversation</h1>
      <p class="timestamp">Exported on ${escapeHtml(exportTimestamp)}</p>
    </div>
    <div class="messages">
      ${messagesHtml}
    </div>
  </div>
</body>
</html>`
}

/**
 * Triggers a download of the HTML content as a file
 */
export function downloadHtml(html: string, filename: string): void {
  const blob = new Blob([html], { type: 'text/html;charset=utf-8' })
  const url = URL.createObjectURL(blob)

  const link = document.createElement('a')
  link.href = url
  link.download = filename
  document.body.appendChild(link)
  link.click()

  // Cleanup
  document.body.removeChild(link)
  URL.revokeObjectURL(url)
}
