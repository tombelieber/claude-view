import { useState } from 'react'
import { FileText, Pencil, Wrench, Terminal, Search, FolderSearch, ChevronDown, ChevronRight } from 'lucide-react'
import type { ToolCall } from '../hooks/use-session'

const TOOL_ICONS: Record<string, React.ReactNode> = {
  Read: <FileText className="w-3.5 h-3.5" />,
  Write: <Pencil className="w-3.5 h-3.5" />,
  Edit: <Wrench className="w-3.5 h-3.5" />,
  Bash: <Terminal className="w-3.5 h-3.5" />,
  Glob: <FolderSearch className="w-3.5 h-3.5" />,
  Grep: <Search className="w-3.5 h-3.5" />,
}

function getToolIcon(toolName: string): React.ReactNode {
  return TOOL_ICONS[toolName] || <Wrench className="w-3.5 h-3.5" />
}

interface ToolBadgeProps {
  toolCalls: ToolCall[]
}

export function ToolBadge({ toolCalls }: ToolBadgeProps) {
  const [isExpanded, setIsExpanded] = useState(false)

  if (toolCalls.length === 0) return null

  const totalCount = toolCalls.reduce((sum, tc) => sum + tc.count, 0)

  return (
    <div className="mt-3">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full text-left px-3 py-2 bg-gray-100 border border-gray-200 rounded-lg text-sm text-gray-600 hover:bg-gray-50 transition-colors duration-150 cursor-pointer focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
        aria-expanded={isExpanded}
        aria-label={`Tool calls: ${totalCount} total`}
      >
        <span className="mr-2 inline-flex" aria-hidden="true">
          {isExpanded ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
        </span>
        <span className="inline-flex items-center gap-1.5">
          {toolCalls.map((tc) => (
            <span key={tc.name} className="inline-flex items-center gap-0.5" aria-hidden="true">
              {getToolIcon(tc.name)}
              <span>{tc.name}</span>
            </span>
          ))}
        </span>
        <span className="text-gray-400 ml-2">({totalCount} {totalCount === 1 ? 'call' : 'calls'})</span>
      </button>

      {isExpanded && (
        <div className="mt-2 pl-6 space-y-2 text-sm text-gray-600">
          {toolCalls.map((tc) => (
            <div key={tc.name} className="flex items-center gap-2">
              <span aria-hidden="true">{getToolIcon(tc.name)}</span>
              <span>{tc.name}</span>
              <span className="text-gray-400">x {tc.count}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
