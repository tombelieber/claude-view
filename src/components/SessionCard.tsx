import { FileText, Terminal, Pencil, Eye, MessageSquare } from 'lucide-react'
import { cn } from '../lib/utils'
import type { SessionInfo } from '../hooks/use-projects'

interface SessionCardProps {
  session: SessionInfo
  isSelected: boolean
  onClick: () => void
  projectDisplayName?: string
}

/** Strip XML tags, system prompt noise, and quotes from preview text */
function cleanPreviewText(text: string): string {
  // Remove XML-like tags
  let cleaned = text.replace(/<[^>]+>/g, '')
  // Remove leading/trailing quotes
  cleaned = cleaned.replace(/^["']|["']$/g, '')
  // Remove slash-command prefixes like "/superpowers:brainstorm"
  cleaned = cleaned.replace(/\/[\w-]+:[\w-]+\s*/g, '')
  // Remove "superpowers:" prefixed words
  cleaned = cleaned.replace(/superpowers:\S+\s*/g, '')
  // Collapse whitespace
  cleaned = cleaned.replace(/\s+/g, ' ').trim()
  // If it starts with common system prompt patterns, show a clean label
  if (cleaned.startsWith('You are a ') || cleaned.startsWith('You are Claude')) {
    return 'System prompt session'
  }
  // If it looks like ls output or file listing
  if (cleaned.match(/^"?\s*total \d+/)) {
    return cleaned.slice(0, 100) + (cleaned.length > 100 ? '...' : '')
  }
  return cleaned || 'Untitled session'
}

function formatRelativeTime(timestamp: number): string {
  // timestamp is Unix seconds, convert to milliseconds for JavaScript Date
  const date = new Date(timestamp * 1000)
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

export function SessionCard({ session, isSelected, onClick, projectDisplayName }: SessionCardProps) {
  const toolCounts = session.toolCounts ?? { edit: 0, read: 0, bash: 0, write: 0 }
  const editCount = toolCounts.edit + toolCounts.write // Combined edit + write
  const totalTools = editCount + toolCounts.bash + toolCounts.read

  // Clean up preview text: strip XML tags and system prompt noise
  const cleanPreview = cleanPreviewText(session.preview)
  const cleanLast = session.lastMessage ? cleanPreviewText(session.lastMessage) : ''

  const projectLabel = projectDisplayName || undefined

  return (
    <button
      onClick={onClick}
      className={cn(
        'w-full text-left p-3.5 rounded-lg border transition-all',
        isSelected
          ? 'bg-blue-50 border-blue-500 shadow-[0_0_0_1px_#3b82f6]'
          : 'bg-white border-gray-200 hover:bg-gray-50 hover:border-gray-300 hover:shadow-sm'
      )}
    >
      {/* Header: Project badge + timestamp */}
      <div className="flex items-center justify-between gap-2 mb-1">
        <div className="flex items-center gap-1.5 min-w-0">
          {projectLabel && (
            <span className="inline-block px-1.5 py-0.5 text-[10px] font-medium bg-gray-100 text-gray-600 rounded flex-shrink-0">
              {projectLabel}
            </span>
          )}
        </div>
        <p className="text-[11px] text-gray-400 tabular-nums whitespace-nowrap flex-shrink-0">
          {formatRelativeTime(session.modifiedAt)}
        </p>
      </div>

      {/* Preview text */}
      <div className="min-w-0">
        <p className="text-sm font-medium text-gray-900 line-clamp-2">
          {cleanPreview}
        </p>

        {/* Last message if different from first */}
        {cleanLast && cleanLast !== cleanPreview && (
          <p className="text-[13px] text-gray-500 line-clamp-1 mt-0.5">
            <span className="text-gray-300 mr-1">→</span>{cleanLast}
          </p>
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

      {/* Footer: Tool counts + Message stats + Skills */}
      <div className="flex items-center justify-between mt-3 pt-3 border-t border-gray-100">
        <div className="flex items-center gap-3">
          {/* Tool counts */}
          {totalTools > 0 && (
            <div className="flex items-center gap-2 text-xs text-gray-400">
              {editCount > 0 && (
                <span className="flex items-center gap-0.5" title="Edits">
                  <Pencil className="w-3 h-3" />
                  {editCount}
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

          {/* Message count and turns */}
          {(session.messageCount ?? 0) > 0 && (
            <div className="flex items-center gap-1 text-xs text-gray-400">
              <MessageSquare className="w-3 h-3" />
              <span>
                {session.messageCount} msgs · {session.turnCount} turns
              </span>
            </div>
          )}
        </div>

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
    </button>
  )
}
