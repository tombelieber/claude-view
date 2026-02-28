import { GitBranch } from 'lucide-react'
import { useCompactCodeBlock } from '../contexts/CodeRenderContext'

interface HookProgressCardProps {
  hookEvent: string
  hookName: string
  command: string
  output?: string
  blockId?: string
  verboseMode?: boolean
}

export function HookProgressCard({
  hookEvent,
  hookName,
  command,
  output,
  blockId,
}: HookProgressCardProps) {
  const CompactCodeBlock = useCompactCodeBlock()
  const hasOutput = output !== undefined

  return (
    <div className="py-0.5 border-l-2 border-l-amber-400 pl-1 my-1">
      <div className="flex items-center gap-1.5 mb-0.5">
        <GitBranch className="w-3 h-3 text-amber-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 truncate">
          {hookEvent} {'\u2192'} {command}
        </span>
      </div>

      {hasOutput && (
        <CompactCodeBlock
          code={output}
          language="bash"
          blockId={blockId ? `${blockId}-out` : `hook-${hookName}-out`}
        />
      )}
    </div>
  )
}
