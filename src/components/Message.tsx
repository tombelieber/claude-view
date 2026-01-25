import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import type { Message as MessageType } from '../hooks/use-session'
import { ToolBadge } from './ToolBadge'
import { CodeBlock } from './CodeBlock'
import { cn } from '../lib/utils'

interface MessageProps {
  message: MessageType
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

export function Message({ message }: MessageProps) {
  const isUser = message.role === 'user'
  const time = formatTime(message.timestamp)

  return (
    <div
      className={cn(
        'p-4 rounded-lg',
        isUser ? 'bg-white border border-gray-200' : 'bg-gray-50'
      )}
    >
      {/* Header */}
      <div className="flex items-start gap-3 mb-3">
        {/* Avatar */}
        <div
          className={cn(
            'w-8 h-8 rounded flex items-center justify-center text-white font-semibold text-sm flex-shrink-0',
            isUser ? 'bg-blue-500' : 'bg-orange-500'
          )}
        >
          {isUser ? 'U' : 'C'}
        </div>

        {/* Name and timestamp */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center justify-between gap-2">
            <span className="font-medium text-gray-900">
              {isUser ? 'You' : 'Claude'}
            </span>
            {time && (
              <span className="text-xs text-gray-400">{time}</span>
            )}
          </div>
        </div>
      </div>

      {/* Content */}
      <div className="pl-11">
        <div className="prose prose-sm prose-gray max-w-none break-words">
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            components={{
              code({ className, children, ...props }) {
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

                return (
                  <CodeBlock
                    code={String(children).replace(/\n$/, '')}
                    language={match?.[1]}
                  />
                )
              },
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
            {message.content}
          </ReactMarkdown>
        </div>

        {/* Tool calls badge */}
        {message.toolCalls && message.toolCalls.length > 0 && (
          <ToolBadge toolCalls={message.toolCalls} />
        )}
      </div>
    </div>
  )
}
