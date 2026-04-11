import type { CommandOutput } from '../../../../../types/sidecar-protocol'

interface Props {
  data: CommandOutput
}

export function CommandOutputBlock({ data }: Props) {
  return (
    <pre className="px-3 py-2 text-xs font-mono text-gray-700 dark:text-gray-300 whitespace-pre-wrap max-h-48 overflow-y-auto bg-gray-50 dark:bg-gray-800/50 rounded-lg border border-gray-200 dark:border-gray-700">
      {data?.content}
    </pre>
  )
}
