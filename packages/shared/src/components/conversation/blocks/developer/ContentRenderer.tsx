import {
  isCodeLikeContent,
  isDiffContent,
  isJsonContent,
  tryParseJson,
} from '../../../../utils/content-detection'

const MAX_CHARS = 2000

interface ContentRendererProps {
  content: string
  maxHeight?: number
}

function DiffBlock({ content, maxHeight }: { content: string; maxHeight: number }) {
  const lines = content.split('\n')
  return (
    <pre
      className="overflow-auto whitespace-pre-wrap rounded bg-gray-100 dark:bg-gray-900 p-2 text-[10px] font-mono"
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
  )
}

export function ContentRenderer({ content, maxHeight = 200 }: ContentRendererProps) {
  if (!content) return null

  const truncated = content.length > MAX_CHARS
  const display = truncated ? content.slice(0, MAX_CHARS) : content

  // JSON
  if (isJsonContent(display)) {
    const parsed = tryParseJson(display)
    return (
      <pre
        className="overflow-auto whitespace-pre-wrap rounded bg-gray-100 dark:bg-gray-900 p-2 text-[10px] font-mono text-gray-700 dark:text-gray-300"
        style={{ maxHeight }}
      >
        {JSON.stringify(parsed, null, 2)}
        {truncated && '\n...truncated'}
      </pre>
    )
  }

  // Diff
  if (isDiffContent(display)) {
    return (
      <DiffBlock content={display + (truncated ? '\n...truncated' : '')} maxHeight={maxHeight} />
    )
  }

  // Code-like
  if (isCodeLikeContent(display)) {
    return (
      <pre
        className="overflow-auto whitespace-pre-wrap rounded bg-gray-100 dark:bg-gray-900 p-2 text-[10px] font-mono text-gray-700 dark:text-gray-300"
        style={{ maxHeight }}
      >
        {display}
        {truncated && '\n...truncated'}
      </pre>
    )
  }

  // Plain text fallback
  return (
    <pre
      className="overflow-auto whitespace-pre-wrap rounded bg-gray-100 dark:bg-gray-900 p-2 text-[10px] font-mono text-gray-700 dark:text-gray-300"
      style={{ maxHeight }}
    >
      {display}
      {truncated && '\n...truncated'}
    </pre>
  )
}
