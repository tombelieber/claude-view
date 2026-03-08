/** Marketplace dot color mapping from design doc Section 6.4. */
const MARKETPLACE_COLORS: Record<string, string> = {
  'claude-plugins-official': 'bg-blue-500',
  'claude-code-plugins': 'bg-indigo-400',
  'anthropic-agent-skills': 'bg-violet-400',
  'superpowers-marketplace': 'bg-amber-400',
  'supabase-agent-skills': 'bg-emerald-400',
  local: 'bg-green-400',
}

export function marketplaceDotColor(marketplace: string): string {
  return MARKETPLACE_COLORS[marketplace] ?? 'bg-gray-400'
}
