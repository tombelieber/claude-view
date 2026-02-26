import { useState } from 'react'
import { Wrench, ChevronRight, ChevronDown, Copy, Check } from 'lucide-react'
import { cn } from '../lib/utils'

interface ToolCallCardProps {
  name: string
  input: Record<string, unknown>
  description: string
  parameters?: Record<string, unknown>
  icon?: React.ReactNode
}

/**
 * Extracts a summary string from tool input for display in the collapsed header.
 * Prioritizes file_path, command, and pattern fields.
 */
function getInputSummary(input: Record<string, unknown>): string {
  if (input.file_path && typeof input.file_path === 'string') {
    return input.file_path
  }
  if (input.command && typeof input.command === 'string') {
    return input.command
  }
  if (input.pattern && typeof input.pattern === 'string') {
    return input.pattern
  }
  // Fallback: show first string value
  for (const value of Object.values(input)) {
    if (typeof value === 'string') return value
  }
  return ''
}

export function ToolCallCard({
  name,
  input,
  description,
  parameters,
  icon,
}: ToolCallCardProps) {
  const [expanded, setExpanded] = useState(false)
  const [copied, setCopied] = useState(false)

  const inputSummary = getInputSummary(input)

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(JSON.stringify(input, null, 2))
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {
      // Clipboard API unavailable (non-HTTPS or older browser)
    }
  }

  return (
    <div className="rounded-lg border border-purple-200 border-l-4 border-l-purple-300 overflow-hidden bg-white my-2">
      {/* Header - always visible */}
      <button
        onClick={() => setExpanded(!expanded)}
        aria-expanded={expanded}
        aria-label={`Tool Call ${name}`}
        className={cn(
          'w-full flex flex-col gap-1 px-3 py-2 text-left',
          'hover:bg-purple-50 transition-colors',
          'focus:outline-none focus-visible:ring-2 focus-visible:ring-purple-400 focus-visible:ring-offset-1'
        )}
      >
        <div className="flex items-center gap-2 w-full">
          {/* Icon */}
          {icon ? (
            <span className="flex-shrink-0">{icon}</span>
          ) : (
            <Wrench className="w-4 h-4 text-purple-500 flex-shrink-0" aria-hidden="true" />
          )}

          {/* Tool name */}
          {name && (
            <span className="text-sm font-semibold text-purple-700 flex-shrink-0">
              {name}
            </span>
          )}

          {/* Input summary - truncated */}
          {inputSummary && (
            <>
              <span className="text-purple-300 flex-shrink-0">&middot;</span>
              <span className="text-sm text-purple-600 truncate flex-1 font-mono">
                {inputSummary}
              </span>
            </>
          )}

          {/* Chevron */}
          {expanded ? (
            <ChevronDown className="w-4 h-4 text-purple-400 flex-shrink-0" />
          ) : (
            <ChevronRight className="w-4 h-4 text-purple-400 flex-shrink-0" />
          )}
        </div>

        {/* Description - always visible as subtitle */}
        {description && (
          <span className="text-xs text-purple-600 break-words pl-6">
            {description}
          </span>
        )}
      </button>

      {/* Expanded details */}
      {expanded && (
        <div className="px-3 py-2 border-t border-purple-100 bg-purple-50">
          {/* Description */}
          <p className="text-sm text-purple-800 break-words">{description}</p>

          {/* Parameters */}
          <div className="mt-2">
            {parameters ? (
              <pre className="text-xs text-purple-700 font-mono whitespace-pre-wrap break-all bg-purple-100 rounded p-2">
                {JSON.stringify(parameters, null, 2)}
              </pre>
            ) : (
              <p className="text-xs text-purple-500 italic">No parameters</p>
            )}
          </div>

          {/* Copy button */}
          <div className="mt-2 flex justify-end">
            <button
              onClick={handleCopy}
              aria-label={copied ? 'Copied' : 'Copy input'}
              className={cn(
                'flex items-center gap-1 px-2 py-1 text-xs rounded',
                'text-purple-600 hover:bg-purple-200 transition-colors',
                'focus:outline-none focus-visible:ring-2 focus-visible:ring-purple-400'
              )}
            >
              {copied ? (
                <Check className="w-3 h-3" aria-hidden="true" />
              ) : (
                <Copy className="w-3 h-3" aria-hidden="true" />
              )}
              {copied ? 'Copied' : 'Copy'}
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
