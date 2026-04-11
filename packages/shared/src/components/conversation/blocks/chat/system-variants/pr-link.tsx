import { ExternalLink } from 'lucide-react'

interface Props {
  data: Record<string, unknown>
}

export function PrLinkCard({ data }: Props) {
  const prUrl = data.prUrl as string | undefined
  const prNumber = data.prNumber as number | undefined
  const prRepo = data.prRepository as string | undefined
  return (
    <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-gray-50 dark:bg-gray-800/50 border border-gray-200 dark:border-gray-700">
      <ExternalLink className="w-3.5 h-3.5 text-gray-500 dark:text-gray-400 flex-shrink-0" />
      {prUrl ? (
        <a
          href={prUrl}
          target="_blank"
          rel="noopener noreferrer"
          className="text-xs text-blue-600 dark:text-blue-400 hover:underline truncate"
        >
          {prRepo ? `${prRepo}#${prNumber}` : `PR #${prNumber}`}
        </a>
      ) : (
        <span className="text-xs text-gray-600 dark:text-gray-400">
          {prRepo ? `${prRepo}#${prNumber}` : `PR #${prNumber}`}
        </span>
      )}
    </div>
  )
}
