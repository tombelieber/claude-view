import { Bot, ChevronDown, ChevronRight } from 'lucide-react'
import { useState } from 'react'
import { useCompactCodeBlock } from '../contexts/CodeRenderContext'

/**
 * AgentProgressCard — purpose-built for AgentProgress schema.
 *
 * Schema fields: agentId, prompt, message
 * Every field is rendered. No phantom props.
 */

interface AgentProgressCardProps {
  /** Agent identifier */
  agentId: string
  /** The task/prompt given to this sub-agent */
  prompt: string
  /** Optional structured message/response from the agent (type: any in schema) */
  message?: unknown
  /** UI-only: stable key for code block rendering */
  blockId?: string
}

const MAX_PROMPT_LENGTH = 1000

export function AgentProgressCard({ agentId, prompt, message, blockId }: AgentProgressCardProps) {
  const CompactCodeBlock = useCompactCodeBlock()
  const [expanded, setExpanded] = useState(false)

  const truncatedPrompt =
    prompt.length > MAX_PROMPT_LENGTH ? prompt.slice(0, MAX_PROMPT_LENGTH) + '\u2026' : prompt

  const hasMessage = message !== undefined && message !== null

  return (
    <div className="py-0.5 border-l-2 border-l-indigo-400 pl-1 my-1">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 mb-0.5 w-full text-left cursor-pointer"
        aria-label="Agent progress"
        aria-expanded={expanded}
      >
        <Bot className="w-3 h-3 text-indigo-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-indigo-400 dark:text-indigo-500 flex-shrink-0">
          #{agentId}
        </span>
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 truncate flex-1">
          {prompt.slice(0, 80)}
        </span>
        {hasMessage && (
          <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-400 dark:text-indigo-300 flex-shrink-0">
            +msg
          </span>
        )}
        {expanded ? (
          <ChevronDown className="w-3 h-3 text-gray-400 flex-shrink-0" />
        ) : (
          <ChevronRight className="w-3 h-3 text-gray-400 flex-shrink-0" />
        )}
      </button>

      {expanded && (
        <div className="mt-0.5 space-y-0.5">
          {truncatedPrompt && (
            <div data-testid="agent-prompt">
              <CompactCodeBlock
                code={truncatedPrompt}
                language="text"
                blockId={blockId ? `${blockId}-prompt` : `agent-${agentId}-prompt`}
              />
            </div>
          )}

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
      )}
    </div>
  )
}
