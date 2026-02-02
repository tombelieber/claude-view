/**
 * MessageTyped — Full 7-Type JSONL Parser Support
 *
 * Comprehensive redesign for new parser schema with visual type hierarchy.
 * Types: user, assistant, tool_use, tool_result, system, progress, summary
 *
 * Design: Editorial/refined with semantic color coding. Each type gets
 * icon + accent color + left border for instant visual recognition.
 * Supports dense conversations while maintaining clarity.
 */

import { useState, useCallback, useMemo } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import {
  User, Copy, Check, MessageSquare, Wrench, CheckCircle,
  AlertCircle, Zap, BookOpen
} from 'lucide-react'
import type { Message as MessageType } from '../hooks/use-session'
import { CodeBlock } from './CodeBlock'
import { XmlCard, extractXmlBlocks } from './XmlCard'
import { ThinkingBlock } from './ThinkingBlock'
import { cn } from '../lib/utils'
import { useThreadHighlight } from '../contexts/ThreadHighlightContext'

// System event cards
import { TurnDurationCard } from './TurnDurationCard'
import { ApiErrorCard } from './ApiErrorCard'
import { CompactBoundaryCard } from './CompactBoundaryCard'
import { HookSummaryCard } from './HookSummaryCard'
import { LocalCommandEventCard } from './LocalCommandEventCard'

// Progress event cards
import { AgentProgressCard } from './AgentProgressCard'
import { BashProgressCard } from './BashProgressCard'
import { HookProgressCard } from './HookProgressCard'
import { McpProgressCard } from './McpProgressCard'
import { TaskQueueCard } from './TaskQueueCard'

// Track 4: Queue, Snapshot, Summary cards
import { MessageQueueEventCard } from './MessageQueueEventCard'
import { FileSnapshotCard } from './FileSnapshotCard'
import { SessionSummaryCard } from './SessionSummaryCard'

/** Maximum nesting depth for thread indentation */
const MAX_INDENT_LEVEL = 5
/** Pixels per indent level (desktop) */
const INDENT_PX = 12

interface MessageTypedProps {
  message: MessageType
  messageIndex?: number
  messageType?: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'system' | 'progress' | 'summary'
  metadata?: Record<string, any>
  /** Parent message UUID for threading */
  parentUuid?: string
  /** Nesting level (0 = root, 1 = child, 2 = grandchild, etc.). Capped at MAX_INDENT_LEVEL. */
  indent?: number
  /** Whether this message is a child in a thread (shows connector line) */
  isChildMessage?: boolean
  /** Callback to get the full thread chain for highlighting */
  onGetThreadChain?: (uuid: string) => Set<string>
}

const TYPE_CONFIG = {
  user: {
    accent: 'border-blue-300 dark:border-blue-700',
    badge: 'bg-blue-100 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300',
    icon: User,
    label: 'You'
  },
  assistant: {
    accent: 'border-orange-300 dark:border-orange-700',
    badge: 'bg-orange-100 dark:bg-orange-900/40 text-orange-700 dark:text-orange-300',
    icon: MessageSquare,
    label: 'Claude'
  },
  tool_use: {
    accent: 'border-purple-300 dark:border-purple-700',
    badge: 'bg-purple-100 dark:bg-purple-900/40 text-purple-700 dark:text-purple-300',
    icon: Wrench,
    label: 'Tool'
  },
  tool_result: {
    accent: 'border-green-300 dark:border-green-700',
    badge: 'bg-green-100 dark:bg-green-900/40 text-green-700 dark:text-green-300',
    icon: CheckCircle,
    label: 'Result'
  },
  system: {
    accent: 'border-amber-300 dark:border-amber-700',
    badge: 'bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300',
    icon: AlertCircle,
    label: 'System'
  },
  progress: {
    accent: 'border-indigo-300 dark:border-indigo-700',
    badge: 'bg-indigo-100 dark:bg-indigo-900/40 text-indigo-700 dark:text-indigo-300',
    icon: Zap,
    label: 'Progress'
  },
  summary: {
    accent: 'border-rose-300 dark:border-rose-700',
    badge: 'bg-rose-100 dark:bg-rose-900/40 text-rose-700 dark:text-rose-300',
    icon: BookOpen,
    label: 'Summary'
  }
}

function formatTime(timestamp?: string | null): string | null {
  if (!timestamp) return null
  const date = new Date(timestamp)
  return date.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })
}

function processContent(content: string): Array<{
  type: 'text' | 'xml'
  content: string
  xmlType?: 'observed_from_primary_session' | 'observation' | 'tool_call' | 'local_command' | 'task_notification' | 'command' | 'tool_error' | 'untrusted_data' | 'hidden' | 'unknown'
}> {
  const xmlBlocks = extractXmlBlocks(content)
  if (xmlBlocks.length === 0) return [{ type: 'text', content }]

  const blocksWithPositions: Array<{
    xml: string
    type: 'observed_from_primary_session' | 'observation' | 'tool_call' | 'local_command' | 'task_notification' | 'command' | 'tool_error' | 'untrusted_data' | 'hidden' | 'unknown'
    index: number
  }> = []
  let searchOffset = 0

  for (const block of xmlBlocks) {
    const index = content.indexOf(block.xml, searchOffset)
    if (index !== -1) {
      blocksWithPositions.push({ ...block, index })
      searchOffset = index + block.xml.length
    }
  }

  blocksWithPositions.sort((a, b) => a.index - b.index)

  const segments: Array<{
    type: 'text' | 'xml'
    content: string
    xmlType?: any
  }> = []
  let lastIndex = 0

  for (const block of blocksWithPositions) {
    if (block.index > lastIndex) {
      const textBefore = content.substring(lastIndex, block.index).trim()
      if (textBefore) segments.push({ type: 'text', content: textBefore })
    }
    segments.push({ type: 'xml', content: block.xml, xmlType: block.type })
    lastIndex = block.index + block.xml.length
  }

  if (lastIndex < content.length) {
    const textAfter = content.substring(lastIndex).trim()
    if (textAfter) segments.push({ type: 'text', content: textAfter })
  }

  return segments
}

function SystemMetadataCard({ metadata, type }: { metadata?: Record<string, any>, type: 'system' | 'progress' }) {
  if (!metadata || Object.keys(metadata).length === 0) return null

  return (
    <div className={cn(
      'mt-3 p-3 rounded-sm text-sm',
      type === 'system'
        ? 'bg-amber-100/30 dark:bg-amber-900/20 border border-amber-200/50 dark:border-amber-700/30'
        : 'bg-indigo-100/30 dark:bg-indigo-900/20 border border-indigo-200/50 dark:border-indigo-700/30'
    )}>
      <div className="space-y-1 font-mono text-xs">
        {Object.entries(metadata).map(([key, value]) => {
          const displayValue = typeof value === 'object' ? JSON.stringify(value) : String(value)
          return (
            <div key={key} className="flex items-start gap-2">
              <span className={cn(
                'font-semibold flex-shrink-0',
                type === 'system' ? 'text-amber-700 dark:text-amber-300' : 'text-indigo-700 dark:text-indigo-300'
              )}>
                {key}:
              </span>
              <span className="text-gray-700 dark:text-gray-300 break-all">{displayValue}</span>
            </div>
          )
        })}
      </div>
    </div>
  )
}

function renderSystemSubtype(metadata: Record<string, any>): React.ReactNode | null {
  const subtype = metadata?.type ?? metadata?.subtype
  switch (subtype) {
    case 'turn_duration':
      return <TurnDurationCard durationMs={metadata.durationMs} startTime={metadata.startTime} endTime={metadata.endTime} />
    case 'api_error':
      return <ApiErrorCard error={metadata.error} retryAttempt={metadata.retryAttempt} maxRetries={metadata.maxRetries} retryInMs={metadata.retryInMs} />
    case 'compact_boundary':
      return <CompactBoundaryCard trigger={metadata.trigger} preTokens={metadata.preTokens} postTokens={metadata.postTokens} />
    case 'hook_summary':
      return <HookSummaryCard hookCount={metadata.hookCount} hookInfos={metadata.hookInfos} hookErrors={metadata.hookErrors} durationMs={metadata.durationMs} preventedContinuation={metadata.preventedContinuation} />
    case 'local_command':
      return <LocalCommandEventCard content={metadata.content} />
    case 'queue-operation':
      return <MessageQueueEventCard operation={metadata.operation} timestamp={metadata.timestamp || ''} content={metadata.content} />
    case 'file-history-snapshot': {
      const snapshot = metadata.snapshot || {}
      const files = Object.keys(snapshot.trackedFileBackups || {})
      return <FileSnapshotCard fileCount={files.length} timestamp={snapshot.timestamp || ''} files={files} isIncremental={metadata.isSnapshotUpdate || false} />
    }
    default:
      return null
  }
}

function renderProgressSubtype(metadata: Record<string, any>): React.ReactNode | null {
  const subtype = metadata?.type
  switch (subtype) {
    case 'agent_progress':
      return <AgentProgressCard agentId={metadata.agentId} prompt={metadata.prompt} model={metadata.model} tokens={metadata.tokens} normalizedMessages={metadata.normalizedMessages} indent={metadata.indent} />
    case 'bash_progress':
      return <BashProgressCard command={metadata.command} output={metadata.output} exitCode={metadata.exitCode} duration={metadata.duration} />
    case 'hook_progress':
      return <HookProgressCard hookEvent={metadata.hookEvent} hookName={metadata.hookName} command={metadata.command} output={metadata.output} />
    case 'mcp_progress':
      return <McpProgressCard server={metadata.server} method={metadata.method} params={metadata.params} result={metadata.result} />
    case 'waiting_for_task':
      return <TaskQueueCard waitDuration={metadata.waitDuration} position={metadata.position} queueLength={metadata.queueLength} />
    default:
      return null
  }
}

function TypeBadge({ type, label }: { type: keyof typeof TYPE_CONFIG, label: string }) {
  const config = TYPE_CONFIG[type]
  const Icon = config.icon

  return (
    <div className={cn('inline-flex items-center gap-1.5 px-2.5 py-1 rounded text-xs font-medium', config.badge)}>
      <Icon className="w-3.5 h-3.5" />
      {label}
    </div>
  )
}

export function MessageTyped({
  message,
  messageIndex,
  messageType = message.role as any,
  metadata,
  parentUuid,
  indent = 0,
  isChildMessage = false,
  onGetThreadChain,
}: MessageTypedProps) {
  const type = messageType as keyof typeof TYPE_CONFIG
  const config = TYPE_CONFIG[type]
  const Icon = config.icon
  const time = formatTime(message.timestamp)
  const [copied, setCopied] = useState(false)

  // Cap indent at MAX_INDENT_LEVEL
  const clampedIndent = Math.min(Math.max(indent, 0), MAX_INDENT_LEVEL)
  if (indent > MAX_INDENT_LEVEL) {
    console.warn(`Max nesting depth (${MAX_INDENT_LEVEL}) exceeded: indent=${indent}, clamped to ${clampedIndent}`)
  }
  const indentPx = clampedIndent * INDENT_PX

  const handleCopyMessage = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(message.content)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch (err) {
      console.error('Failed to copy message:', err)
    }
  }, [message.content])

  const { highlightedUuids, setHighlightedUuids, clearHighlight } = useThreadHighlight()
  const isHighlighted = message.uuid ? highlightedUuids.has(message.uuid) : false

  const handleMouseEnter = useCallback(() => {
    if (message.uuid && onGetThreadChain) {
      setHighlightedUuids(onGetThreadChain(message.uuid))
    }
  }, [message.uuid, onGetThreadChain, setHighlightedUuids])

  const handleMouseLeave = useCallback(() => {
    clearHighlight()
  }, [clearHighlight])

  // Memoize content processing to avoid re-running XML extraction on every render
  const contentSegments = useMemo(() => processContent(message.content), [message.content])

  if ((type === 'system' || type === 'progress') && !message.content && !message.thinking) {
    if (!metadata || Object.keys(metadata).length === 0) return null
  }

  return (
    <div
      role="article"
      aria-level={clampedIndent + 1}
      {...(parentUuid ? { 'data-parent-uuid': parentUuid } : {})}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      className={cn(
        'border-l-4 rounded-r-lg transition-colors',
        config.accent,
        'bg-white dark:bg-gray-900 hover:bg-gray-50/50 dark:hover:bg-gray-800/50',
        isHighlighted && 'bg-indigo-50/60 dark:bg-indigo-950/30',
        isChildMessage && 'thread-child'
      )}
      style={{
        paddingLeft: indentPx > 0 ? `${indentPx}px` : undefined,
        ...(isChildMessage
          ? { borderLeftWidth: '4px', borderLeftStyle: 'dashed' as const, borderLeftColor: '#9CA3AF' }
          : {}),
      }}
    >
      <div className="p-4 group">
        {/* Header */}
        <div className="flex items-start gap-3 mb-3">
          {/* Icon */}
          <div className={cn(
            'w-8 h-8 rounded-sm flex items-center justify-center flex-shrink-0',
            config.badge
          )}>
            <Icon className="w-4 h-4" />
          </div>

          {/* Metadata row */}
          <div className="flex-1 min-w-0">
            <div className="flex items-center justify-between gap-2 flex-wrap">
              <div className="flex items-center gap-2">
                <span className="font-semibold text-gray-900 dark:text-gray-100 text-sm">
                  {config.label}
                </span>
                {type !== message.role && (
                  <TypeBadge type={type} label={config.label} />
                )}
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={handleCopyMessage}
                  className="opacity-0 group-hover:opacity-100 flex items-center gap-1 px-1.5 py-0.5 text-xs text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300 transition-all"
                  title="Copy message"
                >
                  {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                </button>
                {time && (
                  <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap">{time}</span>
                )}
              </div>
            </div>
          </div>
        </div>

        {/* Content */}
        <div className="pl-11 space-y-3">
          {/* Thinking block */}
          {message.thinking && (
            <ThinkingBlock thinking={message.thinking} />
          )}

          {/* Main content */}
          {message.content && (
            <div className="space-y-3">
              {contentSegments.map((segment, i) => {
                if (segment.type === 'xml' && segment.xmlType) {
                  return (
                    <XmlCard
                      key={i}
                      content={segment.content}
                      type={segment.xmlType}
                    />
                  )
                }
                return (
                  <div key={i} className="prose prose-sm prose-gray dark:prose-invert max-w-none break-words text-sm">
                    <ReactMarkdown
                      remarkPlugins={[remarkGfm]}
                      components={{
                        code: (() => {
                          let blockCounter = 0
                          return ({ className, children, ...props }: any) => {
                            const match = /language-(\w+)/.exec(className || '')
                            const isInline = !match && !String(children).includes('\n')

                            if (isInline) {
                              return (
                                <code
                                  className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 rounded text-xs font-mono"
                                  {...props}
                                >
                                  {children}
                                </code>
                              )
                            }

                            const blockId = messageIndex !== undefined
                              ? `${messageIndex}-${blockCounter++}`
                              : undefined

                            return (
                              <CodeBlock
                                code={String(children).replace(/\n$/, '')}
                                language={match?.[1]}
                                blockId={blockId}
                              />
                            )
                          }
                        })(),
                        pre({ children }) {
                          return <>{children}</>
                        },
                        p({ children }) {
                          return <p className="mb-2 last:mb-0">{children}</p>
                        },
                        ul({ children }) {
                          return <ul className="list-disc pl-4 mb-2">{children}</ul>
                        },
                        ol({ children }) {
                          return <ol className="list-decimal pl-4 mb-2">{children}</ol>
                        },
                        li({ children }) {
                          return <li className="mb-1">{children}</li>
                        },
                        a({ href, children }) {
                          return (
                            <a
                              href={href}
                              className="text-blue-500 hover:text-blue-700 underline"
                              target="_blank"
                              rel="noopener noreferrer"
                            >
                              {children}
                            </a>
                          )
                        },
                        blockquote({ children }) {
                          return (
                            <blockquote className="border-l-4 border-gray-300 dark:border-gray-600 pl-4 italic text-gray-600 dark:text-gray-400 my-2">
                              {children}
                            </blockquote>
                          )
                        },
                        h1({ children }) {
                          return <h1 className="text-lg font-bold mt-3 mb-2">{children}</h1>
                        },
                        h2({ children }) {
                          return <h2 className="text-base font-bold mt-2 mb-2">{children}</h2>
                        },
                        h3({ children }) {
                          return <h3 className="text-sm font-bold mt-2 mb-1">{children}</h3>
                        },
                      }}
                    >
                      {segment.content}
                    </ReactMarkdown>
                  </div>
                )
              })}
            </div>
          )}

          {/* System/Progress metadata — dispatch to specialized cards */}
          {type === 'system' && metadata && (
            renderSystemSubtype(metadata) ?? <SystemMetadataCard metadata={metadata} type="system" />
          )}
          {type === 'progress' && metadata && (
            renderProgressSubtype(metadata) ?? <SystemMetadataCard metadata={metadata} type="progress" />
          )}

          {/* Summary card */}
          {type === 'summary' && metadata && (
            <SessionSummaryCard
              summary={metadata.summary || message.content}
              leafUuid={metadata.leafUuid || ''}
              wordCount={(metadata.summary || message.content || '').split(/\s+/).filter(Boolean).length}
            />
          )}

          {/* Tool calls summary */}
          {message.tool_calls && message.tool_calls.length > 0 && (
            <div className="mt-2 pt-3 border-t border-gray-200 dark:border-gray-700">
              <div className="text-xs font-semibold text-gray-600 dark:text-gray-400 mb-2">
                Tool Calls: {message.tool_calls.length}
              </div>
              <div className="flex flex-wrap gap-1.5">
                {message.tool_calls.map((tool, idx) => (
                  <div
                    key={idx}
                    className="px-2 py-1 bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded text-xs font-mono text-gray-700 dark:text-gray-300"
                  >
                    {tool.name}
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

export { TYPE_CONFIG, MAX_INDENT_LEVEL, INDENT_PX }
