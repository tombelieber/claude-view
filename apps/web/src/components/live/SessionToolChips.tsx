import * as Tooltip from '@radix-ui/react-tooltip'
import { Plug2, Zap } from 'lucide-react'

interface ToolUsed {
  name: string
  kind: 'mcp' | 'skill'
}

interface SessionToolChipsProps {
  tools: ToolUsed[]
}

const MAX_VISIBLE = 4

const TOOLTIP_CONTENT_CLASS = 'bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2 shadow-lg z-50 max-w-xs text-xs'
const TOOLTIP_ARROW_CLASS = 'fill-gray-200 dark:fill-gray-700'

export function SessionToolChips({ tools }: SessionToolChipsProps) {
  // No hooks in this component — early return is safe
  if (tools.length === 0) return null

  // Tools are narrower than agent pills, so we show 4 before overflow
  const displayTools = tools.slice(0, MAX_VISIBLE)
  const overflowTools = tools.slice(MAX_VISIBLE)
  const hasMore = overflowTools.length > 0

  return (
    <Tooltip.Provider delayDuration={200}>
      <div className="flex flex-wrap items-center gap-1.5 px-2 py-1">
        {displayTools.map((tool) => (
          <ToolChip key={`${tool.kind}-${tool.name}`} tool={tool} />
        ))}
        {hasMore && (
          <OverflowChip tools={overflowTools} />
        )}
      </div>
    </Tooltip.Provider>
  )
}

function ToolChip({ tool }: { tool: ToolUsed }) {
  const isMcp = tool.kind === 'mcp'

  // Format display name: strip "plugin_" prefix from MCP names if present
  const displayName = isMcp ? formatMcpName(tool.name) : tool.name

  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <span
          className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded border text-xs font-medium cursor-default bg-gray-50 text-gray-600 border-gray-200 dark:bg-gray-800 dark:text-gray-400 dark:border-gray-700"
          aria-label={`${isMcp ? 'MCP' : 'Skill'}: ${tool.name}`}
        >
          {isMcp
            ? <Plug2 className="h-2.5 w-2.5 flex-shrink-0" />
            : <Zap className="h-2.5 w-2.5 flex-shrink-0" />
          }
          <span className="truncate max-w-[120px]">{displayName}</span>
        </span>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={TOOLTIP_CONTENT_CLASS} sideOffset={5}>
          <div className="space-y-1">
            <div className="font-medium text-gray-900 dark:text-gray-100">
              {tool.name}
            </div>
            <div className="text-gray-500 dark:text-gray-400">
              {isMcp ? 'MCP Server' : 'Skill'}
            </div>
          </div>
          <Tooltip.Arrow className={TOOLTIP_ARROW_CLASS} />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  )
}

function OverflowChip({ tools }: { tools: ToolUsed[] }) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <span className="inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium border border-zinc-300 dark:border-zinc-600 bg-zinc-50 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 cursor-default">
          +{tools.length} more
        </span>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={TOOLTIP_CONTENT_CLASS} sideOffset={5}>
          <div className="space-y-1.5">
            {tools.map((tool) => {
              const isMcp = tool.kind === 'mcp'
              return (
                <div key={`${tool.kind}-${tool.name}`} className="flex items-center gap-2">
                  {isMcp
                    ? <Plug2 className="h-3 w-3 flex-shrink-0 text-gray-500 dark:text-gray-400" />
                    : <Zap className="h-3 w-3 flex-shrink-0 text-gray-500 dark:text-gray-400" />
                  }
                  <span className="text-gray-900 dark:text-gray-100">
                    {tool.name}
                  </span>
                </div>
              )
            })}
          </div>
          <Tooltip.Arrow className={TOOLTIP_ARROW_CLASS} />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  )
}

function formatMcpName(name: string): string {
  // MCP server names arrive as "plugin_playwright_playwright" or "chrome-devtools"
  // Strip "plugin_" prefix and dedupe if name repeats (e.g., "plugin_playwright_playwright" -> "playwright")
  let cleaned = name
  if (cleaned.startsWith('plugin_')) {
    cleaned = cleaned.slice(7) // Remove "plugin_"
  }
  // If the remaining name has pattern "foo_foo" (duplicated), simplify to "foo"
  const parts = cleaned.split('_')
  if (parts.length === 2 && parts[0] === parts[1]) {
    return parts[0]
  }
  return cleaned
}
