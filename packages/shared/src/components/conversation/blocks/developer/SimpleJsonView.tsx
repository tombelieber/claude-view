import type { JsonTreeProps } from '../../../../contexts/DeveloperToolsContext'

/**
 * Minimal JSON viewer used when no rich JsonTree is injected via DeveloperToolsContext.
 * Renders formatted JSON in a <pre> block with copy-on-click.
 */
export function SimpleJsonView({ data }: JsonTreeProps) {
  const json = JSON.stringify(data, null, 2)
  return (
    <pre className="text-xs font-mono leading-relaxed text-gray-700 dark:text-gray-400 overflow-auto max-h-96 whitespace-pre-wrap break-all">
      {json}
    </pre>
  )
}
