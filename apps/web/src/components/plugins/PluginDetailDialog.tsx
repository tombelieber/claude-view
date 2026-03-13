import * as Dialog from '@radix-ui/react-dialog'
import { Bot, ExternalLink, FileText, Terminal, Wrench, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { cn } from '../../lib/utils'
import type { AvailablePlugin, PluginInfo, PluginItem } from '../../types/generated'
import { AppleToggle } from './AppleToggle'
import { formatInstallCount, formatRelativeTime } from './format-helpers'
import { marketplaceDotColor } from './marketplace-colors'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function isInstalled(p: PluginInfo | AvailablePlugin): p is PluginInfo {
  return 'scope' in p
}

type ItemKind = 'skill' | 'command' | 'agent' | 'mcp_tool'

function itemFileName(item: PluginItem): string {
  const kind = item.kind as ItemKind
  if (kind === 'mcp_tool') return item.name
  return `${item.name.replace(/\s+/g, '-').toLowerCase()}.md`
}

function ItemIcon({ kind }: { kind: string }) {
  if (kind === 'command') return <Terminal className="w-3.5 h-3.5 flex-shrink-0" />
  if (kind === 'agent') return <Bot className="w-3.5 h-3.5 flex-shrink-0" />
  if (kind === 'mcp_tool') return <Wrench className="w-3.5 h-3.5 flex-shrink-0" />
  return <FileText className="w-3.5 h-3.5 flex-shrink-0" />
}

function kindDotColor(kind: string): string {
  if (kind === 'skill') return 'bg-indigo-400'
  if (kind === 'command') return 'bg-emerald-400'
  if (kind === 'agent') return 'bg-blue-400'
  return 'bg-amber-400'
}

// ---------------------------------------------------------------------------
// Scope badge
// ---------------------------------------------------------------------------

function ScopeBadge({ scope }: { scope: string }) {
  return (
    <span
      className={cn(
        'text-[10px] px-1.5 py-0.5 rounded font-bold uppercase tracking-[0.05em]',
        scope.toLowerCase() === 'project'
          ? 'bg-green-50 text-green-700'
          : 'bg-blue-50 text-blue-600',
      )}
    >
      {scope}
    </span>
  )
}

// ---------------------------------------------------------------------------
// Dialog header (shared)
// ---------------------------------------------------------------------------

interface HeaderProps {
  plugin: PluginInfo | AvailablePlugin
  installed: boolean
  marketplace: string
  version: string | null | undefined
  scope: string | null
  githubUrl?: string | null
  onAction?: (action: string, name: string, scope?: string, projectPath?: string | null) => void
  isPending: boolean
}

function DialogHeader({
  plugin,
  installed,
  marketplace,
  version,
  scope,
  githubUrl,
  onAction,
  isPending,
}: HeaderProps) {
  return (
    <div className="flex items-start justify-between gap-3 px-5 pt-5 pb-3 border-b border-apple-sep2 flex-shrink-0">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 flex-wrap">
          <Dialog.Title className="text-[15px] font-bold text-apple-text1">
            {plugin.name}
          </Dialog.Title>
          {scope && <ScopeBadge scope={scope} />}
          {!installed && (plugin as AvailablePlugin).alreadyInstalled && (
            <span className="text-[10px] font-bold uppercase tracking-wide px-1.5 py-0.5 rounded-[5px] bg-[rgba(52,199,89,0.1)] text-[#248A3D] border border-[rgba(52,199,89,0.2)]">
              INSTALLED
            </span>
          )}
        </div>
        <div className="flex items-center gap-1.5 mt-1 text-[11px] text-apple-text3">
          <span className="flex items-center gap-1">
            <span className={cn('w-1.5 h-1.5 rounded-full', marketplaceDotColor(marketplace))} />
            {marketplace}
          </span>
          <span className="text-apple-sep">·</span>
          <span className="font-mono">{version ?? '—'}</span>
          <span className="text-apple-sep">·</span>
          <span>{formatInstallCount(plugin.installCount)} installs</span>
        </div>
      </div>

      <div className="flex items-center gap-1.5 flex-shrink-0 pt-0.5">
        {githubUrl && (
          <a
            href={githubUrl}
            target="_blank"
            rel="noopener noreferrer"
            title="View on GitHub"
            onClick={(e) => e.stopPropagation()}
            className="p-1.5 rounded-lg hover:bg-apple-sep2 transition-colors text-apple-text3"
          >
            <ExternalLink className="w-4 h-4" />
          </a>
        )}

        {installed && (
          <div className="flex flex-col items-center gap-0.5">
            <AppleToggle
              checked={(plugin as PluginInfo).enabled}
              onChange={() => {
                const p = plugin as PluginInfo
                onAction?.(p.enabled ? 'disable' : 'enable', p.name, p.scope, p.projectPath)
              }}
              disabled={isPending}
            />
            <span className="text-[9px] text-apple-text3 leading-none select-none">
              {(plugin as PluginInfo).enabled ? 'Enabled' : 'Disabled'}
            </span>
          </div>
        )}

        <Dialog.Close asChild>
          <button
            type="button"
            onClick={(e) => e.stopPropagation()}
            className="p-1.5 rounded-lg hover:bg-apple-sep2 transition-colors text-apple-text3"
          >
            <X className="w-4 h-4" />
          </button>
        </Dialog.Close>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Left panel: installed plugin details
// ---------------------------------------------------------------------------

function itemsSummary(items: PluginItem[]): string {
  const counts: Array<[string, number]> = [
    ['skill', items.filter((i) => i.kind === 'skill').length],
    ['cmd', items.filter((i) => i.kind === 'command').length],
    ['agent', items.filter((i) => i.kind === 'agent').length],
    ['MCP tool', items.filter((i) => i.kind === 'mcp_tool').length],
  ]
  return counts
    .filter(([, n]) => n > 0)
    .map(([label, n]) => `${n} ${label}${n !== 1 && label !== 'MCP tool' ? 's' : ''}`)
    .join(' · ')
}

interface InstalledLeftPanelProps {
  plugin: PluginInfo
  onAction?: (action: string, name: string, scope?: string, projectPath?: string | null) => void
  isPending: boolean
}

function InstalledLeftPanel({ plugin, onAction, isPending }: InstalledLeftPanelProps) {
  const invocations = Number(plugin.totalInvocations)
  const sessions = Number(plugin.sessionCount)
  const summary = itemsSummary(plugin.items)

  return (
    <div className="overflow-y-auto flex-1 px-5 py-4 space-y-4">
      <Dialog.Description className="sr-only">{plugin.name} plugin details</Dialog.Description>

      {/* Description */}
      {plugin.description ? (
        <p className="text-[13px] text-apple-text2 leading-relaxed">{plugin.description}</p>
      ) : (
        <p className="text-[13px] text-apple-text3 italic">No description provided.</p>
      )}

      {/* Dates */}
      <div className="flex flex-wrap items-center gap-2 text-[11px] text-apple-text3">
        <span>Installed {plugin.installedAt.split('T')[0]}</span>
        {plugin.lastUpdated && <span>· Updated {plugin.lastUpdated.split('T')[0]}</span>}
        {plugin.gitSha && (
          <span className="font-mono text-[10px]">SHA {plugin.gitSha.slice(0, 12)}</span>
        )}
      </div>

      {/* Usage */}
      {invocations > 0 && (
        <div className="text-[12px] text-apple-text3">
          {invocations.toLocaleString()}× across {sessions} session{sessions !== 1 ? 's' : ''}
          {plugin.lastUsedAt && ` · ${formatRelativeTime(Number(plugin.lastUsedAt))}`}
        </div>
      )}

      {/* Items summary */}
      {summary && <div className="text-[11px] text-apple-text3">{summary}</div>}

      {/* Conflict warning */}
      {plugin.duplicateMarketplaces.length > 0 && (
        <div className="p-2.5 rounded-lg bg-[rgba(255,149,0,0.07)] border border-[rgba(255,149,0,0.2)]">
          <span className="text-[12px] text-apple-text2">
            <strong className="text-[#B45309] font-semibold">Conflict:</strong> also in{' '}
            {plugin.duplicateMarketplaces.join(', ')}. Updates won't apply.
          </span>
        </div>
      )}

      {/* Orphan error */}
      {plugin.errors.length > 0 && (
        <div className="p-2.5 rounded-lg bg-[rgba(255,59,48,0.05)] border border-[rgba(255,59,48,0.18)]">
          <div className="text-[12px] font-bold text-[#C0392B] mb-1">Orphaned install</div>
          <div className="text-[12px] text-apple-text2 mb-2">
            Source path missing. Can't update or verify.
          </div>
          <div className="flex gap-1.5">
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation()
                onAction?.('update', plugin.name, plugin.scope)
              }}
              disabled={isPending}
              className="text-[11px] font-medium px-3 py-1 rounded-[7px] border border-[rgba(255,59,48,0.3)] bg-transparent text-[#C0392B] hover:bg-[rgba(255,59,48,0.07)] disabled:opacity-50 disabled:cursor-wait transition-colors"
            >
              Reinstall
            </button>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation()
                onAction?.('uninstall', plugin.name, plugin.scope, plugin.projectPath)
              }}
              disabled={isPending}
              className="text-[11px] font-medium px-3 py-1 rounded-[7px] bg-[rgba(255,59,48,0.1)] text-[#C0392B] hover:bg-[rgba(255,59,48,0.18)] disabled:opacity-50 disabled:cursor-wait transition-colors"
            >
              Remove
            </button>
          </div>
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Right panel: IDE file viewer
// ---------------------------------------------------------------------------

interface FileViewerPanelProps {
  plugin: PluginInfo
  open: boolean
}

function FileViewerPanel({ plugin, open }: FileViewerPanelProps) {
  const [selectedIdx, setSelectedIdx] = useState(0)
  const items = plugin.items

  useEffect(() => {
    setSelectedIdx(0)
  }, [plugin.id])

  useEffect(() => {
    if (!open || items.length === 0) return
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setSelectedIdx((i) => Math.min(i + 1, items.length - 1))
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault()
        setSelectedIdx((i) => Math.max(i - 1, 0))
      }
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [open, items])

  if (items.length === 0) return null

  const selected = items[selectedIdx]
  const desc = selected?.description && selected.description !== '---' ? selected.description : null
  const invocations = selected ? Number(selected.invocationCount) : 0
  const lastUsed = selected?.lastUsedAt ? formatRelativeTime(Number(selected.lastUsedAt)) : null

  return (
    <div className="w-[45%] flex-shrink-0 border-l border-apple-sep2 flex flex-col overflow-hidden bg-[#fafafa]">
      {/* Panel header */}
      <div className="px-3 py-2 border-b border-apple-sep2 flex-shrink-0">
        <span className="text-[10px] font-bold uppercase tracking-wide text-apple-text3">
          Contents ({items.length})
        </span>
      </div>

      {/* File list */}
      <div className="overflow-y-auto" style={{ maxHeight: '60%' }}>
        {items.map((item, idx) => (
          <button
            key={item.id}
            type="button"
            onClick={(e) => {
              e.stopPropagation()
              setSelectedIdx(idx)
            }}
            className={cn(
              'w-full flex items-center gap-2 py-1.5 px-3 text-left transition-colors',
              idx === selectedIdx
                ? 'bg-[rgba(0,122,255,0.1)] text-apple-blue'
                : 'hover:bg-apple-sep2 text-apple-text2',
            )}
          >
            <span
              className={cn('w-1.5 h-1.5 rounded-full flex-shrink-0', kindDotColor(item.kind))}
            />
            <ItemIcon kind={item.kind} />
            <span className="text-[12px] font-mono truncate">{itemFileName(item)}</span>
          </button>
        ))}
      </div>

      {/* Selected file content */}
      {selected && (
        <div className="flex-1 overflow-y-auto bg-[#f8f9fa] border-t border-apple-sep2 p-3 flex flex-col gap-2 min-h-0">
          {/* Breadcrumb + meta row */}
          <div className="flex items-center gap-1.5 flex-wrap">
            <span className="text-[11px] font-mono text-apple-text2">{itemFileName(selected)}</span>
            <span
              className={cn(
                'text-[9px] font-bold uppercase tracking-wide px-1.5 py-0.5 rounded',
                selected.kind === 'skill' && 'bg-indigo-50 text-indigo-600',
                selected.kind === 'command' && 'bg-emerald-50 text-emerald-600',
                selected.kind === 'agent' && 'bg-blue-50 text-blue-600',
                selected.kind === 'mcp_tool' && 'bg-amber-50 text-amber-600',
              )}
            >
              {selected.kind.replace('_', ' ')}
            </span>
            {invocations > 0 && (
              <span className="text-[10px] text-apple-text3 tabular-nums">
                {invocations}×{lastUsed && ` · ${lastUsed}`}
              </span>
            )}
          </div>

          {/* Description */}
          {desc ? (
            <div className="text-[12px] text-apple-text2 leading-relaxed whitespace-pre-wrap font-mono bg-white border border-apple-sep2 rounded p-2 flex-1 overflow-y-auto">
              {desc}
            </div>
          ) : (
            <p className="text-[12px] text-apple-text3 italic">No description provided.</p>
          )}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Available plugin body (single panel)
// ---------------------------------------------------------------------------

function AvailableBody({
  plugin,
  onInstall,
  isPending,
}: {
  plugin: AvailablePlugin
  onInstall?: (name: string, scope: string) => void
  isPending: boolean
}) {
  return (
    <div className="overflow-y-auto flex-1 px-5 py-4 space-y-4">
      <Dialog.Description className="sr-only">{plugin.name} plugin details</Dialog.Description>
      {plugin.description ? (
        <p className="text-[13px] text-apple-text2 leading-relaxed">{plugin.description}</p>
      ) : (
        <p className="text-[13px] text-apple-text3 italic">No description provided.</p>
      )}
      {!plugin.alreadyInstalled && (
        <button
          type="button"
          disabled={isPending}
          onClick={(e) => {
            e.stopPropagation()
            onInstall?.(plugin.name, 'user')
          }}
          className="inline-flex items-center gap-1.5 text-sm font-semibold px-4 py-2 rounded-[10px] bg-apple-blue text-white hover:opacity-85 transition-opacity disabled:opacity-50 disabled:cursor-wait"
        >
          {isPending ? 'Installing…' : 'Install'}
        </button>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Main dialog
// ---------------------------------------------------------------------------

export interface PluginDetailDialogProps {
  plugin: PluginInfo | AvailablePlugin
  open: boolean
  onOpenChange: (open: boolean) => void
  onAction?: (action: string, name: string, scope?: string, projectPath?: string | null) => void
  onInstall?: (name: string, scope: string) => void
  isPending: boolean
  githubUrl?: string | null
}

export function PluginDetailDialog({
  plugin,
  open,
  onOpenChange,
  onAction,
  onInstall,
  isPending,
  githubUrl,
}: PluginDetailDialogProps) {
  const installed = isInstalled(plugin)
  const marketplace = installed ? plugin.marketplace : plugin.marketplaceName
  const version = installed
    ? plugin.gitSha
      ? plugin.gitSha.slice(0, 6)
      : plugin.version
    : plugin.version
  const scope = installed ? plugin.scope : null
  const hasFileViewer = installed && plugin.items.length > 0

  const contentClass = cn(
    'fixed z-[51] top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
    'w-[90vw] max-h-[85vh] flex flex-col rounded-2xl border border-apple-sep bg-white shadow-2xl overflow-hidden',
    installed ? 'max-w-4xl' : 'max-w-lg',
  )

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/30" />
        <Dialog.Content className={contentClass} onClick={(e) => e.stopPropagation()}>
          <DialogHeader
            plugin={plugin}
            installed={installed}
            marketplace={marketplace}
            version={version}
            scope={scope}
            githubUrl={githubUrl}
            onAction={onAction}
            isPending={isPending}
          />

          {/* Body */}
          <div className="flex flex-1 min-h-0">
            {installed ? (
              <>
                <div className={cn('flex flex-col min-h-0', hasFileViewer ? 'w-[55%]' : 'flex-1')}>
                  <InstalledLeftPanel plugin={plugin} onAction={onAction} isPending={isPending} />
                </div>
                {hasFileViewer && <FileViewerPanel plugin={plugin} open={open} />}
              </>
            ) : (
              <AvailableBody
                plugin={plugin as AvailablePlugin}
                onInstall={onInstall}
                isPending={isPending}
              />
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
