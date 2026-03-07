import type { Components } from 'react-markdown'
import Markdown from 'react-markdown'
import rehypeRaw from 'rehype-raw'
import remarkGfm from 'remark-gfm'
import { CodeBlock } from './CodeBlock'

interface MarkdownContentProps {
  content: string
}

const components: Components = {
  pre({ children }) {
    return <>{children}</>
  },
  code({ className, children }) {
    const match = /language-(\w+)/.exec(className || '')
    const code = String(children).replace(/\n$/, '')

    if (match) {
      return <CodeBlock code={code} language={match[1]} />
    }

    // Inline code
    return (
      <code className="px-1.5 py-0.5 rounded text-sm bg-gray-100 dark:bg-gray-800 text-gray-800 dark:text-gray-200 font-mono">
        {children}
      </code>
    )
  },
  a({ href, children }) {
    return (
      <a
        href={href}
        target="_blank"
        rel="noopener noreferrer"
        className="text-blue-600 dark:text-blue-400 hover:underline"
      >
        {children}
      </a>
    )
  },
  table({ children }) {
    return (
      <div className="overflow-x-auto my-3">
        <table className="min-w-full text-sm border border-gray-200 dark:border-gray-700">
          {children}
        </table>
      </div>
    )
  },
  th({ children }) {
    return (
      <th className="px-3 py-2 text-left font-medium bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
        {children}
      </th>
    )
  },
  td({ children }) {
    return <td className="px-3 py-2 border-b border-gray-100 dark:border-gray-800">{children}</td>
  },
}

export function MarkdownContent({ content }: MarkdownContentProps) {
  return (
    <div className="prose prose-sm dark:prose-invert max-w-none prose-p:my-2 prose-li:my-0.5 prose-headings:mt-4 prose-headings:mb-2 prose-blockquote:text-gray-600 dark:prose-blockquote:text-gray-400 prose-blockquote:border-gray-300 dark:prose-blockquote:border-gray-600">
      <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw]} components={components}>
        {content}
      </Markdown>
    </div>
  )
}
