import { useState } from 'react'
import type { ToolCall } from '../hooks/use-session'

const TOOL_ICONS: Record<string, string> = {
  Read: '📄',
  Write: '✏️',
  Edit: '🔧',
  Bash: '💻',
  Glob: '🔍',
  Grep: '🔎',
}

function getToolIcon(toolName: string): string {
  return TOOL_ICONS[toolName] || '🔧'
}

interface ToolBadgeProps {
  toolCalls: ToolCall[]
}

export function ToolBadge({ toolCalls }: ToolBadgeProps) {
  const [isExpanded, setIsExpanded] = useState(false)

  if (toolCalls.length === 0) return null

  const totalCount = toolCalls.reduce((sum, tc) => sum + tc.count, 0)

  // Create summary text like "📄 Read, 💻 Bash (5 calls)"
  const summaryParts = toolCalls.map(tc => `${getToolIcon(tc.name)} ${tc.name}`)
  const summaryText = summaryParts.join(', ')

  return (
    <div className="mt-3">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full text-left px-3 py-2 bg-gray-100 border border-gray-200 rounded-lg text-sm text-gray-600 hover:bg-gray-50 transition-colors"
      >
        <span className="mr-2">{isExpanded ? '▼' : '▶'}</span>
        <span>{summaryText}</span>
        <span className="text-gray-400 ml-2">({totalCount} {totalCount === 1 ? 'call' : 'calls'})</span>
      </button>

      {isExpanded && (
        <div className="mt-2 pl-6 space-y-2 text-sm text-gray-600">
          {toolCalls.map((tc) => (
            <div key={tc.name} className="flex items-center gap-2">
              <span>{getToolIcon(tc.name)}</span>
              <span>{tc.name}</span>
              <span className="text-gray-400">x {tc.count}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
