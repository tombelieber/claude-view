import { useState, useCallback } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { User, Copy, Check } from 'lucide-react'
import type { Message as MessageType } from '../hooks/use-session'
import { ToolBadge } from './ToolBadge'
import { CodeBlock } from './CodeBlock'
import { XmlCard, extractXmlBlocks } from './XmlCard'
import { ThinkingBlock } from './ThinkingBlock'
import { cn } from '../lib/utils'

interface MessageProps {
  message: MessageType
  messageIndex?: number
}

function formatTime(timestamp?: string): string | null {
  if (!timestamp) return null

  const date = new Date(timestamp)
  return date.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })
}

/**
 * Process message content to extract XML blocks
 * Returns segments of text and XML for mixed rendering
 * Uses position tracking from original content to handle duplicate XML blocks correctly
 */
function processContent(content: string): Array<{ type: 'text' | 'xml'; content: string; xmlType?: 'observed_from_primary_session' | 'observation' | 'tool_call' | 'local_command' | 'task_notification' | 'command' | 'tool_error' | 'untrusted_data' | 'hidden' | 'unknown' }> {
  const xmlBlocks = extractXmlBlocks(content)

  if (xmlBlocks.length === 0) {
    return [{ type: 'text', content }]
  }

  // Find positions of each block in original content, tracking search offset for duplicates
  const blocksWithPositions: Array<{ xml: string; type: 'observed_from_primary_session' | 'observation' | 'tool_call' | 'local_command' | 'task_notification' | 'command' | 'tool_error' | 'untrusted_data' | 'hidden' | 'unknown'; index: number }> = []
  let searchOffset = 0

  for (const block of xmlBlocks) {
    const index = content.indexOf(block.xml, searchOffset)
    if (index !== -1) {
      blocksWithPositions.push({ ...block, index })
      searchOffset = index + block.xml.length
    }
  }

  // Sort by position (should already be sorted, but ensure correctness)
  blocksWithPositions.sort((a, b) => a.index - b.index)

  const segments: Array<{ type: 'text' | 'xml'; content: string; xmlType?: 'observed_from_primary_session' | 'observation' | 'tool_call' | 'local_command' | 'task_notification' | 'command' | 'tool_error' | 'untrusted_data' | 'hidden' | 'unknown' }> = []
  let lastIndex = 0

  for (const block of blocksWithPositions) {
    // Add text before this XML block
    if (block.index > lastIndex) {
      const textBefore = content.substring(lastIndex, block.index).trim()
      if (textBefore) {
        segments.push({ type: 'text', content: textBefore })
      }
    }

    // Add the XML block
    segments.push({ type: 'xml', content: block.xml, xmlType: block.type })
    lastIndex = block.index + block.xml.length
  }

  // Add any remaining text after the last XML block
  if (lastIndex < content.length) {
    const textAfter = content.substring(lastIndex).trim()
    if (textAfter) {
      segments.push({ type: 'text', content: textAfter })
    }
  }

  return segments
}

export function Message({ message, messageIndex }: MessageProps) {
  const isUser = message.role === 'user'
  const time = formatTime(message.timestamp)
  const [copied, setCopied] = useState(false)

  const handleCopyMessage = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(message.content)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch (err) {
      console.error('Failed to copy message:', err)
    }
  }, [message.content])

  return (
    <div
      className={cn(
        'p-4 rounded-lg group',
        isUser ? 'bg-white border border-gray-200' : 'bg-gray-50'
      )}
    >
      {/* Header */}
      <div className="flex items-start gap-3 mb-3">
        {/* Avatar */}
        {isUser ? (
          <div className="w-8 h-8 rounded flex items-center justify-center bg-gray-200 text-gray-600 flex-shrink-0">
            <User className="w-4 h-4" />
          </div>
        ) : (
          <div className="w-8 h-8 rounded flex items-center justify-center bg-orange-500 text-white font-semibold text-sm flex-shrink-0">
            C
          </div>
        )}

        {/* Name and timestamp */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center justify-between gap-2">
            <span className="font-medium text-gray-900">
              {isUser ? 'Human' : 'Claude'}
            </span>
            <div className="flex items-center gap-2">
              <button
                onClick={handleCopyMessage}
                className="opacity-0 group-hover:opacity-100 flex items-center gap-1 px-1.5 py-0.5 text-xs text-gray-400 hover:text-gray-600 transition-all"
                title="Copy message"
              >
                {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
              </button>
              {time && (
                <span className="text-xs text-gray-400">{time}</span>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Content */}
      <div className="pl-11">
        {/* Thinking block (merged from preceding thinking-only message) */}
        {message.thinking && (
          <ThinkingBlock thinking={message.thinking} />
        )}

        {processContent(message.content).map((segment, i) => {
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
            <div key={i} className="prose prose-sm prose-gray max-w-none break-words">
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
                            className="px-1.5 py-0.5 bg-gray-100 rounded text-sm font-mono"
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
                    // Let the code component handle the pre
                    return <>{children}</>
                  },
                  // Styling for other elements
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
                      <blockquote className="border-l-4 border-gray-300 pl-4 italic text-gray-600 my-2">
                        {children}
                      </blockquote>
                    )
                  },
                  h1({ children }) {
                    return <h1 className="text-xl font-bold mt-4 mb-2">{children}</h1>
                  },
                  h2({ children }) {
                    return <h2 className="text-lg font-bold mt-3 mb-2">{children}</h2>
                  },
                  h3({ children }) {
                    return <h3 className="text-base font-bold mt-2 mb-1">{children}</h3>
                  },
                  table({ children }) {
                    return (
                      <div className="overflow-x-auto my-2">
                        <table className="min-w-full border border-gray-200">{children}</table>
                      </div>
                    )
                  },
                  th({ children }) {
                    return (
                      <th className="px-3 py-2 bg-gray-100 border border-gray-200 text-left font-medium">
                        {children}
                      </th>
                    )
                  },
                  td({ children }) {
                    return (
                      <td className="px-3 py-2 border border-gray-200">{children}</td>
                    )
                  },
                }}
              >
                {segment.content}
              </ReactMarkdown>
            </div>
          )
        })}

        {/* Tool calls badge */}
        {message.toolCalls && message.toolCalls.length > 0 && (
          <ToolBadge toolCalls={message.toolCalls} />
        )}
      </div>
    </div>
  )
}
