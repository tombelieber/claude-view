import { Check, Copy } from 'lucide-react'
import { useCallback, useState } from 'react'
import { useExpandContext } from '../contexts/ExpandContext'

interface CompactCodeBlockProps {
  code: string | null | undefined
  language?: string | null | undefined
  blockId?: string
}

const COLLAPSE_THRESHOLD = 12

export function CompactCodeBlock({ code, language: _language, blockId }: CompactCodeBlockProps) {
  const { expandedBlocks, toggleBlock } = useExpandContext()
  const isExpanded = blockId ? expandedBlocks.has(blockId) : false
  const [copied, setCopied] = useState(false)

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
    } catch (err) {
      console.error('Failed to copy code:', err)
    }
  }, [safeCode])

  return (
    <div className="relative my-1 rounded overflow-hidden border border-gray-200 dark:border-gray-700/60">
      {/* Copy button */}
      <button
        onClick={handleCopy}
        className="absolute top-1 right-1 p-1 rounded text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors z-10"
        title="Copy"
      >
        {copied ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
      </button>

      {/* Code — plain text (no syntax highlighting) */}
      <pre className="px-2 py-1.5 text-xs overflow-x-auto bg-gray-50 dark:bg-gray-900/80 m-0">
        <code>{displayCode}</code>
      </pre>

      {/* Collapse/Expand control */}
      {shouldCollapse && (
        <div className="border-t border-gray-200 dark:border-gray-700/60">
          <button
            onClick={() => blockId && toggleBlock(blockId)}
            className="w-full px-2 py-1 text-xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors text-center"
          >
            {isExpanded ? '[ Collapse ]' : `[ Show ${remainingLines} more lines ]`}
          </button>
        </div>
      )}
    </div>
  )
}
