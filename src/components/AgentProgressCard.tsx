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
}

const MAX_PROMPT_LENGTH = 1000

export function AgentProgressCard({
  agentId,
  prompt,
  model,
  tokens,
  normalizedMessages,
  indent = 0,
}: AgentProgressCardProps) {
  const [expanded, setExpanded] = useState(false)

  const totalTokens = tokens
    ? (tokens.input || 0) + (tokens.output || 0)
    : undefined

  const displayName = agentId ? `Agent #${agentId}` : 'Sub-agent'

  const tokenSuffix = totalTokens !== undefined ? ` (${totalTokens} tokens used)` : ''

  const titleParts = [displayName]
  if (model) titleParts.push(`(${model})`)

  const title = titleParts.join(' ')

  const truncatedPrompt =
    prompt && prompt.length > MAX_PROMPT_LENGTH
      ? prompt.slice(0, MAX_PROMPT_LENGTH) + '...'
      : prompt

  return (
    <div
      className={cn(
        'rounded-lg border border-indigo-200 dark:border-indigo-800 border-l-4 border-l-indigo-400 bg-indigo-50 dark:bg-indigo-950/30 my-2 overflow-hidden'
      )}
      style={{ marginLeft: indent ? `${indent * 16}px` : undefined }}
    >
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-indigo-100 dark:hover:bg-indigo-900/30 transition-colors"
        aria-label="Agent progress"
        aria-expanded={expanded}
      >
        <Bot className="w-4 h-4 text-indigo-600 flex-shrink-0" aria-hidden="true" />
        <span className="text-sm font-semibold text-indigo-900 dark:text-indigo-200 truncate flex-1">
          {title}
          {tokenSuffix && (
            <span className="font-normal text-indigo-700 dark:text-indigo-400">{tokenSuffix}</span>
          )}
        </span>
        {expanded ? (
          <ChevronDown className="w-4 h-4 text-indigo-400" />
        ) : (
          <ChevronRight className="w-4 h-4 text-indigo-400" />
        )}
      </button>

      {expanded && (
        <div className="px-3 py-2 border-t border-indigo-100 dark:border-indigo-800 bg-indigo-50/50 dark:bg-indigo-950/20">
          {truncatedPrompt && (
            <div
              className="text-sm text-indigo-800 dark:text-indigo-300 mb-2"
              data-testid="agent-prompt"
            >
              {truncatedPrompt}
            </div>
          )}

          <div className="text-xs text-indigo-700 dark:text-indigo-400 space-y-1">
            {model && (
              <div>
                <span className="font-medium">Model:</span> {model}
              </div>
            )}

            {totalTokens !== undefined && (
              <div>
                <span className="font-medium">Tokens:</span> {totalTokens}
              </div>
            )}

            {normalizedMessages !== undefined && (
              <div>
                <span className="font-medium">Messages:</span> {normalizedMessages}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
