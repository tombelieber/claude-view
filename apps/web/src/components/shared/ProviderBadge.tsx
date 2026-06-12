import { Copilot, Cursor, Gemini, Kimi, OpenAI, Qwen } from '@lobehub/icons'
import { Bot, Code, Terminal } from 'lucide-react'
import { SiGithubcopilot, SiSnowflake } from 'react-icons/si'
import { VscVscode } from 'react-icons/vsc'
import { providerColor, providerLabel } from '../../lib/providers'

type IconComponent = React.ComponentType<{ size?: number; className?: string }>

/** Adapt @lobehub/icons (size accepts string|number) to IconComponent. */
function lobeIcon(
  LobeComp: React.ComponentType<{ size?: string | number; className?: string }>,
): IconComponent {
  return function LobeWrapper({ size, className }: { size?: number; className?: string }) {
    return <LobeComp size={size} className={className} />
  }
}

/** Brand icons where the installed icon sets have them; Bot otherwise. */
const PROVIDER_ICONS: Record<string, IconComponent> = {
  codex: lobeIcon(OpenAI),
  gemini: lobeIcon(Gemini),
  copilot: SiGithubcopilot,
  cursor: lobeIcon(Cursor),
  qwen: lobeIcon(Qwen),
  kimi: lobeIcon(Kimi),
  cortex: SiSnowflake,
  'vscode-copilot': VscVscode,
  positron: lobeIcon(Copilot),
  zed: Code,
  opencode: Terminal,
}

interface ProviderBadgeProps {
  /** Provider kebab id from SessionInfo.provider (e.g. "codex"). */
  provider: string
  size?: 'sm' | 'md'
}

/**
 * Provider chip for foreign-agent sessions (Codex, Cursor, OpenCode, …).
 * Claude Code sessions keep their existing SourceBadge — this badge only
 * renders for sessions ingested from another agent's on-disk store.
 */
export function ProviderBadge({ provider, size = 'sm' }: ProviderBadgeProps) {
  const iconSize = size === 'sm' ? 10 : 12
  const Icon = PROVIDER_ICONS[provider] ?? Bot
  const label = providerLabel(provider)
  const color = providerColor(provider)

  return (
    <span
      className="inline-flex items-center gap-1 text-xs font-semibold bg-gray-100 dark:bg-gray-800 rounded px-1.5 py-0.5 shrink-0"
      style={{ color }}
      title={`Session from ${label}`}
    >
      <Icon size={iconSize} className="shrink-0" />
      {label}
    </span>
  )
}
