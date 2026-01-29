/**
 * AgentProgressCard - Displays agent execution progress safely
 *
 * SECURITY: React Auto-Escaping Prevents XSS
 * ============================================
 *
 * All text content (agentId, prompt, model, etc.) is rendered via React JSX text nodes.
 * React automatically escapes all text nodes, preventing XSS attacks:
 *
 * - Text like `<script>alert("XSS")</script>` is rendered as literal text: &lt;script&gt;...&lt;/script&gt;
 * - Event handler attributes like `onclick="..."` become plain text, not executable attributes
 * - HTML entities are escaped: & becomes &amp;, < becomes &lt;, > becomes &gt;, " becomes &quot;
 * - No dangerouslySetInnerHTML is used
 * - Event handlers cannot bind to escaped text
 *
 * This is React's default behavior for all string interpolation in JSX ({} syntax).
 * It is enforced automatically by React's render engine and cannot be bypassed unless
 * you explicitly use dangerouslySetInnerHTML (which this component does not).
 *
 * Performance:
 * - Text escaping is O(n) where n = text length, typically less than 1ms
 * - No regex or sanitization library needed (React handles it)
 *
 * @example
 * // Input: XSS attempt in props
 * <AgentProgressCard
 *   agentId="<img src=x onerror='alert(\"XSS\")'>"
 *   prompt="<script>alert('hi')</script>"
 *   model="claude-opus"
 * />
 *
 * // Output: Rendered as escaped text (safe)
 * // HTML: <span>&lt;img src=x onerror='alert("XSS")'&gt;</span>
 * // Display: <img src=x onerror='alert("XSS")'> (literal text, no HTML interpretation)
 */

import { Bot } from 'lucide-react'

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

export function AgentProgressCard({
  agentId,
  prompt,
  model,
  tokens,
  normalizedMessages,
  indent = 0,
}: AgentProgressCardProps) {
  // Calculate total tokens (input + output)
  const totalTokens = tokens
    ? (tokens.input || 0) + (tokens.output || 0)
    : undefined

  // Render component with all text escaped by React's default behavior
  return (
    <div
      className="rounded-lg border border-blue-200 bg-blue-50 p-3 my-2"
      style={{ paddingLeft: `${indent}px` }}
    >
      <div className="flex items-start gap-2">
        <Bot className="w-4 h-4 text-blue-600 mt-0.5 flex-shrink-0" aria-hidden="true" />
        <div className="flex-1 min-w-0">
          {agentId && (
            <div className="text-sm font-semibold text-blue-900">{agentId}</div>
          )}

          {prompt && (
            <div className="text-sm text-blue-800 mt-1">{prompt}</div>
          )}

          <div className="text-xs text-blue-700 mt-2 space-y-1">
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
      </div>
    </div>
  )
}
