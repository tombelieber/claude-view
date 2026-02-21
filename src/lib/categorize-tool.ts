import type { ActionCategory } from '../components/live/action-log/types'

/** Categorize a tool call by its name into an ActionCategory. */
export function categorizeTool(toolName: string): ActionCategory {
  if (toolName === 'Skill') return 'skill'
  if (toolName.startsWith('mcp__') || toolName.startsWith('mcp_')) return 'mcp'
  if (toolName === 'Task') return 'agent'
  return 'builtin'
}
