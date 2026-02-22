/**
 * AgentProgressCard - Displays agent execution progress safely
 *
 * SECURITY: React Auto-Escaping Prevents XSS
 * ============================================
 *
 * All text content (agentId, prompt, model, etc.) is rendered via React JSX text nodes.
 * React automatically escapes all text nodes, preventing XSS attacks.
 * No dangerouslySetInnerHTML is used.
 */

import { useState } from 'react'
import { Bot, ChevronRight, ChevronDown } from 'lucide-react'
import { cn } from '../lib/utils'
import { CompactCodeBlock } from './live/CompactCodeBlock'

interface TokenCount {
  input?: number
  output?: number
}

interface AgentProgressCardProps {
  agentId?: string
  prompt?: string
  model?: string
  tokens?: TokenCount
  normalizedMessages?: number
  indent?: number
  blockId?: string
  verboseMode?: boolean
}

const MAX_PROMPT_LENGTH = 1000

export function AgentProgressCard({
  agentId,
  prompt,
  model,
  tokens,
  normalizedMessages,
  indent = 0,
  blockId,
  verboseMode,
}: AgentProgressCardProps) {
  const [expanded, setExpanded] = useState(verboseMode ?? false)

  const totalTokens = tokens
    ? (tokens.input || 0) + (tokens.output || 0)
    : undefined

  const displayName = agentId ? `Agent #${agentId}` : 'Sub-agent'

  const statusParts: string[] = [displayName]
  if (model) statusParts.push(`(${model})`)
  if (totalTokens !== undefined) statusParts.push(`${totalTokens} tokens`)

  const truncatedPrompt =
    prompt && prompt.length > MAX_PROMPT_LENGTH
      ? prompt.slice(0, MAX_PROMPT_LENGTH) + '...'
      : prompt

  return (
    <div
      className={cn('py-0.5 border-l-2 border-l-indigo-400 pl-1 my-1')}
      style={{ marginLeft: indent ? `${indent * 16}px` : undefined }}
    >
      {/* Status line — clickable to expand */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 mb-0.5 w-full text-left"
        aria-label="Agent progress"
        aria-expanded={expanded}
      >
        <Bot className="w-3 h-3 text-indigo-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 truncate flex-1">
          {statusParts.join(' ')}
        </span>
        {expanded ? (
          <ChevronDown className="w-3 h-3 text-gray-400 flex-shrink-0" />
        ) : (
          <ChevronRight className="w-3 h-3 text-gray-400 flex-shrink-0" />
        )}
      </button>

      {/* Expanded details */}
      {expanded && (
        <div className="mt-0.5">
          {truncatedPrompt && (
            <div data-testid="agent-prompt">
              <CompactCodeBlock
                code={truncatedPrompt}
                language="text"
                blockId={blockId ? `${blockId}-prompt` : agentId ? `agent-${agentId}-prompt` : undefined}
              />
            </div>
          )}

          {(model || totalTokens !== undefined || normalizedMessages !== undefined) && (
            <div className="flex items-center gap-2 ml-4 mt-0.5">
              {model && (
                <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500">
                  model: {model}
                </span>
              )}
              {totalTokens !== undefined && (
                <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500">
                  tokens: {totalTokens}
                </span>
              )}
              {normalizedMessages !== undefined && (
                <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500">
                  msgs: {normalizedMessages}
                </span>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
