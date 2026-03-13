import type { LucideIcon } from 'lucide-react'
import { Cpu, FileText, Plug, RefreshCw, Server, Shield, Trash2, Zap } from 'lucide-react'
import type { ModelOption } from '../../hooks/use-models'
import type { SessionCapabilities } from '../../hooks/use-session-capabilities'
import type { PermissionMode } from '../../types/control'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type SubItem = { id: string; label: string; active: boolean }

export type PaletteItem =
  | {
      type: 'action'
      label: string
      icon: LucideIcon
      onSelect: () => void
      disabled?: boolean
      hint?: string
    }
  | {
      type: 'toggle'
      label: string
      icon: LucideIcon
      value: boolean
      onToggle: (v: boolean) => void
      disabled?: boolean
      hint?: string
    }
  | {
      type: 'submenu'
      label: string
      icon: LucideIcon
      current: string
      items: SubItem[]
      onSelect: (id: string) => void
      warning?: string
    }
  | { type: 'link'; label: string; icon: LucideIcon; href: string; badge?: string }
  | { type: 'command'; name: string; description: string; onSelect: () => void }

export type PaletteSection = { label: string; items: PaletteItem[] }

export interface PaletteCallbacks {
  onModelSwitch: (model: string) => void
  onPaletteModeChange: (mode: PermissionMode) => void
  onCommand: (command: string) => void
  onAgent: (agent: string) => void
  onClear: () => void
  onCompact: () => void
}

// ---------------------------------------------------------------------------
// Known command descriptions (bridge until SDK provides descriptions)
// ---------------------------------------------------------------------------

export const KNOWN_COMMAND_DESCRIPTIONS: Record<string, string> = {
  commit: 'Create a git commit with staged changes',
  clear: 'Clear conversation history and start fresh',
  compact: 'Compact conversation to save context window',
  test: 'Run the project test suite',
  review: 'Review recent code changes',
  debug: 'Debug the last error or failure',
  help: 'Show available slash commands',
  cost: 'Show token usage and cost',
  status: 'Show session status and agent state',
  deploy: 'Deploy the current build',
}

// ---------------------------------------------------------------------------
// Permission mode metadata
// ---------------------------------------------------------------------------

const PERMISSION_MODES: { id: PermissionMode; label: string }[] = [
  { id: 'default', label: 'default' },
  { id: 'acceptEdits', label: 'acceptEdits' },
  { id: 'plan', label: 'plan' },
  { id: 'dontAsk', label: 'dontAsk' },
  { id: 'bypassPermissions', label: 'bypassPermissions' },
]

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

function formatModelLabel(modelOptions: ModelOption[], modelId: string): string {
  const found = modelOptions.find((m) => m.id === modelId)
  return found?.label ?? modelId
}

export function buildPaletteSections(
  capabilities: SessionCapabilities,
  modelOptions: ModelOption[],
  callbacks: PaletteCallbacks,
  flags: { isLive: boolean; isStreaming: boolean },
): PaletteSection[] {
  const resumeDisabled = !flags.isLive || flags.isStreaming
  const sections: PaletteSection[] = []

  // --- Context ---
  sections.push({
    label: 'Context',
    items: [
      {
        type: 'action',
        label: 'Attach file...',
        icon: FileText,
        onSelect: () => {},
        disabled: true,
        hint: 'coming soon',
      },
      {
        type: 'action',
        label: 'Clear conversation',
        icon: Trash2,
        onSelect: callbacks.onClear,
      },
      {
        type: 'action',
        label: 'Compact context',
        icon: RefreshCw,
        onSelect: callbacks.onCompact,
      },
    ],
  })

  // --- Model ---
  const currentModelLabel = formatModelLabel(modelOptions, capabilities.model)
  sections.push({
    label: 'Model',
    items: [
      {
        type: 'submenu',
        label: 'Switch model...',
        icon: Cpu,
        current: currentModelLabel,
        items: modelOptions.map((m) => ({
          id: m.id,
          label: m.label,
          active: m.id === capabilities.model,
        })),
        onSelect: (id: string) => {
          if (!resumeDisabled && id !== capabilities.model) {
            callbacks.onModelSwitch(id)
          }
        },
        warning: 'Switching model resumes session. Context re-ingested.',
      },
      {
        type: 'link',
        label: 'Account & usage',
        icon: Zap,
        href: '/settings#usage',
      },
    ],
  })

  // --- Customize ---
  sections.push({
    label: 'Customize',
    items: [
      {
        type: 'submenu',
        label: 'Permissions',
        icon: Shield,
        current: capabilities.permissionMode,
        items: PERMISSION_MODES.map((m) => ({
          id: m.id,
          label: m.label,
          active: m.id === capabilities.permissionMode,
        })),
        onSelect: (id: string) => {
          if (!resumeDisabled && id !== capabilities.permissionMode) {
            callbacks.onPaletteModeChange(id as PermissionMode)
          }
        },
        warning: 'Changing permissions resumes session. Context re-ingested.',
      },
      {
        type: 'link',
        label: 'Manage plugins',
        icon: Plug,
        href: '/plugins',
      },
    ],
  })

  // --- MCP Servers (individual rows with status) ---
  if (capabilities.mcpServers.length > 0) {
    sections.push({
      label: 'MCP Servers',
      items: capabilities.mcpServers.map((srv) => ({
        type: 'link' as const,
        label: srv.name,
        icon: Server,
        href: '/settings#mcp',
        badge: srv.status,
      })),
    })
  }

  // --- Slash Commands (only if sidecar provided any) ---
  const slashSet = new Set(capabilities.slashCommands)
  if (capabilities.slashCommands.length > 0) {
    sections.push({
      label: 'Commands',
      items: capabilities.slashCommands.map((name) => ({
        type: 'command' as const,
        name,
        description: KNOWN_COMMAND_DESCRIPTIONS[name] ?? '',
        onSelect: () => callbacks.onCommand(name),
      })),
    })
  }

  // --- Skills (deduped: only skills NOT already in slashCommands) ---
  const uniqueSkills = capabilities.skills.filter((s) => !slashSet.has(s))
  if (uniqueSkills.length > 0) {
    sections.push({
      label: 'Skills',
      items: uniqueSkills.map((name) => ({
        type: 'command' as const,
        name,
        description: KNOWN_COMMAND_DESCRIPTIONS[name] ?? '',
        onSelect: () => callbacks.onCommand(name),
      })),
    })
  }

  // --- Agents (invoked via @agent-name) ---
  if (capabilities.agents.length > 0) {
    sections.push({
      label: 'Agents',
      items: capabilities.agents.map((name) => ({
        type: 'command' as const,
        name: `@${name}`,
        description: '',
        onSelect: () => callbacks.onAgent(name),
      })),
    })
  }

  return sections
}
