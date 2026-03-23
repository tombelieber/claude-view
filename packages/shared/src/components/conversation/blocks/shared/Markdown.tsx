import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'

interface MarkdownProps {
  content: string
}

/**
 * Lightweight Markdown renderer for conversation blocks.
 * Uses basic styling without the Shiki-based CompactCodeBlock from apps/web.
 * Fenced code blocks render as styled <pre> blocks.
 */
export function Markdown({ content }: MarkdownProps) {
  return (
    <div className="text-sm text-gray-900 dark:text-gray-100 leading-relaxed prose-sm max-w-none">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={{
          pre({ children }) {
            return (
              <pre className="text-[11px] font-mono overflow-x-auto rounded bg-gray-50 dark:bg-gray-900/80 p-2 my-1">
                {children}
              </pre>
            )
          },
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
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  )
}
