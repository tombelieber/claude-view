import type { SessionSourceInfo } from '@claude-view/shared/types/generated/SessionSourceInfo'
import { Claude, Cursor, Windsurf } from '@lobehub/icons'
import { Code, Zap } from 'lucide-react'
import {
  SiAndroidstudio,
  SiEclipseide,
  SiIntellijidea,
  SiNeovim,
  SiSublimetext,
  SiVim,
  SiXcode,
} from 'react-icons/si'
import { VscVscode } from 'react-icons/vsc'

type IconComponent = React.ComponentType<{ size?: number; className?: string }>

/** Wrapper to adapt @lobehub/icons (size accepts string|number) to our IconComponent type. */
function lobeIcon(
  LobeComp: React.ComponentType<{ size?: string | number; className?: string }>,
): IconComponent {
  return function LobeWrapper({ size, className }: { size?: number; className?: string }) {
    return <LobeComp size={size} className={className} />
  }
}

/** Per-IDE icon + brand color. Icon color is inline style (exact hex), not Tailwind. */
const IDE_CONFIG: Record<string, { icon: IconComponent; color: string }> = {
  'VS Code': { icon: VscVscode, color: '#007ACC' },
  Cursor: { icon: lobeIcon(Cursor), color: '#00B4D8' },
  Windsurf: { icon: lobeIcon(Windsurf), color: '#00C896' },
  IntelliJ: { icon: SiIntellijidea, color: '#FC801D' },
  WebStorm: { icon: SiIntellijidea, color: '#00CDD7' },
  PyCharm: { icon: SiIntellijidea, color: '#21D789' },
  GoLand: { icon: SiIntellijidea, color: '#00ACC1' },
  RustRover: { icon: SiIntellijidea, color: '#FC801D' },
  Rider: { icon: SiIntellijidea, color: '#DD1265' },
  CLion: { icon: SiIntellijidea, color: '#21D789' },
  PhpStorm: { icon: SiIntellijidea, color: '#B345F1' },
  Xcode: { icon: SiXcode, color: '#147EFB' },
  Neovim: { icon: SiNeovim, color: '#57A143' },
  Vim: { icon: SiVim, color: '#019733' },
  'Sublime Text': { icon: SiSublimetext, color: '#FF9800' },
  Eclipse: { icon: SiEclipseide, color: '#F7941E' },
  'Android Studio': { icon: SiAndroidstudio, color: '#3DDC84' },
  Zed: { icon: Code, color: '#084CCF' },
  Emacs: { icon: Code, color: '#7F5AB6' },
  Fleet: { icon: Code, color: '#7B61FF' },
}

interface SourceBadgeProps {
  source: SessionSourceInfo | null | undefined
  size?: 'sm' | 'md'
}

export function SourceBadge({ source, size = 'sm' }: SourceBadgeProps) {
  const iconSize = size === 'sm' ? 10 : 12

  // Source not yet resolved — skeleton placeholder while reconciliation loop classifies.
  // Shows a pulsing pill so users expect the badge to appear (resolves within ~30s).
  if (!source) {
    return (
      <span
        className={`inline-block rounded bg-gray-200 dark:bg-gray-700 animate-pulse shrink-0`}
        style={{ width: iconSize + 2, height: iconSize + 2 }}
        title="Detecting source…"
      />
    )
  }

  const textClass = 'text-xs'

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
    const config = source.label ? IDE_CONFIG[source.label] : null
    const Icon = config?.icon ?? Code
    const label = source.label ?? 'IDE'
    const brandColor = config?.color ?? '#A855F7'

    return (
      <span
        className={`inline-flex items-center gap-0.5 ${textClass} font-semibold bg-gray-100 dark:bg-gray-800 rounded px-1 leading-3.5 shrink-0`}
        style={{ color: brandColor }}
        title={`Launched from ${label} extension`}
      >
        <Icon size={iconSize} className="shrink-0" />
        {label}
      </span>
    )
  }

  // Terminal — Claude sparkle logomark (the default launch context)
  return <Claude.Color size={iconSize} className="shrink-0" />
}
