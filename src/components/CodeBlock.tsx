import { useState, useCallback } from 'react'
import { Highlight, themes } from 'prism-react-renderer'
import { Check, Copy } from 'lucide-react'
import { useExpandContext } from '../contexts/ExpandContext'
import { useTheme } from '../hooks/use-theme'

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
  const codeTheme = resolvedTheme === 'dark' ? themes.nightOwl : themes.github

  // Null safety: handle null/undefined code
  const safeCode = code || ''
  const safeLanguage = language || ''

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

  // Map common language aliases
  const normalizedLanguage = safeLanguage.toLowerCase()
  const languageMap: Record<string, string> = {
    'js': 'javascript',
    'ts': 'typescript',
    'tsx': 'tsx',
    'jsx': 'jsx',
    'py': 'python',
    'rb': 'ruby',
    'sh': 'bash',
    'shell': 'bash',
    'zsh': 'bash',
    'yml': 'yaml',
    'md': 'markdown',
  }
  const prismLanguage = languageMap[normalizedLanguage] || normalizedLanguage || 'text'

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

      {/* Code */}
      <Highlight theme={codeTheme} code={displayCode} language={prismLanguage}>
        {({ className, style, tokens, getLineProps, getTokenProps }) => (
          <pre
            className={`${className} p-3 text-sm overflow-x-auto`}
            style={{ ...style, margin: 0 }}
          >
            {tokens.map((line, i) => (
              <div key={i} {...getLineProps({ line })}>
                {line.map((token, key) => (
                  <span key={key} {...getTokenProps({ token })} />
                ))}
              </div>
            ))}
          </pre>
        )}
      </Highlight>

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
