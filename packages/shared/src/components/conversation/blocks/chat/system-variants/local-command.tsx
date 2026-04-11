import type { LocalCommand } from '../../../../../types/sidecar-protocol'

interface Props {
  data: LocalCommand
}

function stripLocalCommandWrapper(content: string): string {
  return content
    .replace(/<local-command-stdout>/g, '')
    .replace(/<\/local-command-stdout>/g, '')
    .trim()
}

export function LocalCommandBlock({ data }: Props) {
  const content = stripLocalCommandWrapper(data?.content ?? '')
  return (
    <pre className="px-3 py-2 text-xs font-mono text-gray-700 dark:text-gray-300 whitespace-pre-wrap max-h-48 overflow-y-auto bg-gray-50 dark:bg-gray-800/50 rounded-lg border border-gray-200 dark:border-gray-700">
      {content}
    </pre>
  )
}
