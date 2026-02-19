import { useState, useRef, useEffect, useCallback, useMemo } from 'react'
import type { Components } from 'react-markdown'
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeRaw from 'rehype-raw'
import {
  User,
  Bot,
  Wrench,
  Brain,
  AlertTriangle,
  ArrowDown,
} from 'lucide-react'
import { ExpandProvider } from '../../contexts/ExpandContext'
import { CompactCodeBlock } from './CompactCodeBlock'
import { JsonKeyValueChips } from './JsonKeyValueChips'
import { JsonTree } from './JsonTree'
import { AskUserQuestionDisplay, isAskUserQuestionInput } from './AskUserQuestionDisplay'

// --- Types ---

export interface RichMessage {
  type: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'thinking' | 'error'
  content: string
  name?: string // tool name for tool_use
  input?: string // tool input summary for tool_use
  inputData?: unknown // raw parsed object for tool_use (avoids re-parsing)
  ts?: number // timestamp
}

export interface RichPaneProps {
  messages: RichMessage[]
  isVisible: boolean
  /** When false (default), only show user + assistant + error messages. */
  verboseMode?: boolean
  /** Signals that the initial WebSocket buffer has been fully loaded.
   *  Triggers an imperative scroll-to-bottom so panes start at the latest message. */
  bufferDone?: boolean
}

// --- Parser ---

/** Strip Claude Code internal command tags from content.
 * These tags appear in JSONL but are not meant for display:
 * <command-name>...</command-name>
 * <command-message>...</command-message>
 * <command-args>...</command-args>
 * <local-command-stdout>...</local-command-stdout>
 */
function stripCommandTags(content: string): string {
  return content
    .replace(/<command-name>[\s\S]*?<\/command-name>/g, '')
    .replace(/<command-message>[\s\S]*?<\/command-message>/g, '')
    .replace(/<command-args>[\s\S]*?<\/command-args>/g, '')
    .replace(/<local-command-stdout>[\s\S]*?<\/local-command-stdout>/g, '')
    .replace(/<system-reminder>[\s\S]*?<\/system-reminder>/g, '')
    .trim()
}

/** Convert a timestamp value (ISO string or number) to Unix seconds, or undefined. */
function parseTimestamp(ts: unknown): number | undefined {
  if (typeof ts === 'number' && isFinite(ts) && ts > 0) return ts
  if (typeof ts === 'string') {
    const ms = Date.parse(ts)
    if (!isNaN(ms)) return ms / 1000
  }
  return undefined
}

/**
 * Parse a raw WebSocket/SSE message string into a structured RichMessage.
 * Returns null for messages that don't map to a displayable type.
 */
export function parseRichMessage(raw: string): RichMessage | null {
  try {
    const msg = JSON.parse(raw)
    if (msg.type === 'message') {
      const content = stripCommandTags(typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content, null, 2))
      if (!content.trim()) return null
      return {
        type: msg.role === 'user' ? 'user' : 'assistant',
        content,
        ts: parseTimestamp(msg.ts),
      }
    }
    if (msg.type === 'tool_use') {
      return {
        type: 'tool_use',
        content: '',
        name: msg.name,
        input: msg.input ? JSON.stringify(msg.input, null, 2) : undefined,
        inputData: msg.input ?? undefined,
        ts: parseTimestamp(msg.ts),
      }
    }
    if (msg.type === 'tool_result') {
      const content = stripCommandTags(typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content || '', null, 2))
      if (!content.trim()) return null
      return {
        type: 'tool_result',
        content,
        ts: parseTimestamp(msg.ts),
      }
    }
    if (msg.type === 'thinking') {
      const content = stripCommandTags(typeof msg.content === 'string' ? msg.content : '')
      if (!content.trim()) return null
      return {
        type: 'thinking',
        content,
        ts: parseTimestamp(msg.ts),
      }
    }
    if (msg.type === 'error') {
      return {
        type: 'error',
        content: typeof msg.message === 'string' ? msg.message : JSON.stringify(msg, null, 2),
      }
    }
    if (msg.type === 'line') {
      const content = stripCommandTags(typeof msg.data === 'string' ? msg.data : '')
      if (!content.trim()) return null
      return {
        type: 'assistant',
        content,
      }
    }
    return null
  } catch {
    return null
  }
}

// --- JSON Detection ---

/** Try to parse a string as JSON. Returns parsed value or null. */
function tryParseJson(str: string): unknown | null {
  try {
    const trimmed = str.trim()
    if ((!trimmed.startsWith('{') && !trimmed.startsWith('[')) || trimmed.length < 2) return null
    return JSON.parse(trimmed)
  } catch {
    return null
  }
}

/** Check if a string is valid JSON object or array. */
function isJsonContent(content: string): boolean {
  return tryParseJson(content) !== null
}

/** Heuristic: content looks like a unified diff (git diff, patch output).
 *  Checks for diff headers or a high density of +/- prefixed lines. */
function isDiffContent(content: string): boolean {
  // Quick checks for diff headers
  if (content.startsWith('diff --git') || content.startsWith('---') || content.startsWith('Index:')) return true
  const lines = content.split('\n')
  if (lines.length < 3) return false
  const nonEmpty = lines.filter((l) => l.length > 0)
  if (nonEmpty.length < 3) return false
  // Count lines that look like diff hunks: @@, +line, -line
  const diffLines = nonEmpty.filter((l) => /^[+-][^+-]/.test(l) || l.startsWith('@@')).length
  return diffLines / nonEmpty.length >= 0.3
}

/** Heuristic: content looks like file output (Read tool, grep, etc.).
 *  Matches lines starting with optional whitespace + digits + → (cat -n format),
 *  or lines starting with filepath:line patterns like "src/foo.tsx:42:". */
const LINE_NUM_RE = /^\s*\d+[→\t|:]/
function isCodeLikeContent(content: string): boolean {
  const lines = content.split('\n')
  if (lines.length < 2) return false
  // If ≥40% of non-empty lines match the pattern, treat as code
  const nonEmpty = lines.filter((l) => l.trim().length > 0)
  if (nonEmpty.length < 2) return false
  const matching = nonEmpty.filter((l) => LINE_NUM_RE.test(l)).length
  return matching / nonEmpty.length >= 0.4
}

/** Strip line-number prefixes (e.g. "  42→ ") so heuristics see actual code. */
function stripLineNumbers(content: string): string {
  return content.replace(/^\s*\d+[→\t|]\s?/gm, '')
}

/** Detect the best Shiki language hint for code-like content. */
function detectCodeLanguage(content: string): string {
  if (isDiffContent(content)) return 'diff'
  const raw = stripLineNumbers(content)
  // Rust — check early because `use` is common in other languages too
  if (/\b(use\s+(std|crate|super|self)::|fn\s+\w+\s*[<(]|impl\s+(<.*>)?\s*\w+|pub\s+(fn|struct|enum|mod|type|trait|const|static)\b|let\s+mut\b|#\[derive)/.test(raw)) return 'rust'
  // TypeScript / TSX / JS
  if (/\bimport\s+.*\bfrom\s+['"]/.test(raw) || /\bexport\s+(default\s+)?(function|const|class|interface|type)\b/.test(raw) || /\bconst\s+\w+\s*[:=]/.test(raw) && /\b(string|number|boolean|Promise|async|await)\b/.test(raw)) {
    return /\bReact\b|['"]react['"]|<\w+[A-Z]|className=|useState|useEffect/.test(raw) ? 'tsx' : 'typescript'
  }
  // Python
  if (/\b(def\s+\w+\s*\(|class\s+\w+[\s:(]|from\s+\w+\s+import|import\s+\w+|self\.\w+|__\w+__|@\w+)/.test(raw)) return 'python'
  // Go
  if (/\bpackage\s+\w+/.test(raw) && /\bfunc\s+/.test(raw)) return 'go'
  // SQL
  if (/\b(SELECT|INSERT INTO|CREATE TABLE|ALTER TABLE|UPDATE\s+\w+\s+SET|DELETE FROM)\b/i.test(raw)) return 'sql'
  // HTML / JSX (angle brackets with component names)
  if (/<\/?[A-Z]\w+[\s/>]/.test(raw) || /<(div|span|section|header|footer|main|form|button|input)\b/i.test(raw)) return 'tsx'
  // CSS
  if (/[.#]\w+\s*\{[\s\S]*?[;}]/.test(raw) && /\b(color|background|margin|padding|display|flex|grid)\s*:/.test(raw)) return 'css'
  // Bash / shell
  if (/^#!\//.test(raw) || /\b(echo|export|source|chmod|mkdir|cd|ls|grep|sed|awk|curl|wget)\b/.test(raw)) return 'bash'
  // JSON (didn't pass isJsonContent but has JSON-like structure)
  if (/^\s*["{\[]/.test(raw.trim())) return 'json'
  // YAML / TOML
  if (/^\w[\w-]*\s*[:=]\s/.test(raw) && !/[;{}()]/.test(raw.slice(0, 200))) return 'yaml'
  // C / C++
  if (/\b(#include\s*<|int\s+main\s*\(|void\s+\w+\(|printf\s*\()/.test(raw)) return 'c'
  // Java
  if (/\b(public\s+class|private\s+|protected\s+|System\.out)/.test(raw)) return 'java'
  // Ruby
  if (/\b(require\s+['"]|module\s+\w+|class\s+\w+\s*<|end\b.*\n.*\bdef\b)/.test(raw)) return 'ruby'
  // Markdown
  if (/^#{1,6}\s+\w/.test(raw) && /\n#{1,6}\s+\w/.test(raw)) return 'markdown'
  return 'text'
}

// --- Helpers ---

/** Format a timestamp as a static time label (chat-app style). Guards against epoch-zero. */
function formatTimestamp(ts: number | undefined): string | null {
  if (!ts || ts <= 0) return null
  const date = new Date(ts * 1000)
  if (isNaN(date.getTime())) return null
  const now = new Date()
  const time = date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })
  // Today: "10:30 AM"
  if (date.toDateString() === now.toDateString()) return time
  // Yesterday: "Yesterday 10:30 AM"
  const yesterday = new Date(now)
  yesterday.setDate(yesterday.getDate() - 1)
  if (date.toDateString() === yesterday.toDateString()) return `Yesterday ${time}`
  // This year: "Jan 15, 10:30 AM"
  if (date.getFullYear() === now.getFullYear()) {
    const month = date.toLocaleString(undefined, { month: 'short' })
    return `${month} ${date.getDate()}, ${time}`
  }
  // Older: "Jan 15 '25, 10:30 AM"
  const month = date.toLocaleString(undefined, { month: 'short' })
  return `${month} ${date.getDate()} '${String(date.getFullYear()).slice(-2)}, ${time}`
}

// --- Markdown custom renderers ---

/** Stable counter for generating unique code block IDs within a single render cycle. */
let mdBlockCounter = 0

/**
 * Custom react-markdown `components` that route fenced code blocks
 * through CompactCodeBlock (Shiki highlighting, copy, collapse) and
 * give inline `code` a distinct visual treatment.
 */
const markdownComponents: Components = {
  // Fenced code blocks: ```lang ... ``` → CompactCodeBlock with Shiki
  // react-markdown wraps these in <pre><code className="language-X">
  pre({ children }) {
    // Extract <code> child props to get language + text
    const codeChild = Array.isArray(children) ? children[0] : children
    if (codeChild && typeof codeChild === 'object' && 'props' in codeChild) {
      const { className, children: codeText } = codeChild.props as {
        className?: string
        children?: React.ReactNode
      }
      const langMatch = /language-(\w+)/.exec(className || '')
      const lang = langMatch ? langMatch[1] : 'text'
      const text = String(codeText || '').replace(/\n$/, '')
      const id = `md-code-${mdBlockCounter++}`
      return <CompactCodeBlock code={text} language={lang} blockId={id} />
    }
    // Fallback: plain <pre> (shouldn't happen with standard markdown)
    return <pre className="text-[11px] font-mono overflow-x-auto">{children}</pre>
  },
  // Inline code — compact monospace chip
  code({ children, ...rest }) {
    return (
      <code
        className="px-1 py-0.5 rounded text-[11px] font-mono bg-gray-100 dark:bg-gray-800 text-pink-600 dark:text-pink-400"
        {...rest}
      >
        {children}
      </code>
    )
  },
}

// --- Tool Name Helpers ---

/** Shorten verbose MCP tool names: "mcp__chrome-devtools__take_snapshot" → "take_snapshot"
 *  Also strips common prefixes for readability in the terminal view. */
function shortenToolName(name: string): { short: string; server?: string } {
  // MCP tools: mcp__<server>__<tool_name>
  const mcpMatch = /^mcp__([^_]+(?:_[^_]+)*)__(.+)$/.exec(name)
  if (mcpMatch) {
    return { short: mcpMatch[2], server: mcpMatch[1] }
  }
  return { short: name }
}

/** Pick a distinct color class based on tool category. */
function toolChipColor(name: string): string {
  // MCP tools
  if (name.startsWith('mcp__')) return 'bg-blue-500/10 dark:bg-blue-500/20 text-blue-700 dark:text-blue-300'
  // Task / sub-agent
  if (name === 'Task') return 'bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-700 dark:text-indigo-300'
  // Skill
  if (name === 'Skill') return 'bg-purple-500/10 dark:bg-purple-500/20 text-purple-700 dark:text-purple-300'
  // Common built-in tools
  if (name === 'Read' || name === 'Glob' || name === 'Grep') return 'bg-emerald-500/10 dark:bg-emerald-500/20 text-emerald-700 dark:text-emerald-300'
  if (name === 'Write' || name === 'Edit') return 'bg-amber-500/10 dark:bg-amber-500/20 text-amber-700 dark:text-amber-300'
  if (name === 'Bash') return 'bg-gray-500/10 dark:bg-gray-500/20 text-gray-700 dark:text-gray-300'
  // Default (orange for other tools)
  return 'bg-orange-500/10 dark:bg-orange-500/20 text-orange-700 dark:text-orange-300'
}

// --- Message Card Components ---

function UserMessage({ message, verboseMode = false }: { message: RichMessage; index?: number; verboseMode?: boolean }) {
  const jsonDetected = isJsonContent(message.content)
  const parsedJson = jsonDetected ? tryParseJson(message.content) : null
  return (
    <div className="border-l-2 border-blue-500 pl-2 py-1">
      <div className="flex items-start gap-1.5">
        <User className="w-3 h-3 text-blue-500 dark:text-blue-400 flex-shrink-0 mt-0.5" />
        <div className="min-w-0 flex-1">
          {parsedJson !== null ? (
            verboseMode ? (
              <JsonTree data={parsedJson} verboseMode={verboseMode} />
            ) : (
              <CompactCodeBlock code={JSON.stringify(parsedJson, null, 2)} language="json" blockId={`user-json-${message.ts ?? 0}`} />
            )
          ) : (
            <div className="text-xs text-gray-800 dark:text-gray-200 leading-relaxed prose dark:prose-invert prose-sm max-w-none">
              <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw]} components={markdownComponents}>{message.content}</Markdown>
            </div>
          )}
        </div>
        <Timestamp ts={message.ts} />
      </div>
    </div>
  )
}

function AssistantMessage({ message, verboseMode = false }: { message: RichMessage; index?: number; verboseMode?: boolean }) {
  const jsonDetected = isJsonContent(message.content)
  const parsedJson = jsonDetected ? tryParseJson(message.content) : null
  return (
    <div className="pl-2 py-1">
      <div className="flex items-start gap-1.5">
        <Bot className="w-3 h-3 text-gray-500 dark:text-gray-400 flex-shrink-0 mt-0.5" />
        <div className="min-w-0 flex-1">
          {parsedJson !== null ? (
            verboseMode ? (
              <JsonTree data={parsedJson} verboseMode={verboseMode} />
            ) : (
              <CompactCodeBlock code={JSON.stringify(parsedJson, null, 2)} language="json" blockId={`asst-json-${message.ts ?? 0}`} />
            )
          ) : (
            <div className="text-xs text-gray-700 dark:text-gray-300 leading-relaxed prose dark:prose-invert prose-sm max-w-none">
              <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw]} components={markdownComponents}>{message.content}</Markdown>
            </div>
          )}
        </div>
        <Timestamp ts={message.ts} />
      </div>
    </div>
  )
}

function ToolUseMessage({ message, index, verboseMode = false }: { message: RichMessage; index: number; verboseMode?: boolean }) {
  const [expanded, setExpanded] = useState(verboseMode)
  const rawName = message.name || 'Tool'
  const { short: label, server } = shortenToolName(rawName)
  const chipColor = toolChipColor(rawName)
  const inputObj = message.inputData
  const isObjectInput = inputObj !== null && inputObj !== undefined && typeof inputObj === 'object' && !Array.isArray(inputObj)
  const isAskUserQuestion = rawName === 'AskUserQuestion' && isAskUserQuestionInput(inputObj)

  if (isAskUserQuestion && !verboseMode) {
    return (
      <div className="py-0.5">
        <AskUserQuestionDisplay inputData={inputObj} variant="amber" />
      </div>
    )
  }

  return (
    <div className="py-0.5 border-l-2 border-orange-500/30 dark:border-orange-500/20 pl-1">
      <div className="flex items-start gap-1.5">
        <Wrench className="w-3 h-3 text-orange-500 dark:text-orange-400 flex-shrink-0 mt-0.5" />
        <div className="min-w-0 flex-1">
          <div className="flex items-start gap-1.5 flex-wrap">
            <span className={`inline-flex items-center px-2 py-0.5 rounded text-[10px] font-mono font-semibold flex-shrink-0 ${chipColor}`}>
              {label}
            </span>
            {server && (
              <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600 flex-shrink-0 self-center">
                {server}
              </span>
            )}
            {!expanded && isObjectInput && (
              <JsonKeyValueChips
                data={inputObj as Record<string, unknown>}
                onExpand={() => setExpanded(true)}
                verboseMode={verboseMode}
              />
            )}
            {!expanded && !isObjectInput && message.input && (
              <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 break-all">{message.input}</span>
            )}
          </div>
          {expanded && (
            <div className="mt-1">
              <button
                onClick={() => setExpanded(false)}
                className="text-[10px] text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 mb-1 transition-colors cursor-pointer"
              >
                [ Collapse ]
              </button>
              {isObjectInput ? (
                <JsonTree data={inputObj} verboseMode={verboseMode} />
              ) : (
                <CompactCodeBlock code={message.input} language="json" blockId={`tool-input-${index}`} />
              )}
            </div>
          )}
        </div>
        <Timestamp ts={message.ts} />
      </div>
    </div>
  )
}

function ToolResultMessage({ message, index, verboseMode = false }: { message: RichMessage; index: number; verboseMode?: boolean }) {
  const hasContent = message.content.length > 0
  const jsonDetected = hasContent && isJsonContent(message.content)
  const diffLike = hasContent && !jsonDetected && isDiffContent(message.content)
  const codeLike = hasContent && !jsonDetected && !diffLike && isCodeLikeContent(message.content)
  const codeLang = codeLike ? detectCodeLanguage(message.content) : 'text'
  // Strip line-number prefixes (e.g. "  42→ ") so Shiki can parse clean code
  const cleanCode = codeLike ? stripLineNumbers(message.content) : message.content

  // Always use JsonTree for JSON results — it collapses nested objects,
  // truncates long strings with tooltips, and avoids horizontal scroll.
  const parsedJson = jsonDetected ? tryParseJson(message.content) : null

  return (
    <div className="py-0.5 pl-3 border-l-2 border-gray-300/30 dark:border-gray-700/50 ml-1">
      <div className="flex items-center gap-1">
        <span className="text-[10px] text-gray-500 dark:text-gray-600 font-mono">result</span>
        <div className="flex-1" />
        <Timestamp ts={message.ts} />
      </div>
      {hasContent && (
        jsonDetected && parsedJson !== null ? (
          <div className="mt-0.5 pl-4">
            <JsonTree data={parsedJson} verboseMode={verboseMode} />
          </div>
        ) : diffLike ? (
          <div className="mt-0.5 pl-4 diff-block">
            <CompactCodeBlock code={message.content} language="diff" blockId={`result-${index}`} />
          </div>
        ) : codeLike ? (
          <div className="mt-0.5 pl-4">
            <CompactCodeBlock code={cleanCode} language={codeLang} blockId={`result-${index}`} />
          </div>
        ) : (
          <div className="text-[10px] text-gray-600 dark:text-gray-500 mt-0.5 pl-4 font-mono leading-relaxed prose dark:prose-invert prose-sm max-w-none">
            <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw]} components={markdownComponents}>{message.content}</Markdown>
          </div>
        )
      )}
    </div>
  )
}

function ThinkingMessage({ message }: { message: RichMessage }) {
  const [expanded, setExpanded] = useState(false)
  // Show a preview: first line or first ~120 chars
  const preview = useMemo(() => {
    const first = message.content.split('\n')[0] || ''
    return first.length > 120 ? first.slice(0, 120) + '…' : first
  }, [message.content])

  return (
    <div className="py-0.5">
      <button
        onClick={() => setExpanded((v) => !v)}
        className="flex items-center gap-1.5 w-full text-left cursor-pointer group"
      >
        <Brain className="w-3 h-3 text-purple-500/50 dark:text-purple-400/50 flex-shrink-0" />
        <span className="text-[10px] text-gray-500 dark:text-gray-600 italic">thinking...</span>
        <span className="text-[10px] text-gray-500 dark:text-gray-700 italic truncate flex-1 min-w-0 opacity-60 group-hover:opacity-100 transition-opacity">
          {preview}
        </span>
        <Timestamp ts={message.ts} />
      </button>
      {expanded && (
        <div className="text-[10px] text-gray-500 dark:text-gray-600 italic mt-0.5 pl-5 leading-relaxed prose dark:prose-invert prose-sm max-w-none border-l border-purple-500/20 ml-1.5">
          <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw]} components={markdownComponents}>{message.content}</Markdown>
        </div>
      )}
    </div>
  )
}

function ErrorMessage({ message, index }: { message: RichMessage; index: number }) {
  const jsonDetected = isJsonContent(message.content)
  return (
    <div className="border-l-2 border-red-500 pl-2 py-1">
      <div className="flex items-start gap-1.5">
        <AlertTriangle className="w-3 h-3 text-red-500 dark:text-red-400 flex-shrink-0 mt-0.5" />
        {jsonDetected ? (
          <div className="flex-1 min-w-0">
            <CompactCodeBlock code={message.content} language="json" blockId={`error-${index}`} />
          </div>
        ) : (
          <pre className="text-xs text-red-600 dark:text-red-300 whitespace-pre-wrap break-words font-sans leading-relaxed flex-1 min-w-0">
            {message.content}
          </pre>
        )}
        <Timestamp ts={message.ts} />
      </div>
    </div>
  )
}

function Timestamp({ ts }: { ts?: number }) {
  const label = formatTimestamp(ts)
  if (!label) return null
  return (
    <span className="text-[9px] text-gray-400 dark:text-gray-600 tabular-nums flex-shrink-0 whitespace-nowrap">
      {label}
    </span>
  )
}

// --- Message renderer dispatch ---

function MessageCard({ message, index, verboseMode = false }: { message: RichMessage; index: number; verboseMode?: boolean }) {
  switch (message.type) {
    case 'user':
      return <UserMessage message={message} index={index} verboseMode={verboseMode} />
    case 'assistant':
      return <AssistantMessage message={message} index={index} verboseMode={verboseMode} />
    case 'tool_use':
      return <ToolUseMessage message={message} index={index} verboseMode={verboseMode} />
    case 'tool_result':
      return <ToolResultMessage message={message} index={index} verboseMode={verboseMode} />
    case 'thinking':
      return <ThinkingMessage message={message} />
    case 'error':
      return <ErrorMessage message={message} index={index} />
    default:
      return null
  }
}

// --- Main Component ---

export function RichPane({ messages, isVisible, verboseMode = false, bufferDone = false }: RichPaneProps) {
  const displayMessages = useMemo(() => {
    if (verboseMode) return messages
    return messages.filter((m) => {
      if (m.type === 'user' || m.type === 'error') return true
      if (m.type === 'assistant') {
        // Hide raw Task/sub-agent JSON blobs (e.g. {"task_id":...,"task_type":"local_agent"})
        const t = m.content.trim()
        if (t.startsWith('{') && t.includes('"task_id"') && t.includes('"task_type"')) return false
        return true
      }
      // Show AskUserQuestion in compact mode (friendly card, not raw JSON)
      if (m.type === 'tool_use' && m.name === 'AskUserQuestion' && isAskUserQuestionInput(m.inputData)) return true
      return false
    })
  }, [messages, verboseMode])

  const virtuosoRef = useRef<VirtuosoHandle>(null)
  const [isAtBottom, setIsAtBottom] = useState(true)
  const [hasNewMessages, setHasNewMessages] = useState(false)
  const prevMessageCountRef = useRef(displayMessages.length)
  const hasScrolledToBottomRef = useRef(false)
  const prevVerboseModeRef = useRef(verboseMode)

  // Jump to bottom once after initial buffer loads
  useEffect(() => {
    if (bufferDone && !hasScrolledToBottomRef.current && displayMessages.length > 0) {
      hasScrolledToBottomRef.current = true
      // Use requestAnimationFrame to ensure Virtuoso has rendered the data
      requestAnimationFrame(() => {
        virtuosoRef.current?.scrollToIndex({
          index: displayMessages.length - 1,
          behavior: 'auto',
        })
      })
    }
  }, [bufferDone, displayMessages.length])

  // Scroll to bottom when verbose mode toggles (list length changes drastically)
  useEffect(() => {
    if (prevVerboseModeRef.current !== verboseMode) {
      prevVerboseModeRef.current = verboseMode
      if (displayMessages.length > 0) {
        requestAnimationFrame(() => {
          virtuosoRef.current?.scrollToIndex({
            index: displayMessages.length - 1,
            behavior: 'auto',
          })
        })
      }
    }
  }, [verboseMode, displayMessages.length])

  // Track when new messages arrive while user is scrolled up
  useEffect(() => {
    if (displayMessages.length > prevMessageCountRef.current) {
      if (isAtBottom) {
        setHasNewMessages(false)
      } else {
        setHasNewMessages(true)
      }
    }
    prevMessageCountRef.current = displayMessages.length
  }, [displayMessages.length, isAtBottom])

  const handleAtBottomStateChange = useCallback((atBottom: boolean) => {
    setIsAtBottom(atBottom)
    if (atBottom) {
      setHasNewMessages(false)
    }
  }, [])

  const scrollToBottom = useCallback(() => {
    virtuosoRef.current?.scrollToIndex({
      index: displayMessages.length - 1,
      behavior: 'smooth',
    })
    setHasNewMessages(false)
  }, [displayMessages.length])

  if (!isVisible) return null

  if (displayMessages.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-xs text-gray-500 dark:text-gray-600">
        No messages yet
      </div>
    )
  }

  return (
    <ExpandProvider>
      <div className="relative h-full w-full">
        <Virtuoso
          ref={virtuosoRef}
          data={displayMessages}
          initialTopMostItemIndex={displayMessages.length - 1}
          alignToBottom
          followOutput={'smooth'}
          atBottomStateChange={handleAtBottomStateChange}
          atBottomThreshold={30}
          itemContent={(index, message) => (
            <div className="px-2 py-0.5">
              <MessageCard message={message} index={index} verboseMode={verboseMode} />
            </div>
          )}
          className="h-full"
        />

        {/* "New messages" floating pill — click to scroll to latest */}
        {hasNewMessages && !isAtBottom && (
          <button
            onClick={scrollToBottom}
            className="absolute bottom-2 left-1/2 -translate-x-1/2 inline-flex items-center gap-1 bg-blue-600 hover:bg-blue-500 text-white text-xs px-3 py-1 rounded-full shadow-lg transition-colors z-10"
          >
            <ArrowDown className="w-3 h-3" />
            New messages
          </button>
        )}
      </div>
    </ExpandProvider>
  )
}
