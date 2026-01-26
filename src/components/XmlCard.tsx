import { useState } from 'react'
import { ChevronRight, ChevronDown, FileText, Brain, Wrench, FileCode } from 'lucide-react'
import { cn } from '../lib/utils'
import { CodeBlock } from './CodeBlock'

interface XmlCardProps {
  content: string
  type: 'observed_from_primary_session' | 'observation' | 'tool_call' | 'unknown'
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
    default:
      return 'Structured Content'
  }
}

export function XmlCard({ content, type }: XmlCardProps) {
  const [expanded, setExpanded] = useState(false)

  const Icon = getIcon(type)
  const label = getLabel(type)

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

  const patterns = [
    { regex: /<observed_from_primary_session>[\s\S]*?<\/observed_from_primary_session>/g, type: 'observed_from_primary_session' as const },
    { regex: /<observation>[\s\S]*?<\/observation>/g, type: 'observation' as const },
  ]

  for (const { regex, type } of patterns) {
    const matches = content.match(regex)
    if (matches) {
      for (const match of matches) {
        blocks.push({ xml: match, type })
      }
    }
  }

  return blocks
}
