import {
  isCodeLikeContent,
  isDiffContent,
  isJsonContent,
  tryParseJson,
} from '../../../../utils/content-detection'
import { CopyButton } from '../shared/CopyButton'

const MAX_CHARS = 2000

interface ContentRendererProps {
  content: string
  maxHeight?: number
}

function PreBlock({
  content,
  className,
  maxHeight,
  children,
}: {
  content: string
  className: string
  maxHeight: number
  children?: React.ReactNode
}) {
  return (
    <div className="relative group">
      <pre className={className} style={{ maxHeight }}>
        {children ?? content}
      </pre>
      <div className="absolute top-1 right-1 opacity-0 group-hover:opacity-100 transition-opacity duration-200">
        <CopyButton text={content} />
      </div>
    </div>
  )
}

function DiffBlock({
  content,
  rawContent,
  maxHeight,
}: { content: string; rawContent: string; maxHeight: number }) {
  const lines = content.split('\n')
  return (
    <div className="relative group">
      <pre
        className="overflow-auto whitespace-pre-wrap rounded bg-gray-100 dark:bg-gray-900 p-2 text-xs font-mono"
        style={{ maxHeight }}
      >
        {lines.map((line, i) => {
          let color = 'text-gray-600 dark:text-gray-400'
          if (line.startsWith('+')) color = 'text-green-700 dark:text-green-400'
          else if (line.startsWith('-')) color = 'text-red-700 dark:text-red-400'
          else if (line.startsWith('@@')) color = 'text-cyan-700 dark:text-cyan-400'
          return (
            // biome-ignore lint/suspicious/noArrayIndexKey: static diff output from split — lines can duplicate, never reordered
            <span key={i} className={color}>
              {line}
              {'\n'}
            </span>
          )
        })}
      </pre>
      <div className="absolute top-1 right-1 opacity-0 group-hover:opacity-100 transition-opacity duration-200">
        <CopyButton text={rawContent} />
      </div>
    </div>
  )
}

export function ContentRenderer({ content, maxHeight = 200 }: ContentRendererProps) {
  if (!content) return null

  const truncated = content.length > MAX_CHARS
  const display = truncated ? content.slice(0, MAX_CHARS) : content

  // JSON
  if (isJsonContent(display)) {
    const parsed = tryParseJson(display)
    const jsonText = JSON.stringify(parsed, null, 2) + (truncated ? '\n...truncated' : '')
    return (
      <PreBlock
        content={jsonText}
        className="overflow-auto whitespace-pre-wrap rounded bg-gray-100 dark:bg-gray-900 p-2 text-xs font-mono text-gray-700 dark:text-gray-300"
        maxHeight={maxHeight}
      />
    )
  }

  // Diff
  if (isDiffContent(display)) {
    return (
      <DiffBlock
        content={display + (truncated ? '\n...truncated' : '')}
        rawContent={display}
        maxHeight={maxHeight}
      />
    )
  }

  // Code-like
  if (isCodeLikeContent(display)) {
    const codeText = display + (truncated ? '\n...truncated' : '')
    return (
      <PreBlock
        content={codeText}
        className="overflow-auto whitespace-pre-wrap rounded bg-gray-100 dark:bg-gray-900 p-2 text-xs font-mono text-gray-700 dark:text-gray-300"
        maxHeight={maxHeight}
      />
    )
  }

  // Plain text fallback
  const plainText = display + (truncated ? '\n...truncated' : '')
  return (
    <PreBlock
      content={plainText}
      className="overflow-auto whitespace-pre-wrap rounded bg-gray-100 dark:bg-gray-900 p-2 text-xs font-mono text-gray-700 dark:text-gray-300"
      maxHeight={maxHeight}
    />
  )
}
