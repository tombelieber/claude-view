import { FileText, Terminal, Pencil, Eye } from 'lucide-react'
import { cn } from '../lib/utils'
import type { SessionInfo } from '../hooks/use-projects'

interface SessionCardProps {
  session: SessionInfo
  isSelected: boolean
  isActive: boolean
  onClick: () => void
}

function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))

  const timeStr = date.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })

  if (diffDays === 0) {
    return `Today, ${timeStr}`
  } else if (diffDays === 1) {
    return `Yesterday, ${timeStr}`
  } else if (diffDays < 7) {
    const dayName = date.toLocaleDateString('en-US', { weekday: 'long' })
    return `${dayName}, ${timeStr}`
  } else {
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
    }) + `, ${timeStr}`
  }
}

export function SessionCard({ session, isSelected, isActive, onClick }: SessionCardProps) {
  const toolCounts = session.toolCounts ?? { edit: 0, read: 0, bash: 0, write: 0 }
  const totalTools = toolCounts.edit + toolCounts.bash + toolCounts.read + toolCounts.write

  return (
    <button
      onClick={onClick}
      className={cn(
        'w-full text-left p-4 rounded-lg border transition-all',
        isSelected
          ? 'bg-blue-50 border-blue-500 shadow-[0_0_0_1px_#3b82f6]'
          : 'bg-white border-gray-200 hover:bg-gray-50 hover:border-gray-300 hover:shadow-sm',
        isActive && 'border-l-2 border-l-green-500'
      )}
    >
      {/* Header: Started message + Active indicator */}
      <div className="flex items-start justify-between gap-2">
        <div className="flex-1 min-w-0">
          <p className="text-sm text-gray-900 line-clamp-2">
            <span className="text-gray-400 text-xs font-medium">Started:</span>{' '}
            "{session.preview}"
          </p>

          {/* Last message if different from first */}
          {session.lastMessage && session.lastMessage !== session.preview && (
            <p className="text-sm text-gray-600 line-clamp-1 mt-1">
              <span className="text-gray-400 text-xs font-medium">Ended:</span>{' '}
              "{session.lastMessage}"
            </p>
          )}
        </div>

        {isActive && (
          <span className="flex items-center gap-1 text-xs text-green-600 flex-shrink-0">
            <span className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
            Active
          </span>
        )}
      </div>

      {/* Files touched */}
      {(session.filesTouched?.length ?? 0) > 0 && (
        <div className="flex items-center gap-1.5 mt-3 text-xs text-gray-500">
          <FileText className="w-3.5 h-3.5 text-gray-400" />
          <span className="truncate">
            {session.filesTouched?.join(', ')}
          </span>
        </div>
      )}

      {/* Footer: Tool counts + Skills + Timestamp */}
      <div className="flex items-center justify-between mt-3 pt-3 border-t border-gray-100">
        <div className="flex items-center gap-3">
          {/* Tool counts */}
          {totalTools > 0 && (
            <div className="flex items-center gap-2 text-xs text-gray-400">
              {toolCounts.edit > 0 && (
                <span className="flex items-center gap-0.5" title="Edits">
                  <Pencil className="w-3 h-3" />
                  {toolCounts.edit}
                </span>
              )}
              {toolCounts.bash > 0 && (
                <span className="flex items-center gap-0.5" title="Bash commands">
                  <Terminal className="w-3 h-3" />
                  {toolCounts.bash}
                </span>
              )}
              {toolCounts.read > 0 && (
                <span className="flex items-center gap-0.5" title="File reads">
                  <Eye className="w-3 h-3" />
                  {toolCounts.read}
                </span>
              )}
            </div>
          )}

          {/* Skills used */}
          {(session.skillsUsed?.length ?? 0) > 0 && (
            <div className="flex items-center gap-1">
              {session.skillsUsed?.slice(0, 2).map(skill => (
                <span
                  key={skill}
                  className="px-1.5 py-0.5 text-xs bg-gray-100 text-gray-600 rounded font-mono"
                >
                  {skill}
                </span>
              ))}
              {(session.skillsUsed?.length ?? 0) > 2 && (
                <span className="text-xs text-gray-400">
                  +{(session.skillsUsed?.length ?? 0) - 2}
                </span>
              )}
            </div>
          )}
        </div>

        {/* Timestamp */}
        <p className="text-xs text-gray-400 tabular-nums">
          {formatRelativeTime(session.modifiedAt)}
        </p>
      </div>
    </button>
  )
}
