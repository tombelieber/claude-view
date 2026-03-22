import type { SessionSourceInfo } from '@claude-view/shared/types/generated/SessionSourceInfo'
import { Code, TerminalSquare, Zap } from 'lucide-react'
import {
  SiAndroidstudio,
  SiEclipseide,
  SiIntellijidea,
  SiNeovim,
  SiSublimetext,
  SiVim,
  SiXcode,
} from 'react-icons/si'

type IconComponent = React.ComponentType<{ size?: number; className?: string }>

const IDE_ICONS: Record<string, IconComponent> = {
  'VS Code': Code,
  Cursor: Code,
  IntelliJ: SiIntellijidea,
  WebStorm: SiIntellijidea,
  PyCharm: SiIntellijidea,
  GoLand: SiIntellijidea,
  RustRover: SiIntellijidea,
  Rider: SiIntellijidea,
  CLion: SiIntellijidea,
  PhpStorm: SiIntellijidea,
  Xcode: SiXcode,
  Neovim: SiNeovim,
  Vim: SiVim,
  'Sublime Text': SiSublimetext,
  Eclipse: SiEclipseide,
  'Android Studio': SiAndroidstudio,
}

interface SourceBadgeProps {
  source: SessionSourceInfo | null | undefined
  size?: 'sm' | 'md'
}

export function SourceBadge({ source, size = 'sm' }: SourceBadgeProps) {
  if (!source) return null

  const iconSize = size === 'sm' ? 10 : 12
  const textClass = size === 'sm' ? 'text-[9px]' : 'text-[10px]'

  if (source.category === 'agent_sdk') {
    return (
      <span
        className={`inline-flex items-center gap-0.5 ${textClass} font-semibold text-blue-500 dark:text-blue-400 bg-blue-100 dark:bg-blue-900/30 rounded px-1 leading-3.5 shrink-0`}
        title="Started from this app — you can send prompts and interact directly"
      >
        <Zap size={iconSize} />
        This App
      </span>
    )
  }

  if (source.category === 'ide') {
    const IdeIcon = source.label ? IDE_ICONS[source.label] : null
    const Icon = IdeIcon ?? Code
    const label = source.label ?? 'IDE'

    return (
      <span
        className={`inline-flex items-center gap-0.5 ${textClass} font-semibold text-purple-500 dark:text-purple-400 bg-purple-100 dark:bg-purple-900/30 rounded px-1 leading-3.5 shrink-0`}
        title={`Launched from ${label} extension`}
      >
        <Icon size={iconSize} className="shrink-0" />
        {label}
      </span>
    )
  }

  // Terminal — just icon, no badge text (it's the default)
  return <TerminalSquare size={iconSize} className="text-gray-400 dark:text-gray-500 shrink-0" />
}
