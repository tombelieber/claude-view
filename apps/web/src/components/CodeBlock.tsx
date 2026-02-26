import { useState, useCallback, useMemo } from 'react'
import { Check, Copy } from 'lucide-react'
import { useExpandContext } from '../contexts/ExpandContext'
import { useTheme } from '../hooks/use-theme'
import { useShikiHighlighter } from '../hooks/use-shiki'
import { resolveLanguage } from '../lib/shiki'

interface CodeBlockProps {
  code: string | null | undefined
  language?: string | null | undefined
  blockId?: string
}

const COLLAPSE_THRESHOLD = 25

export function CodeBlock({ code, language, blockId }: CodeBlockProps) {
  const { expandedBlocks, toggleBlock } = useExpandContext()
  const isExpanded = blockId ? expandedBlocks.has(blockId) : false
  const [copied, setCopied] = useState(false)
  const { resolvedTheme } = useTheme()
  const highlighter = useShikiHighlighter()

  const safeCode = code || ''
  const safeLanguage = language || ''
  const shikiLang = resolveLanguage(safeLanguage)
  const shikiTheme = resolvedTheme === 'dark' ? 'github-dark' : 'github-light'

  const lines = safeCode.split('\n')
  const shouldCollapse = lines.length > COLLAPSE_THRESHOLD
  const displayCode = shouldCollapse && !isExpanded
    ? lines.slice(0, COLLAPSE_THRESHOLD).join('\n')
    : safeCode
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

  // Shiki generates HTML from code strings we control (not user-submitted web content).
  // The input is source code from local JSONL session files, and Shiki's output is
  // deterministic spans with inline styles — no script injection vector.
  const highlightedHtml = useMemo(() => {
    if (!highlighter) return null
    try {
      return highlighter.codeToHtml(displayCode, {
        lang: shikiLang,
        theme: shikiTheme,
      })
    } catch {
      // Language not loaded or unknown — fall back to plain text
      return null
    }
  }, [highlighter, displayCode, shikiLang, shikiTheme])

  return (
    <div className="relative my-3 rounded-lg overflow-hidden border border-gray-200 dark:border-gray-700">
      {/* Header */}
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

      {/* Code — Shiki highlighted or plain fallback */}
      {highlightedHtml ? (
        <div
          className="p-3 text-sm overflow-x-auto [&_pre]:!m-0 [&_pre]:!p-0 [&_pre]:!bg-transparent [&_code]:!bg-transparent"
          dangerouslySetInnerHTML={{ __html: highlightedHtml }}
        />
      ) : (
        <pre className="p-3 text-sm overflow-x-auto bg-gray-50 dark:bg-gray-900 m-0">
          <code>{displayCode}</code>
        </pre>
      )}

      {/* Collapse/Expand control */}
      {shouldCollapse && (
        <div className="border-t border-gray-200 dark:border-gray-700">
          <button
            onClick={() => blockId && toggleBlock(blockId)}
            className="w-full px-3 py-2 text-xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors text-center"
          >
            {isExpanded ? '[ Collapse ]' : `[ Show ${remainingLines} more lines ]`}
          </button>
        </div>
      )}
    </div>
  )
}
