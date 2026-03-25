import type { ProgressBlock } from '../types/blocks'

/** Summary of an agent's tool activity extracted from progress block messages. */
export interface AgentToolSummary {
  /** Tool name → call count */
  tools: Record<string, number>
  /** Total number of operations (tool calls + results) */
  totalOps: number
  /** Agent's initial prompt/description */
  prompt: string
  /** Agent ID */
  agentId: string
}

/** Extract tool call names from an agent_progress message payload. */
function extractToolNames(message: unknown): string[] {
  if (!message || typeof message !== 'object') return []
  const msg = message as Record<string, unknown>
  if (msg.type !== 'assistant') return []

  const inner = msg.message as Record<string, unknown> | undefined
  const content = inner?.content
  if (!Array.isArray(content)) return []

  const names: string[] = []
  for (const c of content) {
    if (c && typeof c === 'object' && (c as Record<string, unknown>).type === 'tool_use') {
      const name = (c as Record<string, unknown>).name
      if (typeof name === 'string') names.push(name)
    }
  }
  return names
}

/** Build a summary of tool activity from a group of agent progress blocks. */
export function summarizeAgentGroup(blocks: ProgressBlock[]): AgentToolSummary {
  const tools: Record<string, number> = {}
  let totalOps = 0
  let prompt = ''
  let agentId = ''

  for (const block of blocks) {
    if (block.data.type !== 'agent') continue
    if (!agentId && block.data.agentId) agentId = block.data.agentId
    if (!prompt && block.data.prompt) prompt = block.data.prompt

    const names = extractToolNames(block.data.message)
    for (const name of names) {
      tools[name] = (tools[name] ?? 0) + 1
      totalOps++
    }
    // Count tool results as ops too
    if (block.data.message && typeof block.data.message === 'object') {
      const msg = block.data.message as Record<string, unknown>
      if (msg.type === 'user') {
        const inner = msg.message as Record<string, unknown> | undefined
        const content = inner?.content
        if (Array.isArray(content)) {
          for (const c of content) {
            if (
              c &&
              typeof c === 'object' &&
              (c as Record<string, unknown>).type === 'tool_result'
            ) {
              totalOps++
            }
          }
        }
      }
    }
  }

  return { tools, totalOps, prompt, agentId }
}

/** Format tool summary as a compact string: "Read ×12, Grep ×5" */
export function formatToolSummary(tools: Record<string, number>): string {
  const entries = Object.entries(tools).sort(([, a], [, b]) => b - a)
  if (entries.length === 0) return ''
  return entries.map(([name, count]) => `${name} ×${count}`).join(', ')
}
