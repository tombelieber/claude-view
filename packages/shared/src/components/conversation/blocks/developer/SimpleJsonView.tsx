import { useMemo } from 'react'
import type { JsonTreeProps } from '../../../../contexts/DeveloperToolsContext'

/**
 * Minimal JSON viewer with syntax coloring.
 * Tokenizes JSON output and applies distinct colors for keys, strings,
 * numbers, booleans, and null — zero external dependencies.
 */

interface ColoredToken {
  text: string
  kind: 'key' | 'string' | 'number' | 'boolean' | 'null' | 'punctuation'
}

/**
 * Tokenize a pretty-printed JSON string into colored spans.
 * Works on the output of JSON.stringify(data, null, 2).
 */
function tokenizeJson(json: string): ColoredToken[] {
  const tokens: ColoredToken[] = []
  // Match: strings (keys or values), numbers, booleans, null, and punctuation
  const regex =
    /("(?:\\.|[^"\\])*")\s*:|("(?:\\.|[^"\\])*")|([-+]?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|(\btrue\b|\bfalse\b)|(\bnull\b)|([[\]{},:])|([ \t\n]+)/g

  let lastIndex = 0

  for (let match = regex.exec(json); match !== null; match = regex.exec(json)) {
    // Capture any gap (shouldn't happen with well-formed JSON.stringify output)
    if (match.index > lastIndex) {
      tokens.push({ text: json.slice(lastIndex, match.index), kind: 'punctuation' })
    }
    lastIndex = regex.lastIndex

    if (match[1] !== undefined) {
      // Key (string followed by colon)
      tokens.push({ text: match[1], kind: 'key' })
      tokens.push({ text: ': ', kind: 'punctuation' })
    } else if (match[2] !== undefined) {
      // String value
      tokens.push({ text: match[2], kind: 'string' })
    } else if (match[3] !== undefined) {
      tokens.push({ text: match[3], kind: 'number' })
    } else if (match[4] !== undefined) {
      tokens.push({ text: match[4], kind: 'boolean' })
    } else if (match[5] !== undefined) {
      tokens.push({ text: match[5], kind: 'null' })
    } else if (match[6] !== undefined) {
      tokens.push({ text: match[6], kind: 'punctuation' })
    } else if (match[7] !== undefined) {
      // Whitespace — keep as punctuation for structure
      tokens.push({ text: match[7], kind: 'punctuation' })
    }
  }

  // Trailing content
  if (lastIndex < json.length) {
    tokens.push({ text: json.slice(lastIndex), kind: 'punctuation' })
  }

  return tokens
}

const TOKEN_COLORS: Record<ColoredToken['kind'], string> = {
  key: 'text-indigo-600 dark:text-indigo-400',
  string: 'text-emerald-700 dark:text-emerald-400',
  number: 'text-amber-600 dark:text-amber-400',
  boolean: 'text-cyan-600 dark:text-cyan-400',
  null: 'text-red-500 dark:text-red-400',
  punctuation: 'text-gray-500 dark:text-gray-500',
}

export function SimpleJsonView({ data }: JsonTreeProps) {
  const json = JSON.stringify(data, null, 2)
  const tokens = useMemo(() => tokenizeJson(json), [json])

  return (
    <pre className="text-xs font-mono leading-relaxed overflow-auto max-h-96 whitespace-pre-wrap break-all">
      {tokens.map((token, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: token array is static per render
        <span key={i} className={TOKEN_COLORS[token.kind]}>
          {token.text}
        </span>
      ))}
    </pre>
  )
}
