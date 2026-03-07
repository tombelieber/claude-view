import { Check, Copy } from 'lucide-react'
import { useCallback, useState } from 'react'

interface CodeBlockProps {
  code: string | null | undefined
  language?: string | null | undefined
  blockId?: string
}

const COLLAPSE_THRESHOLD = 25

export function CodeBlock({ code, language }: CodeBlockProps) {
  const [copied, setCopied] = useState(false)
  const [isExpanded, setIsExpanded] = useState(false)

  const safeCode = code || ''
  const lines = safeCode.split('\n')
  const shouldCollapse = lines.length > COLLAPSE_THRESHOLD
  const displayCode =
    shouldCollapse && !isExpanded ? lines.slice(0, COLLAPSE_THRESHOLD).join('\n') : safeCode
  const remainingLines = lines.length - COLLAPSE_THRESHOLD

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(safeCode)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {
      // Clipboard API may not be available in all contexts
    }
  }, [safeCode])

  return (
    <div className="relative my-3 rounded-lg overflow-hidden border border-gray-200 dark:border-gray-700">
      <div className="flex items-center justify-between px-3 py-1.5 bg-gray-100 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
        <span className="text-xs text-gray-500 dark:text-gray-400 font-mono">
          {language || 'text'}
        </span>
        <button
          onClick={handleCopy}
          className="flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 transition-colors"
          title="Copy code"
        >
          {copied ? (
            <>
              <Check className="w-3.5 h-3.5" />
              <span>Copied</span>
            </>
          ) : (
            <>
              <Copy className="w-3.5 h-3.5" />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>

      <pre className="p-3 text-sm overflow-x-auto bg-gray-50 dark:bg-gray-900 m-0">
        <code>{displayCode}</code>
      </pre>

      {shouldCollapse && (
        <div className="border-t border-gray-200 dark:border-gray-700">
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="w-full px-3 py-2 text-xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors text-center"
          >
            {isExpanded ? '[ Collapse ]' : `[ Show ${remainingLines} more lines ]`}
          </button>
        </div>
      )}
    </div>
  )
}
