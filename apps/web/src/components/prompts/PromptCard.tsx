import { ArrowRight, Copy, Paperclip } from 'lucide-react'
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import type { PromptInfo } from '../../types/generated/PromptInfo'

const intentColors: Record<string, string> = {
  fix: 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400',
  create: 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400',
  review: 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400',
  explain: 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400',
  ship: 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400',
  refactor: 'bg-cyan-100 text-cyan-700 dark:bg-cyan-900/30 dark:text-cyan-400',
  confirm: 'bg-gray-100 text-gray-700 dark:bg-gray-700/30 dark:text-gray-400',
  command: 'bg-orange-100 text-orange-700 dark:bg-orange-900/30 dark:text-orange-400',
  other: 'bg-gray-100 text-gray-600 dark:bg-gray-800/30 dark:text-gray-500',
}

function formatRelativeTime(tsMs: number): { relative: string; absolute: string } {
  if (tsMs <= 0) return { relative: '--', absolute: '--' }
  const date = new Date(tsMs)
  const now = Date.now()
  const diffSec = Math.floor((now - tsMs) / 1000)

  let relative: string
  if (diffSec < 60) relative = 'just now'
  else if (diffSec < 3600) relative = `${Math.floor(diffSec / 60)}m ago`
  else if (diffSec < 86400) relative = `${Math.floor(diffSec / 3600)}h ago`
  else if (diffSec < 604800) relative = `${Math.floor(diffSec / 86400)}d ago`
  else relative = date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })

  const absolute = date.toLocaleString()
  return { relative, absolute }
}

interface PromptCardProps {
  prompt: PromptInfo
}

export function PromptCard({ prompt }: PromptCardProps) {
  const [expanded, setExpanded] = useState(false)

  const ts = formatRelativeTime(Number(prompt.timestamp))
  const intentClass = intentColors[prompt.intent] ?? intentColors.other

  function handleCopy(e: React.MouseEvent) {
    e.stopPropagation()
    navigator.clipboard.writeText(prompt.display).then(() => {
      toast.success('Copied!')
    })
  }

  return (
    <div className="p-3 rounded-lg border border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600 transition-colors">
      {/* Header row: project badge + intent + timestamp */}
      <div className="flex items-center justify-between text-xs">
        <div className="flex items-center gap-1.5 min-w-0">
          <span className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 font-medium truncate max-w-[140px]">
            {prompt.projectDisplayName}
          </span>
          <span className={`px-1.5 py-0.5 rounded font-medium ${intentClass}`}>
            {prompt.intent}
          </span>
        </div>
        <span className="text-gray-400 dark:text-gray-500 shrink-0" title={ts.absolute}>
          {ts.relative}
        </span>
      </div>

      {/* Prompt text — click to expand/collapse */}
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="mt-2 text-left text-sm text-gray-800 dark:text-gray-200 w-full cursor-pointer"
      >
        <p className={expanded ? 'whitespace-pre-wrap' : 'line-clamp-3'}>{prompt.display}</p>
      </button>

      {/* Secondary info row: branch, model, paste indicator */}
      <div className="mt-2 flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
        {prompt.branch && <span>{prompt.branch}</span>}
        {prompt.branch && prompt.model && <span>&middot;</span>}
        {prompt.model && <span>{prompt.model}</span>}
        {prompt.hasPaste && (
          <span
            className="inline-flex items-center gap-0.5"
            title={prompt.pastePreview ?? undefined}
          >
            <Paperclip className="w-3 h-3" />
            paste attached
          </span>
        )}
      </div>

      {/* Action buttons */}
      <div className="mt-2 flex items-center gap-1">
        <button
          type="button"
          onClick={handleCopy}
          className="p-1 rounded text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
          title="Copy prompt"
        >
          <Copy className="w-3.5 h-3.5" />
        </button>
        {prompt.sessionId && (
          <Link
            to={`/sessions/${prompt.sessionId}`}
            onClick={(e) => e.stopPropagation()}
            className="p-1 rounded text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            title="Go to session"
          >
            <ArrowRight className="w-3.5 h-3.5" />
          </Link>
        )}
      </div>
    </div>
  )
}
