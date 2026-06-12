// Provider display config for foreign-agent sessions.
//
// Mirrors ProviderKind in crates/providers/src/kind.rs — ids are the kebab
// strings the backend puts in SessionInfo.provider and the /api/providers
// summary. Unknown ids degrade to a generic label (never hidden).

export const CLAUDE_PROVIDER_ID = 'claude-code'

export interface ProviderConfig {
  label: string
  /** Brand accent (inline style hex — exact brand colors, not Tailwind). */
  color: string
}

export const PROVIDER_CONFIG: Record<string, ProviderConfig> = {
  [CLAUDE_PROVIDER_ID]: { label: 'Claude Code', color: '#D97757' },
  codex: { label: 'Codex', color: '#74AA9C' },
  gemini: { label: 'Gemini CLI', color: '#4285F4' },
  copilot: { label: 'Copilot CLI', color: '#8957E5' },
  cursor: { label: 'Cursor', color: '#00B4D8' },
  opencode: { label: 'OpenCode', color: '#F97316' },
  hermes: { label: 'Hermes', color: '#8B5CF6' },
  amp: { label: 'Amp', color: '#FF5C00' },
  qwen: { label: 'Qwen Code', color: '#615CED' },
  iflow: { label: 'iFlow', color: '#0EA5E9' },
  openhands: { label: 'OpenHands', color: '#FFB703' },
  zencoder: { label: 'Zencoder', color: '#10B981' },
  pi: { label: 'Pi', color: '#EC4899' },
  openclaw: { label: 'OpenClaw', color: '#EF4444' },
  qclaw: { label: 'QClaw', color: '#F59E0B' },
  kimi: { label: 'Kimi', color: '#6366F1' },
  commandcode: { label: 'Command Code', color: '#14B8A6' },
  cortex: { label: 'Cortex Code', color: '#29B5E8' },
  workbuddy: { label: 'WorkBuddy', color: '#A855F7' },
  zed: { label: 'Zed', color: '#084CCF' },
  forge: { label: 'Forge', color: '#E11D48' },
  piebald: { label: 'Piebald', color: '#78716C' },
  kiro: { label: 'Kiro CLI', color: '#7C3AED' },
  'kiro-ide': { label: 'Kiro IDE', color: '#9333EA' },
  'vscode-copilot': { label: 'VS Code Copilot', color: '#007ACC' },
  positron: { label: 'Positron', color: '#447099' },
}

export function providerLabel(id: string): string {
  return PROVIDER_CONFIG[id]?.label ?? id
}

export function providerColor(id: string): string {
  return PROVIDER_CONFIG[id]?.color ?? '#6B7280'
}

/** True when a session id is namespaced to a foreign provider ("codex:…"). */
export function isForeignSessionId(sessionId: string): boolean {
  const prefix = sessionId.split(':', 1)[0]
  return prefix !== sessionId && prefix !== CLAUDE_PROVIDER_ID && prefix in PROVIDER_CONFIG
}
