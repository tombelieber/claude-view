import type { Message, ToolCall } from '../hooks/use-session'

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

  // Links (validate URL scheme and escape quotes for href attribute safety)
  const SAFE_URL_SCHEME = /^https?:\/\//i
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, text, url) => {
    if (!SAFE_URL_SCHEME.test(url)) {
      return text // render as plain text, not a link
    }
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
 * Matches ToolBadge UI component styling
 */
function renderToolCalls(toolCalls: ToolCall[]): string {
  if (!toolCalls || toolCalls.length === 0) return ''

  const totalCount = toolCalls.reduce((sum, tc) => sum + tc.count, 0)
  const badges = toolCalls.map((tc) => `<span class="tool-badge">${escapeHtml(tc.name)}</span>`).join('')

  const toolDetails = toolCalls
    .map(
      (tc) =>
        `<div class="tool-item"><span class="tool-badge">${escapeHtml(tc.name)}</span><span class="tool-count">x${tc.count}</span></div>`
    )
    .join('')

  return `
    <details class="tool-calls">
      <summary class="tool-summary">${badges}<span class="tool-total">${totalCount} ${totalCount === 1 ? 'call' : 'calls'}</span></summary>
      <div class="tool-details">
        ${toolDetails}
      </div>
    </details>
  `
}

/**
 * Formats a timestamp for display
 */
function formatTime(timestamp?: string | null): string {
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
 * Styling matches the React UI exactly for consistency
 */
export function generateStandaloneHtml(messages: Message[]): string {
  const exportTimestamp = new Date().toLocaleString()

  const messagesHtml = messages
    .map((message) => {
      const isUser = message.role === 'user'
      const roleClass = isUser ? 'user' : 'assistant'
      const avatarClass = isUser ? 'avatar-user' : 'avatar-assistant'
      const displayName = isUser ? 'Human' : 'Claude'
      const time = formatTime(message.timestamp)

      return `
      <div class="message ${roleClass}">
        <div class="message-header">
          <div class="${avatarClass}"></div>
          <div class="message-info">
            <div class="message-name-row">
              <span class="message-name">${displayName}</span>
              ${time ? `<span class="message-time">${time}</span>` : ''}
            </div>
          </div>
        </div>
        <div class="message-content">
          ${markdownToHtml(message.content)}
          ${renderToolCalls(message.tool_calls || [])}
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

    html {
      scroll-behavior: smooth;
    }

    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Helvetica Neue', sans-serif;
      line-height: 1.6;
      color: #111827;
      background-color: #ffffff;
      padding: 24px 16px;
    }

    .container {
      max-width: 768px;
      margin: 0 auto;
    }

    /* Header */
    .header {
      text-align: center;
      padding: 0 0 24px 0;
      margin-bottom: 24px;
      border-bottom: 1px solid #e5e7eb;
    }

    .header h1 {
      font-size: 24px;
      font-weight: 700;
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
      background-color: #fafafa;
    }

    .message-header {
      display: flex;
      align-items: flex-start;
      gap: 12px;
      margin-bottom: 12px;
    }

    /* Avatars - match UI component styling */
    .avatar-user,
    .avatar-assistant {
      width: 32px;
      height: 32px;
      border-radius: 4px;
      display: flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;
    }

    .avatar-user {
      background-color: #d1d5db;
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
      font-size: 14px;
    }

    .message-time {
      font-size: 12px;
      color: #9ca3af;
    }

    .message-content {
      padding-left: 44px;
    }

    /* Typography - match prose styling */
    .message-content p {
      margin-bottom: 12px;
    }

    .message-content p:last-child {
      margin-bottom: 0;
    }

    .message-content h1 {
      font-size: 20px;
      font-weight: 700;
      margin-top: 16px;
      margin-bottom: 8px;
      color: #111827;
    }

    .message-content h2 {
      font-size: 18px;
      font-weight: 700;
      margin-top: 12px;
      margin-bottom: 8px;
      color: #111827;
    }

    .message-content h3 {
      font-size: 16px;
      font-weight: 700;
      margin-top: 8px;
      margin-bottom: 4px;
      color: #111827;
    }

    .message-content ul,
    .message-content ol {
      margin-left: 20px;
      margin-bottom: 12px;
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
      color: #6b7280;
      margin: 12px 0;
    }

    /* Code - print-friendly styling */
    .inline-code {
      background-color: #f3f4f6;
      padding: 2px 6px;
      border-radius: 4px;
      font-family: 'SF Mono', Monaco, 'Courier New', monospace;
      font-size: 0.875em;
      color: #374151;
    }

    .code-block {
      background-color: #f9fafb;
      color: #374151;
      padding: 16px;
      border-radius: 8px;
      border: 1px solid #e5e7eb;
      overflow-x: auto;
      margin: 12px 0;
    }

    .code-block code {
      font-family: 'SF Mono', Monaco, 'Courier New', monospace;
      font-size: 13px;
      line-height: 1.5;
      white-space: pre;
    }

    /* Tool Badges - match ToolBadge component */
    .tool-badge {
      display: inline-block;
      background-color: #f3f4f6;
      border: 1px solid #e5e7eb;
      color: #6b7280;
      padding: 2px 8px;
      border-radius: 4px;
      font-size: 12px;
      font-weight: 500;
      margin-right: 6px;
      white-space: nowrap;
    }

    /* Tool calls */
    .tool-calls {
      margin-top: 12px;
      background-color: #f9fafb;
      border: 1px solid #e5e7eb;
      border-radius: 8px;
    }

    .tool-summary {
      padding: 8px 12px;
      cursor: pointer;
      color: #6b7280;
      user-select: none;
      list-style: none;
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: 13px;
    }

    .tool-summary:hover {
      background-color: #f3f4f6;
      border-radius: 8px;
    }

    .tool-summary::marker {
      display: none;
    }

    .tool-calls summary::before {
      content: '▶ ';
      display: inline-block;
      margin-right: 4px;
      font-size: 10px;
    }

    .tool-calls[open] summary::before {
      content: '▼ ';
    }

    .tool-total {
      color: #9ca3af;
      font-size: 12px;
      margin-left: auto;
    }

    .tool-details {
      padding: 8px 12px 12px 32px;
      border-top: 1px solid #e5e7eb;
      display: flex;
      flex-direction: column;
      gap: 4px;
    }

    .tool-item {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 2px 0;
      font-size: 13px;
    }

    .tool-item .tool-badge {
      margin: 0;
    }

    .tool-count {
      color: #9ca3af;
      font-size: 12px;
      margin-left: 4px;
    }

    /* Print optimized styles */
    @media print {
      body {
        background-color: white;
        padding: 0;
        margin: 0;
      }

      .container {
        max-width: 100%;
      }

      .message {
        break-inside: avoid;
        page-break-inside: avoid;
        margin-bottom: 16px;
        border: 1px solid #e5e7eb;
      }

      .header {
        padding: 0 0 16px 0;
      }

      /* Ensure code blocks are readable in print */
      .code-block {
        background-color: #ffffff;
        border: 1px solid #d1d5db;
        break-inside: avoid;
      }

      .tool-calls {
        break-inside: avoid;
        background-color: #ffffff;
      }

      .tool-calls[open] .tool-details {
        display: flex;
      }

      /* Hide hover effects in print */
      .tool-summary:hover {
        background-color: transparent;
      }

      a {
        text-decoration: underline;
      }
    }

    /* Mobile responsive */
    @media (max-width: 640px) {
      body {
        padding: 16px 12px;
      }

      .container {
        max-width: 100%;
      }

      .message-content {
        padding-left: 40px;
      }

      .message-content h1 {
        font-size: 18px;
      }

      .message-content h2 {
        font-size: 16px;
      }

      .message-content h3 {
        font-size: 15px;
      }

      .code-block {
        font-size: 12px;
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

/**
 * Opens a print dialog to save conversation as PDF
 * Uses browser's native print-to-PDF functionality
 * Uses data: URL to avoid document.write() security concerns
 */
export function exportToPdf(messages: Message[]): void {
  const html = generateStandaloneHtml(messages)
  const dataUrl = `data:text/html;charset=utf-8,${encodeURIComponent(html)}`
  const printWindow = window.open(dataUrl, '_blank')
  if (printWindow) {
    // Give the window time to load before triggering print
    setTimeout(() => {
      printWindow.print()
    }, 250)
  }
}
