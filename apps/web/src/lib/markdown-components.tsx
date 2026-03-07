import type { Components } from 'react-markdown'
import { CompactCodeBlock } from '../components/live/CompactCodeBlock'

/** Monotonically increasing counter for globally unique code block IDs across all renders. */
let mdBlockCounter = 0

/**
 * Custom react-markdown `components` that route fenced code blocks
 * through CompactCodeBlock (Shiki highlighting, copy, collapse) and
 * give inline `code` a distinct visual treatment.
 */
export const markdownComponents: Components = {
  pre({ children }) {
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
    return <pre className="text-[11px] font-mono overflow-x-auto">{children}</pre>
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
}
