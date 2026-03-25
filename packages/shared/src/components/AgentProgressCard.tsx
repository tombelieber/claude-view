import { Bot } from 'lucide-react'
import { useCompactCodeBlock } from '../contexts/CodeRenderContext'

/**
 * AgentProgressCard — all fields visible by default.
 * Follows SessionInitDetail pattern: header + content blocks.
 *
 * Schema: agentId, prompt, message
 */

interface AgentProgressCardProps {
  agentId: string
  prompt: string
  message?: unknown
  blockId?: string
}

const MAX_PROMPT_LENGTH = 2000

export function AgentProgressCard({ agentId, prompt, message, blockId }: AgentProgressCardProps) {
  const CompactCodeBlock = useCompactCodeBlock()

  const truncatedPrompt =
    prompt.length > MAX_PROMPT_LENGTH ? `${prompt.slice(0, MAX_PROMPT_LENGTH)}\u2026` : prompt

  const hasMessage = message !== undefined && message !== null

  return (
    <div className="space-y-1">
      {/* Header: icon + agent ID */}
      <div className="flex items-center gap-1.5">
        <Bot className="w-3 h-3 text-indigo-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-xs font-mono text-indigo-700 dark:text-indigo-300">#{agentId}</span>
      </div>

      {/* Prompt — always visible */}
      {truncatedPrompt && (
        <div data-testid="agent-prompt">
          <CompactCodeBlock
            code={truncatedPrompt}
            language="text"
            blockId={blockId ? `${blockId}-prompt` : `agent-${agentId}-prompt`}
          />
        </div>
      )}

      {/* Message — always visible when present */}
      {hasMessage && (
        <div data-testid="agent-message">
          <CompactCodeBlock
            code={typeof message === 'string' ? message : JSON.stringify(message, null, 2)}
            language="json"
            blockId={blockId ? `${blockId}-msg` : `agent-${agentId}-msg`}
          />
        </div>
      )}
    </div>
  )
}
