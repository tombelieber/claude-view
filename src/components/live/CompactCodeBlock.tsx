import { useState, useCallback, useMemo } from 'react'
import { Check, Copy } from 'lucide-react'
import { useExpandContext } from '../../contexts/ExpandContext'
import { useMonitorStore } from '../../store/monitor-store'
import { useTheme } from '../../hooks/use-theme'
import { useShikiHighlighter } from '../../hooks/use-shiki'
import { resolveLanguage } from '../../lib/shiki'

interface CompactCodeBlockProps {
  code: string | null | undefined
  language?: string | null | undefined
  blockId?: string
}

const COLLAPSE_THRESHOLD = 12

export function CompactCodeBlock({ code, language, blockId }: CompactCodeBlockProps) {
  const { expandedBlocks, toggleBlock } = useExpandContext()
  const verboseMode = useMonitorStore((s) => s.verboseMode)
  const isExpanded = verboseMode || (blockId ? expandedBlocks.has(blockId) : false)
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

  const isDiff = shikiLang === 'diff'

  // Shiki generates HTML from code strings we control (local JSONL session files).
  // Shiki's output is deterministic spans with inline styles — no script injection vector.
  const highlightedHtml = useMemo(() => {
    if (!highlighter) return null
    try {
      let html = highlighter.codeToHtml(displayCode, {
        lang: shikiLang,
        theme: shikiTheme,
      })
      // For diff: inject data-diff attributes on each <span class="line"> so CSS
      // can apply green/red/blue line backgrounds (GitHub-style).
      if (isDiff) {
        html = html.replace(/<span class="line">(.*?)<\/span>/g, (match, inner: string) => {
          // Strip HTML tags to get the raw text, then check prefix character
          const text = inner.replace(/<[^>]*>/g, '')
          if (text.startsWith('+')) return `<span class="line" data-diff="add">${inner}</span>`
          if (text.startsWith('-')) return `<span class="line" data-diff="del">${inner}</span>`
          if (text.startsWith('@@')) return `<span class="line" data-diff="hunk">${inner}</span>`
          return match
        })
      }
      return html
    } catch {
      return null
    }
  }, [highlighter, displayCode, shikiLang, shikiTheme, isDiff])

  return (
    <div className="relative my-1 rounded overflow-hidden border border-gray-200 dark:border-gray-700/60">
      {/* Copy button — top-right icon only */}
      <button
        onClick={handleCopy}
        className="absolute top-1 right-1 p-1 rounded text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors z-10"
        title="Copy"
      >
        {copied ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
      </button>

      {/* Code — Shiki highlighted or plain fallback */}
      {highlightedHtml ? (
        <div
          className="px-2 py-1.5 text-[11px] overflow-x-auto bg-gray-50 dark:bg-gray-900/80 [&_pre]:!m-0 [&_pre]:!p-0 [&_pre]:!bg-transparent [&_code]:!bg-transparent"
          dangerouslySetInnerHTML={{ __html: highlightedHtml }}
        />
      ) : (
        <pre className="px-2 py-1.5 text-[11px] overflow-x-auto bg-gray-50 dark:bg-gray-900/80 m-0">
          <code>{displayCode}</code>
        </pre>
      )}

      {/* Collapse/Expand control — hidden in verbose mode (always expanded) */}
      {shouldCollapse && !verboseMode && (
        <div className="border-t border-gray-200 dark:border-gray-700/60">
          <button
            onClick={() => blockId && toggleBlock(blockId)}
            className="w-full px-2 py-1 text-[10px] text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors text-center"
          >
            {isExpanded ? '[ Collapse ]' : `[ Show ${remainingLines} more lines ]`}
          </button>
        </div>
      )}
    </div>
  )
}
