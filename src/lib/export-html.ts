import type { Message } from '../types/generated/Message'
import type { ToolCall } from '../types/generated/ToolCall'
import { buildThreadMap, type ThreadInfo } from './thread-map'

// ---------------------------------------------------------------------------
// ExportMetadata
// ---------------------------------------------------------------------------

export interface ExportMetadata {
  sessionId: string
  projectName: string
  projectPath?: string
  primaryModel?: string | null
  durationSeconds?: number
  totalInputTokens?: number | null
  totalOutputTokens?: number | null
  messageCount: number
  userPromptCount: number
  toolCallCount: number
  filesEditedCount?: number
  filesReadCount?: number
  commitCount?: number
  gitBranch?: string | null
  exportDate: string // ISO string
}

// ---------------------------------------------------------------------------
// Export-side TYPE_CONFIG
// ---------------------------------------------------------------------------

const EXPORT_TYPE_CONFIG: Record<string, { label: string; iconId: string }> = {
  user:        { label: 'You',      iconId: 'user' },
  assistant:   { label: 'Claude',   iconId: 'assistant' },
  tool_use:    { label: 'Tool',     iconId: 'tool-use' },
  tool_result: { label: 'Result',   iconId: 'tool-result' },
  system:      { label: 'System',   iconId: 'system' },
  progress:    { label: 'Progress', iconId: 'progress' },
  summary:     { label: 'Summary',  iconId: 'summary' },
}

// ---------------------------------------------------------------------------
// Inline SVG icon definitions (Lucide)
// ---------------------------------------------------------------------------

const SVG_DEFS = `<svg style="display:none"><defs>
  <symbol id="icon-user" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/>
    <circle cx="12" cy="7" r="4"/>
  </symbol>
  <symbol id="icon-assistant" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
  </symbol>
  <symbol id="icon-tool-use" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>
  </symbol>
  <symbol id="icon-tool-result" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
    <polyline points="22 4 12 14.01 9 11.01"/>
  </symbol>
  <symbol id="icon-system" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <circle cx="12" cy="12" r="10"/>
    <line x1="12" y1="8" x2="12" y2="12"/>
    <line x1="12" y1="16" x2="12.01" y2="16"/>
  </symbol>
  <symbol id="icon-progress" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
  </symbol>
  <symbol id="icon-summary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"/>
    <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"/>
  </symbol>
  <symbol id="icon-brain" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M9.5 2A2.5 2.5 0 0 1 12 4.5v15a2.5 2.5 0 0 1-4.96.44 2.5 2.5 0 0 1-2.96-3.08 3 3 0 0 1-.34-5.58 2.5 2.5 0 0 1 1.32-4.24 2.5 2.5 0 0 1 1.98-3A2.5 2.5 0 0 1 9.5 2Z"/>
    <path d="M14.5 2A2.5 2.5 0 0 0 12 4.5v15a2.5 2.5 0 0 0 4.96.44 2.5 2.5 0 0 0 2.96-3.08 3 3 0 0 0 .34-5.58 2.5 2.5 0 0 0-1.32-4.24 2.5 2.5 0 0 0-1.98-3A2.5 2.5 0 0 0 14.5 2Z"/>
  </symbol>
  <symbol id="icon-sun" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <circle cx="12" cy="12" r="5"/>
    <line x1="12" y1="1" x2="12" y2="3"/>
    <line x1="12" y1="21" x2="12" y2="23"/>
    <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/>
    <line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/>
    <line x1="1" y1="12" x2="3" y2="12"/>
    <line x1="21" y1="12" x2="23" y2="12"/>
    <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/>
    <line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>
  </symbol>
  <symbol id="icon-moon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>
  </symbol>
</defs></svg>`

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

function icon(name: string, cls?: string): string {
  return `<svg class="icon${cls ? ' ' + cls : ''}" width="16" height="16"><use href="#icon-${name}"/></svg>`
}

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  if (h > 0) return `${h}h ${m}m`
  return s > 0 ? `${m}m ${s}s` : `${m}m`
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

function formatTime(timestamp?: string | null): string {
  if (!timestamp) return ''
  const date = new Date(timestamp)
  return date.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })
}

// ---------------------------------------------------------------------------
// Smart message filtering (matches ConversationView.tsx lines 26-44)
// ---------------------------------------------------------------------------

const EMPTY_CONTENT = new Set(['(no content)', ''])

function filterExportMessages(messages: Message[]): Message[] {
  return messages.filter(msg => {
    if (msg.role === 'user') return true
    if (msg.role === 'assistant') {
      // Hide assistant messages with no real content (only tool calls, no text)
      // BUT keep assistant messages that have thinking blocks
      if (EMPTY_CONTENT.has(msg.content.trim()) && !msg.thinking) return false
      return true
    }
    // Filter out tool_use, tool_result, system, progress, summary
    return false
  })
}

// ---------------------------------------------------------------------------
// Markdown to HTML
// ---------------------------------------------------------------------------

/**
 * Converts basic markdown to HTML.
 * Processing order is critical — do not rearrange.
 *
 * 1. escapeHtml() FIRST — prevents XSS
 * 2. Code blocks (triple backtick) — before other formatting
 * 3. Inline code (single backtick) — before bold/italic
 * 4. Headers (#, ##, ###)
 * 5. Bold/italic (**, *, __, _)
 * 6. Blockquotes (>)
 * 7. Horizontal rules (---, ***, ___)
 * 8. Links ([text](url)) with SAFE_URL_SCHEME
 * 9. Lists (- / * for ul, 1. for ol) — data-attribute markers
 * 10. Paragraphs (double newline → <p>)
 */
function markdownToHtml(markdown: string): string {
  let html = escapeHtml(markdown)

  // Code blocks (must be processed before other formatting)
  html = html.replace(
    /```(\w*)\n([\s\S]*?)```/g,
    (_, lang, code) => {
      const langLabel = lang || 'text'
      return `<div class="code-block"><div class="code-header">${escapeHtml(langLabel)}</div><pre><code class="language-${langLabel}">${code.trim()}</code></pre></div>`
    }
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

  // Horizontal rules
  html = html.replace(/^(---|&ast;&ast;&ast;|___)$/gm, '<hr>')

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
        block.startsWith('<div class="code-block') ||
        block.startsWith('<blockquote') ||
        block.startsWith('<hr')
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

// ---------------------------------------------------------------------------
// Tool calls rendering
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Thinking block rendering
// ---------------------------------------------------------------------------

function renderThinkingBlock(thinking: string): string {
  const trimmed = thinking.trim()
  if (!trimmed) return ''

  // Preview: first ~80 chars at word boundary
  let preview = trimmed.substring(0, 80)
  if (trimmed.length > 80) {
    const lastSpace = preview.lastIndexOf(' ')
    if (lastSpace > 40) preview = preview.substring(0, lastSpace)
    preview += '...'
  }

  return `
    <details class="thinking-block">
      <summary class="thinking-summary">
        ${icon('brain', 'thinking-icon')}
        <span class="thinking-label">Thinking</span>
        <span class="thinking-preview">${escapeHtml(preview)}</span>
      </summary>
      <div class="thinking-content">${escapeHtml(trimmed).replace(/\n/g, '<br>')}</div>
    </details>
  `
}

// ---------------------------------------------------------------------------
// Metadata header rendering
// ---------------------------------------------------------------------------

function renderMetadataHeader(meta: ExportMetadata): string {
  const items: string[] = []

  if (meta.primaryModel) {
    items.push(`<div class="meta-item"><span class="meta-label">Model</span><span class="meta-value">${escapeHtml(meta.primaryModel)}</span></div>`)
  }
  if (meta.durationSeconds && meta.durationSeconds > 0) {
    items.push(`<div class="meta-item"><span class="meta-label">Duration</span><span class="meta-value">${formatDuration(meta.durationSeconds)}</span></div>`)
  }
  if (meta.totalInputTokens != null && meta.totalOutputTokens != null) {
    const total = meta.totalInputTokens + meta.totalOutputTokens
    items.push(`<div class="meta-item"><span class="meta-label">Tokens</span><span class="meta-value">${formatTokens(total)}</span></div>`)
  }
  items.push(`<div class="meta-item"><span class="meta-label">Messages</span><span class="meta-value">${meta.messageCount}</span></div>`)
  if (meta.userPromptCount > 0) {
    items.push(`<div class="meta-item"><span class="meta-label">Prompts</span><span class="meta-value">${meta.userPromptCount}</span></div>`)
  }
  if (meta.toolCallCount > 0) {
    items.push(`<div class="meta-item"><span class="meta-label">Tool Calls</span><span class="meta-value">${meta.toolCallCount}</span></div>`)
  }
  if (meta.filesEditedCount && meta.filesEditedCount > 0) {
    items.push(`<div class="meta-item"><span class="meta-label">Files Edited</span><span class="meta-value">${meta.filesEditedCount}</span></div>`)
  }
  if (meta.filesReadCount && meta.filesReadCount > 0) {
    items.push(`<div class="meta-item"><span class="meta-label">Files Read</span><span class="meta-value">${meta.filesReadCount}</span></div>`)
  }
  if (meta.commitCount && meta.commitCount > 0) {
    items.push(`<div class="meta-item"><span class="meta-label">Commits</span><span class="meta-value">${meta.commitCount}</span></div>`)
  }

  const branchBadge = meta.gitBranch
    ? `<span class="branch-badge">${escapeHtml(meta.gitBranch)}</span>`
    : ''

  const exportDate = new Date(meta.exportDate).toLocaleString()

  return `
    <div class="doc-header">
      <div class="doc-title-row">
        <h1 class="doc-title">${escapeHtml(meta.projectName)}</h1>
        ${branchBadge}
      </div>
      ${meta.projectPath ? `<div class="doc-path">${escapeHtml(meta.projectPath)}</div>` : ''}
      <div class="meta-grid">
        ${items.join('\n        ')}
      </div>
      <div class="doc-export-date">Exported ${escapeHtml(exportDate)}</div>
    </div>
  `
}

// ---------------------------------------------------------------------------
// Per-message rendering
// ---------------------------------------------------------------------------

function renderMessage(message: Message, thread?: ThreadInfo): string {
  const role = message.role || 'assistant'
  const config = EXPORT_TYPE_CONFIG[role] || EXPORT_TYPE_CONFIG.assistant
  const time = formatTime(message.timestamp)
  const indent = thread?.indent ?? 0
  const isChild = thread?.isChild ?? false
  const indentPx = indent * 12

  const roleClass = `message--${role}`
  const threadClass = isChild ? ' message--threaded' : ''
  const indentStyle = indentPx > 0 ? ` style="margin-left:${indentPx}px"` : ''

  const thinkingHtml = (message.thinking && message.thinking.trim())
    ? renderThinkingBlock(message.thinking)
    : ''

  const contentHtml = message.content.trim()
    ? markdownToHtml(message.content)
    : ''

  const toolCallsHtml = renderToolCalls(message.tool_calls || [])

  return `
    <div class="message ${roleClass}${threadClass}"${indentStyle}>
      <div class="message-header">
        <div class="message-icon message-icon--${role}">
          ${icon(config.iconId)}
        </div>
        <div class="message-meta">
          <span class="message-label">${config.label}</span>
          ${time ? `<span class="message-time">${time}</span>` : ''}
        </div>
      </div>
      <div class="message-content">
        ${thinkingHtml}
        ${contentHtml}
        ${toolCallsHtml}
      </div>
    </div>
  `
}

// ---------------------------------------------------------------------------
// CSS with theme variables
// ---------------------------------------------------------------------------

const CSS = `
@import url('https://fonts.googleapis.com/css2?family=Fira+Code:wght@400;500;600&family=Fira+Sans:wght@400;500;600;700&display=swap');

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

html {
  scroll-behavior: smooth;
}

/* Light theme (default) */
:root {
  --bg-primary: #ffffff;
  --bg-secondary: #f8fafc;
  --bg-tertiary: #f1f5f9;
  --text-primary: #0f172a;
  --text-secondary: #475569;
  --text-muted: #94a3b8;
  --border-primary: #e2e8f0;
  --border-secondary: #cbd5e1;
  --link-color: #3b82f6;
  --link-hover: #1d4ed8;

  /* Accent colors (matching TYPE_CONFIG in MessageTyped.tsx) */
  --accent-user: #3b82f6;
  --accent-assistant: #f97316;
  --accent-tool_use: #a855f7;
  --accent-tool_result: #22c55e;
  --accent-system: #f59e0b;
  --accent-progress: #6366f1;
  --accent-summary: #f43f5e;

  /* Badge backgrounds (light pastels) */
  --badge-user: #dbeafe;
  --badge-assistant: #ffedd5;
  --badge-tool_use: #f3e8ff;
  --badge-tool_result: #dcfce7;
  --badge-system: #fef3c7;
  --badge-progress: #e0e7ff;
  --badge-summary: #ffe4e6;

  /* Code (always dark) */
  --code-bg: #1e1e2e;
  --code-text: #cdd6f4;
  --code-border: #313244;
  --code-header-bg: #181825;

  /* Thinking */
  --thinking-bg: #eef2ff;
  --thinking-border: #c7d2fe;
  --thinking-text: #4338ca;

  /* Branch badge */
  --branch-bg: #f0fdf4;
  --branch-text: #166534;
  --branch-border: #bbf7d0;
}

/* Dark theme */
[data-theme="dark"] {
  --bg-primary: #0f172a;
  --bg-secondary: #1e293b;
  --bg-tertiary: #334155;
  --text-primary: #f8fafc;
  --text-secondary: #cbd5e1;
  --text-muted: #64748b;
  --border-primary: #334155;
  --border-secondary: #475569;
  --link-color: #60a5fa;
  --link-hover: #93bbfd;

  /* Badge backgrounds (dark mode) */
  --badge-user: rgba(59,130,246,0.15);
  --badge-assistant: rgba(249,115,22,0.15);
  --badge-tool_use: rgba(168,85,247,0.15);
  --badge-tool_result: rgba(34,197,94,0.15);
  --badge-system: rgba(245,158,11,0.15);
  --badge-progress: rgba(99,102,241,0.15);
  --badge-summary: rgba(244,63,94,0.15);

  /* Thinking */
  --thinking-bg: rgba(99,102,241,0.1);
  --thinking-border: #6366f1;
  --thinking-text: #a5b4fc;

  /* Branch badge */
  --branch-bg: rgba(34,197,94,0.15);
  --branch-text: #86efac;
  --branch-border: rgba(34,197,94,0.3);
}

body {
  font-family: 'Fira Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-primary);
  background-color: var(--bg-secondary);
  padding: 24px 16px;
}

.container {
  max-width: 800px;
  margin: 0 auto;
}

/* Icon base */
.icon {
  display: inline-block;
  vertical-align: middle;
  flex-shrink: 0;
}

/* --- Document Header --- */
.doc-header {
  background: var(--bg-primary);
  border: 1px solid var(--border-primary);
  border-radius: 12px;
  padding: 24px;
  margin-bottom: 24px;
}

.doc-title-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 8px;
}

.doc-title {
  font-size: 24px;
  font-weight: 700;
  color: var(--text-primary);
  margin: 0;
}

.branch-badge {
  display: inline-flex;
  align-items: center;
  padding: 2px 10px;
  border-radius: 9999px;
  font-size: 12px;
  font-weight: 500;
  font-family: 'Fira Code', monospace;
  background: var(--branch-bg);
  color: var(--branch-text);
  border: 1px solid var(--branch-border);
}

.doc-path {
  font-size: 12px;
  color: var(--text-muted);
  font-family: 'Fira Code', monospace;
  margin-bottom: 16px;
}

.meta-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 12px;
  margin: 16px 0;
}

.meta-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.meta-label {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
}

.meta-value {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
}

.doc-export-date {
  font-size: 12px;
  color: var(--text-muted);
  padding-top: 12px;
  border-top: 1px solid var(--border-primary);
}

/* --- Messages --- */
.messages {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.message {
  border-left: 4px solid transparent;
  border-radius: 0 8px 8px 0;
  padding: 16px;
  background: var(--bg-primary);
}

.message--user { border-left-color: var(--accent-user); }
.message--assistant { border-left-color: var(--accent-assistant); }
.message--tool_use { border-left-color: var(--accent-tool_use); }
.message--tool_result { border-left-color: var(--accent-tool_result); }
.message--system { border-left-color: var(--accent-system); }
.message--progress { border-left-color: var(--accent-progress); }
.message--summary { border-left-color: var(--accent-summary); }

.message--threaded {
  border-left-style: dashed;
  border-left-color: var(--text-muted);
}

.message-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.message-icon {
  width: 32px;
  height: 32px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.message-icon--user { background: var(--badge-user); color: var(--accent-user); }
.message-icon--assistant { background: var(--badge-assistant); color: var(--accent-assistant); }
.message-icon--tool_use { background: var(--badge-tool_use); color: var(--accent-tool_use); }
.message-icon--tool_result { background: var(--badge-tool_result); color: var(--accent-tool_result); }
.message-icon--system { background: var(--badge-system); color: var(--accent-system); }
.message-icon--progress { background: var(--badge-progress); color: var(--accent-progress); }
.message-icon--summary { background: var(--badge-summary); color: var(--accent-summary); }

.message-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex: 1;
  min-width: 0;
}

.message-label {
  font-weight: 600;
  font-size: 14px;
  color: var(--text-primary);
}

.message-time {
  font-size: 12px;
  color: var(--text-muted);
}

.message-content {
  padding-left: 44px;
}

/* --- Typography --- */
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
  color: var(--text-primary);
}

.message-content h2 {
  font-size: 18px;
  font-weight: 700;
  margin-top: 12px;
  margin-bottom: 8px;
  color: var(--text-primary);
}

.message-content h3 {
  font-size: 16px;
  font-weight: 700;
  margin-top: 8px;
  margin-bottom: 4px;
  color: var(--text-primary);
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
  color: var(--link-color);
  text-decoration: underline;
}

.message-content a:hover {
  color: var(--link-hover);
}

.message-content blockquote {
  border-left: 4px solid var(--border-secondary);
  padding-left: 16px;
  font-style: italic;
  color: var(--text-secondary);
  margin: 12px 0;
}

.message-content hr {
  border: none;
  border-top: 1px solid var(--border-primary);
  margin: 16px 0;
}

.message-content strong {
  font-weight: 600;
}

/* --- Code (always dark, VS Code-like) --- */
.inline-code {
  background-color: var(--bg-tertiary);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: 'Fira Code', 'SF Mono', Monaco, 'Courier New', monospace;
  font-size: 0.85em;
  color: var(--text-primary);
}

.code-block {
  background-color: var(--code-bg);
  border: 1px solid var(--code-border);
  border-radius: 8px;
  overflow: hidden;
  margin: 12px 0;
}

.code-header {
  background-color: var(--code-header-bg);
  padding: 6px 16px;
  font-size: 12px;
  font-weight: 500;
  color: #a6adc8;
  font-family: 'Fira Code', monospace;
  border-bottom: 1px solid var(--code-border);
  text-transform: lowercase;
}

.code-block pre {
  margin: 0;
  padding: 16px;
  overflow-x: auto;
}

.code-block code {
  font-family: 'Fira Code', 'SF Mono', Monaco, 'Courier New', monospace;
  font-size: 13px;
  line-height: 1.5;
  color: var(--code-text);
  white-space: pre;
}

/* --- Thinking Blocks --- */
.thinking-block {
  background: var(--thinking-bg);
  border: 1px solid var(--thinking-border);
  border-radius: 8px;
  margin-bottom: 12px;
  overflow: hidden;
}

.thinking-summary {
  padding: 10px 16px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--thinking-text);
  user-select: none;
  list-style: none;
}

.thinking-summary::-webkit-details-marker {
  display: none;
}

.thinking-summary::before {
  content: '\\25B6';
  font-size: 10px;
  transition: transform 0.2s;
}

.thinking-block[open] .thinking-summary::before {
  transform: rotate(90deg);
}

.thinking-icon {
  color: var(--thinking-text);
}

.thinking-label {
  font-weight: 600;
}

.thinking-preview {
  color: var(--text-muted);
  font-style: italic;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.thinking-content {
  padding: 12px 16px;
  border-top: 1px solid var(--thinking-border);
  font-size: 13px;
  color: var(--text-secondary);
  line-height: 1.6;
  white-space: pre-wrap;
}

/* --- Tool Badges --- */
.tool-badge {
  display: inline-block;
  background-color: var(--bg-tertiary);
  border: 1px solid var(--border-primary);
  color: var(--text-secondary);
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 500;
  font-family: 'Fira Code', monospace;
  margin-right: 6px;
  white-space: nowrap;
}

.tool-calls {
  margin-top: 12px;
  background-color: var(--bg-secondary);
  border: 1px solid var(--border-primary);
  border-radius: 8px;
}

.tool-summary {
  padding: 8px 12px;
  cursor: pointer;
  color: var(--text-secondary);
  user-select: none;
  list-style: none;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
}

.tool-summary::-webkit-details-marker {
  display: none;
}

.tool-summary:hover {
  background-color: var(--bg-tertiary);
  border-radius: 8px;
}

.tool-calls summary::before {
  content: '\\25B6 ';
  display: inline-block;
  margin-right: 4px;
  font-size: 10px;
}

.tool-calls[open] summary::before {
  content: '\\25BC ';
}

.tool-total {
  color: var(--text-muted);
  font-size: 12px;
  margin-left: auto;
}

.tool-details {
  padding: 8px 12px 12px 32px;
  border-top: 1px solid var(--border-primary);
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
  color: var(--text-muted);
  font-size: 12px;
  margin-left: 4px;
}

/* --- Footer --- */
.doc-footer {
  text-align: center;
  padding: 24px 0;
  font-size: 12px;
  color: var(--text-muted);
}

.doc-footer a {
  color: var(--link-color);
  text-decoration: none;
}

.doc-footer a:hover {
  text-decoration: underline;
}

/* --- Theme Toggle --- */
.theme-toggle {
  position: fixed;
  bottom: 20px;
  right: 20px;
  width: 40px;
  height: 40px;
  border-radius: 50%;
  border: 1px solid var(--border-primary);
  background: var(--bg-primary);
  color: var(--text-secondary);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  box-shadow: 0 2px 8px rgba(0,0,0,0.1);
  transition: background-color 0.2s, color 0.2s;
}

.theme-toggle:hover {
  background: var(--bg-tertiary);
}

/* --- Print Styles --- */
@media print {
  :root {
    --bg-primary: #ffffff !important;
    --bg-secondary: #f8fafc !important;
    --bg-tertiary: #f1f5f9 !important;
    --text-primary: #0f172a !important;
    --text-secondary: #475569 !important;
    --text-muted: #94a3b8 !important;
    --border-primary: #e2e8f0 !important;
    --border-secondary: #cbd5e1 !important;
  }

  [data-theme="dark"] {
    --bg-primary: #ffffff !important;
    --bg-secondary: #f8fafc !important;
    --bg-tertiary: #f1f5f9 !important;
    --text-primary: #0f172a !important;
    --text-secondary: #475569 !important;
    --text-muted: #94a3b8 !important;
    --border-primary: #e2e8f0 !important;
    --border-secondary: #cbd5e1 !important;
  }

  body {
    background-color: white !important;
    padding: 0;
    margin: 0;
  }

  .container {
    max-width: 100%;
  }

  .message {
    break-inside: avoid;
    page-break-inside: avoid;
  }

  .code-block {
    break-inside: avoid;
    page-break-inside: avoid;
  }

  .thinking-block {
    break-inside: avoid;
    page-break-inside: avoid;
  }

  .theme-toggle {
    display: none !important;
  }

  .tool-summary:hover {
    background-color: transparent;
  }

  a {
    text-decoration: underline;
  }
}

/* --- Responsive --- */
@media (max-width: 640px) {
  body {
    padding: 12px;
  }

  .meta-grid {
    grid-template-columns: repeat(2, 1fr);
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

  .code-block code {
    font-size: 12px;
  }
}
`

// ---------------------------------------------------------------------------
// Theme toggle JS
// ---------------------------------------------------------------------------

const THEME_SCRIPT = `<script>
(function() {
  if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
    document.documentElement.setAttribute('data-theme', 'dark');
    var use = document.querySelector('.theme-toggle-icon');
    if (use) use.setAttribute('href', '#icon-sun');
  }
  window.toggleTheme = function() {
    var current = document.documentElement.getAttribute('data-theme');
    if (current === 'dark') {
      document.documentElement.removeAttribute('data-theme');
      var use = document.querySelector('.theme-toggle-icon');
      if (use) use.setAttribute('href', '#icon-moon');
    } else {
      document.documentElement.setAttribute('data-theme', 'dark');
      var use = document.querySelector('.theme-toggle-icon');
      if (use) use.setAttribute('href', '#icon-sun');
    }
  };
})();
</script>`

// ---------------------------------------------------------------------------
// Main exported functions
// ---------------------------------------------------------------------------

/**
 * Generates a standalone HTML document from conversation messages.
 * Publication-quality output with dark mode, metadata header, 7 message types,
 * thinking blocks, thread indentation, and VS Code-style code blocks.
 */
export function generateStandaloneHtml(messages: Message[], metadata?: ExportMetadata): string {
  const filtered = filterExportMessages(messages)
  const threadMap = buildThreadMap(filtered)

  const messagesHtml = filtered
    .map((message) => {
      const thread = message.uuid ? threadMap.get(message.uuid) : undefined
      return renderMessage(message, thread)
    })
    .join('\n')

  const title = metadata ? escapeHtml(metadata.projectName) : 'Claude Conversation'

  const headerHtml = metadata
    ? renderMetadataHeader(metadata)
    : `
    <div class="doc-header">
      <h1 class="doc-title">Claude Conversation</h1>
      <div class="doc-export-date">Exported ${escapeHtml(new Date().toLocaleString())}</div>
    </div>
  `

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${title}</title>
  <style>${CSS}</style>
</head>
<body>
  ${SVG_DEFS}
  <div class="container">
    ${headerHtml}
    <div class="messages">
      ${messagesHtml}
    </div>
    <div class="doc-footer">
      Exported from <a href="https://github.com/tombelieber/claude-view" target="_blank" rel="noopener noreferrer">claude-view</a>
    </div>
  </div>
  <button class="theme-toggle" onclick="toggleTheme()" aria-label="Toggle dark mode">
    <svg width="20" height="20"><use class="theme-toggle-icon" href="#icon-moon"/></svg>
  </button>
  ${THEME_SCRIPT}
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
 * Opens a print dialog to save conversation as PDF.
 * Uses Blob URL instead of data: URL to avoid browser size limits (Safari ~32KB).
 */
export function exportToPdf(messages: Message[], metadata?: ExportMetadata): void {
  const html = generateStandaloneHtml(messages, metadata)
  const blob = new Blob([html], { type: 'text/html;charset=utf-8' })
  const blobUrl = URL.createObjectURL(blob)
  const printWindow = window.open(blobUrl, '_blank')
  if (printWindow) {
    setTimeout(() => {
      printWindow.print()
      // Cleanup blob URL after a delay to ensure print dialog has the content
      setTimeout(() => URL.revokeObjectURL(blobUrl), 60000)
    }, 500)
  }
}
