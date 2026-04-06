import { useQuery } from '@tanstack/react-query'

// ── Types matching Rust ClaudeCodeSettings ──

export interface EnvVar {
  key: string
  value: string
  redacted: boolean
}

export interface PermissionRules {
  allow: string[]
  deny: string[]
  ask: string[]
}

export interface UserHook {
  event: string
  matcher: string | null
  command: string
  isAsync: boolean
}

export interface PluginEntry {
  id: string
  marketplace: string
  enabled: boolean
}

export interface CustomMarketplace {
  name: string
  sourceType: string
  sourceValue: string
}

export interface ClaudeCodeSettings {
  env: EnvVar[]
  permissions: PermissionRules
  userHooks: UserHook[]
  systemHookCount: number
  systemHooks: UserHook[]
  plugins: PluginEntry[]
  customMarketplaces: CustomMarketplace[]
  voiceEnabled: boolean
  skipDangerousPrompt: boolean
  statusLine: string | null
  defaultMode: string | null
}

async function fetchClaudeCodeSettings(): Promise<ClaudeCodeSettings> {
  const response = await fetch('/api/claude-code-settings')
  if (!response.ok) {
    throw new Error(`Failed to fetch settings: ${await response.text()}`)
  }
  return response.json()
}

/**
 * Fetch Claude Code settings (merged global + local, redacted).
 * Stale time: 60s — settings rarely change.
 */
export function useClaudeCodeSettings() {
  return useQuery({
    queryKey: ['claude-code-settings'],
    queryFn: fetchClaudeCodeSettings,
    staleTime: 60_000,
  })
}
