import { useState, useCallback } from 'react'
import { Highlight, themes } from 'prism-react-renderer'
import { Check, Copy } from 'lucide-react'
import { useExpandContext } from '../contexts/ExpandContext'

interface CodeBlockProps {
  code: string
  language?: string
  blockId?: string
}

const COLLAPSE_THRESHOLD = 25

export function CodeBlock({ code, language = '', blockId }: CodeBlockProps) {
  const { expandedBlocks, toggleBlock } = useExpandContext()
  const isExpanded = blockId ? expandedBlocks.has(blockId) : false
  const [copied, setCopied] = useState(false)

  const lines = code.split('\n')
  const shouldCollapse = lines.length > COLLAPSE_THRESHOLD
  const displayCode = shouldCollapse && !isExpanded
    ? lines.slice(0, COLLAPSE_THRESHOLD).join('\n')
    : code
  const remainingLines = lines.length - COLLAPSE_THRESHOLD

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch (err) {
      console.error('Failed to copy code:', err)
    }
  }, [code])

  // Map common language aliases
  const normalizedLanguage = language.toLowerCase()
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
    <div className="relative my-3 rounded-lg overflow-hidden border border-gray-200">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-1.5 bg-gray-100 border-b border-gray-200">
        <span className="text-xs text-gray-500 font-mono">
          {language || 'text'}
        </span>
        <button
          onClick={handleCopy}
          className="flex items-center gap-1 px-2 py-1 text-xs text-gray-500 hover:text-gray-700 transition-colors"
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
      <Highlight theme={themes.github} code={displayCode} language={prismLanguage}>
        {({ className, style, tokens, getLineProps, getTokenProps }) => (
          <pre
            className={`${className} p-3 text-sm overflow-x-auto`}
            style={{ ...style, margin: 0, background: '#f9fafb' }}
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
        <div className="border-t border-gray-200">
          <button
            onClick={() => blockId && toggleBlock(blockId)}
            className="w-full px-3 py-2 text-xs text-gray-500 hover:text-gray-700 hover:bg-gray-50 transition-colors text-center"
          >
            {isExpanded ? '[ Collapse ]' : `[ Show ${remainingLines} more lines ]`}
          </button>
        </div>
      )}
    </div>
  )
}
