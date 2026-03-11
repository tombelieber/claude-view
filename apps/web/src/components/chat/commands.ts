export interface SlashCommand {
  name: string
  description: string
  category: 'mode' | 'session' | 'action' | 'info'
}

export const COMMANDS: SlashCommand[] = [
  // Mode commands
  {
    name: 'default',
    description: 'Default mode — prompts for dangerous operations',
    category: 'mode',
  },
  { name: 'acceptEdits', description: 'Auto-approve file edits', category: 'mode' },
  { name: 'plan', description: 'Plan mode — think and plan, no tool execution', category: 'mode' },
  { name: 'dontAsk', description: 'Skip tools that need permission', category: 'mode' },
  {
    name: 'bypassPermissions',
    description: 'Trust all — auto-approve everything (dangerous)',
    category: 'mode',
  },

  // Session commands
  { name: 'clear', description: 'Clear conversation history and start fresh', category: 'session' },
  {
    name: 'compact',
    description: 'Compact conversation to save context window',
    category: 'session',
  },
  { name: 'context', description: 'Show current context window usage', category: 'session' },

  // Action commands
  { name: 'review', description: 'Review recent code changes', category: 'action' },
  { name: 'commit', description: 'Create a git commit with staged changes', category: 'action' },
  { name: 'test', description: 'Run the project test suite', category: 'action' },
  { name: 'debug', description: 'Debug the last error or failure', category: 'action' },
  { name: 'deploy', description: 'Deploy the current build', category: 'action' },

  // Info commands
  { name: 'help', description: 'Show available slash commands', category: 'info' },
  { name: 'cost', description: 'Show token usage and cost', category: 'info' },
  { name: 'status', description: 'Show session status and agent state', category: 'info' },
]

/**
 * Filters commands by matching the query against name or description (case-insensitive).
 * Returns all commands if query is empty.
 */
export function filterCommands(query: string): SlashCommand[] {
  if (!query) return COMMANDS
  const q = query.toLowerCase()
  return COMMANDS.filter(
    (cmd) => cmd.name.toLowerCase().includes(q) || cmd.description.toLowerCase().includes(q),
  )
}
