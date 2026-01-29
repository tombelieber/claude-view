import { useState, useMemo } from 'react'
import { ChevronRight, ChevronDown, FileText, Brain, Wrench, FileCode, Terminal, Bot, CheckCircle2, XCircle, AlertTriangle, Zap, Shield } from 'lucide-react'
import DOMPurify from 'dompurify'
import { cn } from '../lib/utils'
import { CodeBlock } from './CodeBlock'

interface XmlCardProps {
  content: string
  type: 'observed_from_primary_session' | 'observation' | 'tool_call' | 'local_command' | 'task_notification' | 'command' | 'tool_error' | 'untrusted_data' | 'hidden' | 'unknown'
}

interface ParsedObservation {
  type?: string
  title?: string
  subtitle?: string
  facts?: string[]
  narrative?: string
  filesRead?: string[]
  filesModified?: string[]
}

interface ParsedToolCall {
  whatHappened?: string
  parameters?: string
  outcome?: string
  workingDirectory?: string
}

function parseObservation(xml: string): ParsedObservation {
  const result: ParsedObservation = {}

  const typeMatch = xml.match(/<type>([^<]+)<\/type>/)
  if (typeMatch) result.type = typeMatch[1]

  const titleMatch = xml.match(/<title>([^<]+)<\/title>/)
  if (titleMatch) result.title = titleMatch[1]

  const subtitleMatch = xml.match(/<subtitle>([^<]+)<\/subtitle>/)
  if (subtitleMatch) result.subtitle = subtitleMatch[1]

  const factsMatch = xml.match(/<facts>([\s\S]*?)<\/facts>/)
  if (factsMatch) {
    const factMatches = factsMatch[1].match(/<fact>([^<]+)<\/fact>/g)
    if (factMatches) {
      result.facts = factMatches.map(f => f.replace(/<\/?fact>/g, ''))
    }
  }

  const narrativeMatch = xml.match(/<narrative>([\s\S]*?)<\/narrative>/)
  if (narrativeMatch) result.narrative = narrativeMatch[1].trim()

  const filesReadMatch = xml.match(/<files_read>([\s\S]*?)<\/files_read>/)
  if (filesReadMatch) {
    const fileMatches = filesReadMatch[1].match(/<file>([^<]+)<\/file>/g)
    if (fileMatches) {
      result.filesRead = fileMatches.map(f => f.replace(/<\/?file>/g, ''))
    }
  }

  return result
}

function parseToolCall(xml: string): ParsedToolCall {
  const result: ParsedToolCall = {}

  const whatMatch = xml.match(/<what_happened>([^<]+)<\/what_happened>/)
  if (whatMatch) result.whatHappened = whatMatch[1]

  const paramsMatch = xml.match(/<parameters>"?([^"<]+)"?<\/parameters>/)
  if (paramsMatch) {
    try {
      const parsed = JSON.parse(paramsMatch[1])
      result.parameters = parsed.file_path || JSON.stringify(parsed).substring(0, 100)
    } catch {
      result.parameters = paramsMatch[1].substring(0, 100)
    }
  }

  const dirMatch = xml.match(/<working_directory>([^<]+)<\/working_directory>/)
  if (dirMatch) result.workingDirectory = dirMatch[1]

  return result
}

function getIcon(type: XmlCardProps['type']) {
  switch (type) {
    case 'observed_from_primary_session':
      return FileText
    case 'observation':
      return Brain
    case 'tool_call':
      return Wrench
    case 'local_command':
      return Terminal
    case 'task_notification':
      return Bot
    case 'command':
      return Zap
    case 'tool_error':
      return XCircle
    case 'untrusted_data':
      return Shield
    default:
      return FileCode
  }
}

function getLabel(type: XmlCardProps['type']) {
  switch (type) {
    case 'observed_from_primary_session':
      return 'Tool Call'
    case 'observation':
      return 'Observation'
    case 'tool_call':
      return 'Tool'
    case 'local_command':
      return 'Command Output'
    case 'task_notification':
      return 'Agent Task'
    case 'command':
      return 'Command'
    case 'tool_error':
      return 'Tool Error'
    case 'untrusted_data':
      return 'External Content'
    default:
      return 'Structured Content'
  }
}

interface ParsedTaskNotification {
  taskId?: string
  status?: string
  summary?: string
  result?: string
}

function parseTaskNotification(xml: string): ParsedTaskNotification {
  const result: ParsedTaskNotification = {}
  const idMatch = xml.match(/<task-id>([^<]+)<\/task-id>/)
  if (idMatch) result.taskId = idMatch[1]
  const statusMatch = xml.match(/<status>([^<]+)<\/status>/)
  if (statusMatch) result.status = statusMatch[1]
  const summaryMatch = xml.match(/<summary>([^<]+)<\/summary>/)
  if (summaryMatch) result.summary = summaryMatch[1]
  const resultMatch = xml.match(/<result>([\s\S]*?)<\/result>/)
  if (resultMatch) result.result = resultMatch[1].trim()
  return result
}

interface ParsedCommand {
  name: string
  args: string
}

function parseCommand(xml: string): ParsedCommand {
  const result: ParsedCommand = { name: '', args: '' }
  const nameMatch = xml.match(/<command-name>([^<]+)<\/command-name>/)
  if (nameMatch) result.name = nameMatch[1].trim()
  const argsMatch = xml.match(/<command-args>([\s\S]*?)<\/command-args>/)
  if (argsMatch) result.args = argsMatch[1].trim()
  return result
}

function computeDefaultExpanded(content: string, type: XmlCardProps['type']): boolean {
  if (type === 'command') {
    const argsMatch = content.match(/<command-args>([\s\S]*?)<\/command-args>/)
    const args = argsMatch?.[1]?.trim() || ''
    return args.split('\n').length <= 10
  }
  if (type === 'untrusted_data') {
    const tagMatch = content.match(/<untrusted-data-[a-f0-9-]+>([\s\S]*?)<\/untrusted-data-[a-f0-9-]+>/)
    const inner = tagMatch?.[1]?.trim() || content.trim()
    return inner.split('\n').length <= 10
  }
  return false
}

export function XmlCard({ content, type }: XmlCardProps) {
  const [expanded, setExpanded] = useState(() => computeDefaultExpanded(content, type))

  // Hidden types render nothing
  if (type === 'hidden') return null

  const Icon = getIcon(type)
  const label = getLabel(type)

  // Local command output: render as terminal-style inline block (no collapse)
  if (type === 'local_command') {
    const stdout = content.match(/<local-command-stdout>([\s\S]*?)<\/local-command-stdout>/)
    const stderr = content.match(/<local-command-stderr>([\s\S]*?)<\/local-command-stderr>/)
    const outputText = (stdout?.[1] || stderr?.[1] || '').trim()
    if (!outputText) return null
    const isError = !!stderr && !stdout

    return (
      <div className={cn(
        'flex items-start gap-2 my-2 px-3 py-2 rounded-lg font-mono text-xs',
        isError ? 'bg-red-950' : 'bg-gray-900'
      )}>
        <Terminal className={cn(
          'w-3.5 h-3.5 flex-shrink-0 mt-0.5',
          isError ? 'text-red-400' : 'text-green-400'
        )} />
        <span className={cn(
          'break-all whitespace-pre-wrap',
          isError ? 'text-red-300' : 'text-green-300'
        )}>{outputText}</span>
      </div>
    )
  }

  // Task notification: render as agent status card
  if (type === 'task_notification') {
    const parsed = parseTaskNotification(content)
    const isCompleted = parsed.status === 'completed'
    const isFailed = parsed.status === 'failed'
    const StatusIcon = isFailed ? XCircle : isCompleted ? CheckCircle2 : AlertTriangle
    const statusColor = isFailed ? 'text-red-500' : isCompleted ? 'text-green-500' : 'text-yellow-500'

    return (
      <div className="border border-gray-200 rounded-lg overflow-hidden bg-white my-2">
        <button
          onClick={() => setExpanded(!expanded)}
          className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-gray-50 transition-colors"
        >
          <Bot className="w-4 h-4 text-gray-400 flex-shrink-0" />
          <StatusIcon className={cn('w-3.5 h-3.5 flex-shrink-0', statusColor)} />
          <span className="text-sm text-gray-600 truncate flex-1">
            {parsed.summary || 'Agent task'}
          </span>
          {expanded ? (
            <ChevronDown className="w-4 h-4 text-gray-400" />
          ) : (
            <ChevronRight className="w-4 h-4 text-gray-400" />
          )}
        </button>
        {expanded && parsed.result && (
          <div className="px-3 py-2 border-t border-gray-100 bg-gray-50 text-sm text-gray-600 whitespace-pre-wrap">
            {parsed.result}
          </div>
        )}
      </div>
    )
  }

  // Command invocation: render as indigo-accented card
  if (type === 'command') {
    const parsed = parseCommand(content)
    const hasArgs = parsed.args.length > 0
    const argLines = hasArgs ? parsed.args.split('\n').length : 0
    const shouldCollapse = argLines > 10
    const firstLine = hasArgs ? parsed.args.split('\n')[0] : ''

    return (
      <div className="border border-gray-200 border-l-4 border-l-indigo-400 rounded-lg overflow-hidden bg-white my-2">
        <button
          onClick={() => hasArgs && shouldCollapse && setExpanded(!expanded)}
          className={cn(
            'w-full flex items-center gap-2 px-3 py-2 text-left bg-indigo-950/30',
            hasArgs && shouldCollapse && 'cursor-pointer hover:bg-indigo-950/40 transition-colors'
          )}
        >
          <Zap className="w-4 h-4 text-indigo-400 flex-shrink-0" />
          <span className="text-sm font-mono text-indigo-300 flex-1">
            {parsed.name}
          </span>
          {hasArgs && shouldCollapse && (
            expanded ? (
              <ChevronDown className="w-4 h-4 text-indigo-400" />
            ) : (
              <ChevronRight className="w-4 h-4 text-indigo-400" />
            )
          )}
        </button>
        {hasArgs && (
          <div className="px-3 py-2 border-t border-gray-100 bg-gray-50">
            {shouldCollapse && !expanded ? (
              <pre className="text-xs text-gray-600 whitespace-pre-wrap break-all font-mono">
                {firstLine}...
              </pre>
            ) : (
              <pre className="text-xs text-gray-700 whitespace-pre-wrap break-all font-mono">
                {parsed.args}
              </pre>
            )}
          </div>
        )}
      </div>
    )
  }

  // Tool use error: render as red error card (always expanded)
  if (type === 'tool_error') {
    const errorText = content.replace(/<\/?tool_use_error>/g, '').trim()
    const firstLine = errorText.split('\n')[0].trim()
    const errorType = firstLine.length > 60 ? firstLine.substring(0, 57) + '...' : firstLine

    return (
      <div className="border border-gray-200 border-l-4 border-l-red-500 rounded-lg overflow-hidden bg-white my-2">
        <div className="flex items-center gap-2 px-3 py-2 bg-red-50">
          <XCircle className="w-4 h-4 text-red-500 flex-shrink-0" />
          <span className="text-sm font-medium text-red-700">Tool Error</span>
          {errorType && (
            <>
              <span className="text-gray-300">·</span>
              <span className="text-sm text-red-600 truncate flex-1">{errorType}</span>
            </>
          )}
        </div>
        <div className="px-3 py-2 bg-red-950 font-mono text-xs text-red-300 whitespace-pre-wrap break-all">
          {errorText}
        </div>
      </div>
    )
  }

  // Untrusted data: render external content with amber dashed border (plaintext only for security)
  if (type === 'untrusted_data') {
    const tagMatch = content.match(/<untrusted-data-[a-f0-9-]+>([\s\S]*?)<\/untrusted-data-[a-f0-9-]+>/)
    const innerContent = tagMatch?.[1]?.trim() || content.trim()
    const lines = innerContent.split('\n')
    const shouldCollapse = lines.length > 10
    const displayContent = expanded
      ? innerContent
      : lines.slice(0, 3).join('\n') + (lines.length > 3 ? '\n...' : '')

    // Security: Render as plaintext in <pre> tag
    // React's <pre> tag renders all content as text nodes (not HTML), which prevents:
    // - Script execution (no way to execute scripts when rendering as text)
    // - Event handler binding (onclick/onerror don't bind to text)
    // - Any HTML interpretation (markup appears literally)
    //
    // Additional defense-in-depth: Apply DOMPurify to strip any executable content
    // ALLOWED_TAGS: [] with KEEP_CONTENT: true means:
    // - All tags are stripped (script, svg, iframe, etc. become harmless)
    // - All attributes are removed (onclick, onerror, src, etc. removed)
    // - Text content is preserved
    // Final result: dangerous markup becomes plaintext when rendered in <pre>
    const sanitized = useMemo(
      () => {
        if (!displayContent || !displayContent.trim()) {
          return ''
        }
        return DOMPurify.sanitize(displayContent, {
          ALLOWED_TAGS: [],
          ALLOWED_ATTR: [],
          KEEP_CONTENT: true,
        })
      },
      [displayContent]
    )

    return (
      <div className="border border-dashed border-l-4 border-l-amber-400 rounded-lg overflow-hidden bg-amber-950/20 my-2">
        <button
          onClick={() => setExpanded(!expanded)}
          className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-amber-950/30 transition-colors"
        >
          <Shield className="w-4 h-4 text-amber-400 flex-shrink-0" />
          <span className="text-sm text-amber-200 flex-1">
            External Content
          </span>
          {shouldCollapse && (
            expanded ? (
              <ChevronDown className="w-4 h-4 text-amber-400" />
            ) : (
              <ChevronRight className="w-4 h-4 text-amber-400" />
            )
          )}
        </button>
        <pre className="px-3 py-2 border-t border-amber-900/50 bg-amber-950/10 text-amber-100 whitespace-pre-wrap font-mono text-xs overflow-auto break-all">
          {sanitized}
        </pre>
      </div>
    )
  }

  // Parse based on type
  let summary = ''
  let details: React.ReactNode = null

  if (type === 'observed_from_primary_session') {
    const parsed = parseToolCall(content)
    summary = `${parsed.whatHappened || 'Action'}`
    if (parsed.parameters) {
      const filename = parsed.parameters.split('/').pop() || parsed.parameters
      summary += ` · ${filename}`
    }

    details = (
      <div className="space-y-2 text-sm">
        {parsed.workingDirectory && (
          <p className="text-gray-500 font-mono text-xs truncate">
            {parsed.workingDirectory}
          </p>
        )}
        {parsed.parameters && (
          <p className="text-gray-600">
            <span className="text-gray-400">Path:</span> {parsed.parameters}
          </p>
        )}
      </div>
    )
  } else if (type === 'observation') {
    const parsed = parseObservation(content)
    summary = `${parsed.type || 'Discovery'} · ${parsed.title || 'Observation'}`

    details = (
      <div className="space-y-3 text-sm">
        {parsed.subtitle && (
          <p className="text-gray-600 italic">{parsed.subtitle}</p>
        )}
        {parsed.facts && parsed.facts.length > 0 && (
          <div>
            <p className="text-xs text-gray-400 uppercase tracking-wider mb-1">Key facts:</p>
            <ul className="list-disc pl-4 space-y-0.5 text-gray-600">
              {parsed.facts.slice(0, expanded ? undefined : 3).map((fact, i) => (
                <li key={i}>{fact}</li>
              ))}
              {!expanded && parsed.facts.length > 3 && (
                <li className="text-gray-400">+{parsed.facts.length - 3} more...</li>
              )}
            </ul>
          </div>
        )}
        {parsed.filesRead && parsed.filesRead.length > 0 && (
          <p className="text-gray-500 text-xs">
            Files: {parsed.filesRead.join(', ')}
          </p>
        )}
      </div>
    )
  } else {
    // Unknown XML - show as code block
    summary = 'Structured content'
    details = (
      <CodeBlock code={content} language="xml" />
    )
  }

  return (
    <div className="border border-gray-200 rounded-lg overflow-hidden bg-white my-2">
      {/* Header - always visible */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-gray-50 transition-colors"
      >
        <Icon className="w-4 h-4 text-gray-400 flex-shrink-0" />
        <span className="text-sm text-gray-600 truncate flex-1">
          {summary}
        </span>
        {expanded ? (
          <ChevronDown className="w-4 h-4 text-gray-400" />
        ) : (
          <ChevronRight className="w-4 h-4 text-gray-400" />
        )}
      </button>

      {/* Expanded content */}
      {expanded && (
        <div className="px-3 py-2 border-t border-gray-100 bg-gray-50">
          {details}
        </div>
      )}
    </div>
  )
}

/**
 * Detect if content contains XML that should be rendered as a card
 */
export function detectXmlType(content: string): XmlCardProps['type'] | null {
  if (content.includes('<observed_from_primary_session>')) {
    return 'observed_from_primary_session'
  }
  if (content.includes('<observation>')) {
    return 'observation'
  }
  if (content.includes('<tool_call>')) {
    return 'tool_call'
  }
  if (content.includes('<local-command-stdout>') || content.includes('<local-command-stderr>')) {
    return 'local_command'
  }
  if (content.includes('<task-notification>')) {
    return 'task_notification'
  }
  if (content.includes('<command-name>')) {
    return 'command'
  }
  if (content.includes('<tool_use_error>')) {
    return 'tool_error'
  }
  if (/<untrusted-data-[a-f0-9-]+>/.test(content)) {
    return 'untrusted_data'
  }
  if (content.includes('<local-command-caveat>') || content.includes('<system-reminder>') || content.includes('<claude-mem-context>')) {
    return 'hidden'
  }
  // Check for any XML-like structure
  if (/<[a-z_]+>[\s\S]*<\/[a-z_]+>/i.test(content) && content.length > 100) {
    return 'unknown'
  }
  return null
}

/**
 * Extract XML blocks from content
 */
export function extractXmlBlocks(content: string): { xml: string; type: XmlCardProps['type'] }[] {
  const blocks: { xml: string; type: XmlCardProps['type'] }[] = []

  // Specific patterns with known types
  // Order matters: grouped command pattern must come before individual command-* hidden patterns
  const knownPatterns = [
    { regex: /<observed_from_primary_session>[\s\S]*?<\/observed_from_primary_session>/g, type: 'observed_from_primary_session' as const },
    { regex: /<observation>[\s\S]*?<\/observation>/g, type: 'observation' as const },
    { regex: /<tool_call>[\s\S]*?<\/tool_call>/g, type: 'tool_call' as const },
    { regex: /<local-command-stdout>[\s\S]*?<\/local-command-stdout>/g, type: 'local_command' as const },
    { regex: /<local-command-stderr>[\s\S]*?<\/local-command-stderr>/g, type: 'local_command' as const },
    { regex: /<task-notification>[\s\S]*?<\/task-notification>/g, type: 'task_notification' as const },
    { regex: /<tool_use_error>[\s\S]*?<\/tool_use_error>/g, type: 'tool_error' as const },
    { regex: /<untrusted-data-[a-f0-9-]+>[\s\S]*?<\/untrusted-data-[a-f0-9-]+>/g, type: 'untrusted_data' as const },
    // Command invocations: match either tag order (name-message-args or message-name-args)
    { regex: /<command-name>[\s\S]*?<\/command-name>\s*<command-message>[\s\S]*?<\/command-message>\s*<command-args>[\s\S]+?<\/command-args>/g, type: 'command' as const },
    { regex: /<command-message>[\s\S]*?<\/command-message>\s*<command-name>[\s\S]*?<\/command-name>\s*<command-args>[\s\S]+?<\/command-args>/g, type: 'command' as const },
    // Hidden: system noise that shouldn't render
    { regex: /<local-command-caveat>[\s\S]*?<\/local-command-caveat>/g, type: 'hidden' as const },
    { regex: /<system-reminder>[\s\S]*?<\/system-reminder>/g, type: 'hidden' as const },
    { regex: /<command-args>\s*<\/command-args>/g, type: 'hidden' as const },
    { regex: /<command-message>[\s\S]*?<\/command-message>/g, type: 'hidden' as const },
    { regex: /<command-name>[\s\S]*?<\/command-name>/g, type: 'hidden' as const },
    { regex: /<claude-mem-context>[\s\S]*?<\/claude-mem-context>/g, type: 'hidden' as const },
  ]

  const matchedRanges: Array<{ start: number; end: number }> = []

  for (const { regex, type } of knownPatterns) {
    let match
    while ((match = regex.exec(content)) !== null) {
      // Skip if already matched by a previous pattern
      const isOverlapping = matchedRanges.some(
        range => match!.index >= range.start && match!.index < range.end
      )
      if (!isOverlapping) {
        blocks.push({ xml: match[0], type })
        matchedRanges.push({ start: match.index, end: match.index + match[0].length })
      }
    }
  }

  // Generic pattern for other XML-like structures
  // Only match if longer than 50 chars to avoid catching small inline tags
  const genericRegex = /<([a-z][a-z0-9_-]*)>[\s\S]*?<\/\1>/gi
  let match
  while ((match = genericRegex.exec(content)) !== null) {
    // Skip if already matched by a known pattern
    const isOverlapping = matchedRanges.some(
      range => match!.index >= range.start && match!.index < range.end
    )
    if (!isOverlapping && match[0].length > 20) {
      blocks.push({ xml: match[0], type: 'unknown' })
      matchedRanges.push({ start: match.index, end: match.index + match[0].length })
    }
  }

  // Sort by position in content
  blocks.sort((a, b) => content.indexOf(a.xml) - content.indexOf(b.xml))

  return blocks
}
