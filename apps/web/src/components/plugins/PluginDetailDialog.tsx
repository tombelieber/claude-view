import * as Dialog from '@radix-ui/react-dialog'
import { X } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { AvailablePlugin, PluginInfo, PluginItem } from '../../types/generated'
import { AppleToggle } from './AppleToggle'
import { formatInstallCount, formatRelativeTime } from './format-helpers'
import { marketplaceDotColor } from './marketplace-colors'

function isInstalled(p: PluginInfo | AvailablePlugin): p is PluginInfo {
  return 'scope' in p
}

// ---------------------------------------------------------------------------
// Items listing (used only in dialog body)
// ---------------------------------------------------------------------------

function ItemRow({ item }: { item: PluginItem }) {
  const count = Number(item.invocationCount)
  const lastUsed = item.lastUsedAt ? formatRelativeTime(Number(item.lastUsedAt)) : null
  // treat placeholder "---" or empty as no description
  const desc = item.description && item.description !== '---' ? item.description : null
  return (
    <div className="py-1.5 border-b border-apple-sep2 last:border-0">
      <div className="flex items-center justify-between text-xs">
        <span className="text-apple-text1 font-medium truncate">{item.name}</span>
        <span className="text-apple-text3 whitespace-nowrap ml-2 tabular-nums">
          {count > 0 && `${count}×`}
          {lastUsed && ` · ${lastUsed}`}
        </span>
      </div>
      {desc && <div className="text-[11px] text-apple-text3 truncate mt-0.5">{desc}</div>}
    </div>
  )
}

function ItemsGroup({ label, items }: { label: string; items: PluginItem[] }) {
  if (items.length === 0) return null
  return (
    <div>
      <div className="text-[10px] font-bold uppercase tracking-wide text-apple-text3 mb-0.5">
        {label} ({items.length})
      </div>
      {items.map((item) => (
        <ItemRow key={item.id} item={item} />
      ))}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Dialog body variants
// ---------------------------------------------------------------------------

function InstalledBody({
  plugin,
  onAction,
  isPending,
}: {
  plugin: PluginInfo
  onAction?: (action: string, name: string, scope?: string, projectPath?: string | null) => void
  isPending: boolean
}) {
  const skills = plugin.items.filter((i) => i.kind === 'skill')
  const agents = plugin.items.filter((i) => i.kind === 'agent')
  const commands = plugin.items.filter((i) => i.kind === 'command')
  const mcpTools = plugin.items.filter((i) => i.kind === 'mcp_tool')
  const invocations = Number(plugin.totalInvocations)
  const sessions = Number(plugin.sessionCount)

  return (
    <>
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

      {/* Items listing */}
      {plugin.items.length > 0 && (
        <div className="border-t border-apple-sep2 pt-3 space-y-3">
          <ItemsGroup label="Skills" items={skills} />
          <ItemsGroup label="Agents" items={agents} />
          <ItemsGroup label="Commands" items={commands} />
          <ItemsGroup label="MCP Tools" items={mcpTools} />
        </div>
      )}

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
              onClick={() => onAction?.('update', plugin.name, plugin.scope)}
              disabled={isPending}
              className="text-[11px] font-medium px-3 py-1 rounded-[7px] border border-[rgba(255,59,48,0.3)] bg-transparent text-[#C0392B] hover:bg-[rgba(255,59,48,0.07)] disabled:opacity-50 disabled:cursor-wait transition-colors"
            >
              Reinstall
            </button>
            <button
              type="button"
              onClick={() => onAction?.('uninstall', plugin.name, plugin.scope, plugin.projectPath)}
              disabled={isPending}
              className="text-[11px] font-medium px-3 py-1 rounded-[7px] bg-[rgba(255,59,48,0.1)] text-[#C0392B] hover:bg-[rgba(255,59,48,0.18)] disabled:opacity-50 disabled:cursor-wait transition-colors"
            >
              Remove
            </button>
          </div>
        </div>
      )}
    </>
  )
}

function AvailableBody({
  plugin,
  onInstall,
  isPending,
}: {
  plugin: AvailablePlugin
  onInstall?: (name: string, scope: string) => void
  isPending: boolean
}) {
  if (plugin.alreadyInstalled) return null
  return (
    <button
      type="button"
      disabled={isPending}
      onClick={() => onInstall?.(plugin.name, 'user')}
      className="inline-flex items-center gap-1.5 text-sm font-semibold px-4 py-2 rounded-[10px] bg-apple-blue text-white hover:opacity-85 transition-opacity disabled:opacity-50 disabled:cursor-wait"
    >
      {isPending ? 'Installing…' : 'Install'}
    </button>
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
}

export function PluginDetailDialog({
  plugin,
  open,
  onOpenChange,
  onAction,
  onInstall,
  isPending,
}: PluginDetailDialogProps) {
  const installed = isInstalled(plugin)
  const marketplace = installed ? plugin.marketplace : plugin.marketplaceName
  const version = installed
    ? plugin.gitSha
      ? plugin.gitSha.slice(0, 6)
      : plugin.version
    : plugin.version
  const scope = installed ? plugin.scope : null

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        {/* Overlay */}
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/30" />
        {/* Centering wrapper: pointer-events-none so clicks outside content hit the overlay */}
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 pointer-events-none">
          <Dialog.Content
            className="pointer-events-auto w-full max-w-lg max-h-[80vh] flex flex-col rounded-2xl border border-apple-sep bg-white shadow-2xl"
            onClick={(e) => e.stopPropagation()}
          >
            {/* ── Header (flex-shrink-0 = never scrolls away) ── */}
            <div className="flex items-start justify-between gap-3 px-5 pt-5 pb-3 border-b border-apple-sep2 flex-shrink-0">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2 flex-wrap">
                  <Dialog.Title className="text-[15px] font-bold text-apple-text1">
                    {plugin.name}
                  </Dialog.Title>
                  {/* Scope badge moved to header — inline with title */}
                  {scope && (
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
                  )}
                  {!installed && plugin.alreadyInstalled && (
                    <span className="text-[10px] font-bold uppercase tracking-wide px-1.5 py-0.5 rounded-[5px] bg-[rgba(52,199,89,0.1)] text-[#248A3D] border border-[rgba(52,199,89,0.2)]">
                      INSTALLED
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-1.5 mt-1 text-[11px] text-apple-text3">
                  <span className="flex items-center gap-1">
                    <span
                      className={cn('w-1.5 h-1.5 rounded-full', marketplaceDotColor(marketplace))}
                    />
                    {marketplace}
                  </span>
                  <span className="text-apple-sep">·</span>
                  <span className="font-mono">{version ?? '—'}</span>
                  <span className="text-apple-sep">·</span>
                  <span>{formatInstallCount(plugin.installCount)} installs</span>
                </div>
              </div>

              {/* ── Enable / Disable toggle — always visible in fixed header ── */}
              {installed && (
                <div className="flex flex-col items-center gap-0.5 flex-shrink-0 pt-0.5">
                  <AppleToggle
                    checked={plugin.enabled}
                    onChange={() =>
                      onAction?.(
                        plugin.enabled ? 'disable' : 'enable',
                        plugin.name,
                        plugin.scope,
                        plugin.projectPath,
                      )
                    }
                    disabled={isPending}
                  />
                  <span className="text-[9px] text-apple-text3 leading-none select-none">
                    {plugin.enabled ? 'Enabled' : 'Disabled'}
                  </span>
                </div>
              )}

              <Dialog.Close asChild>
                <button
                  type="button"
                  className="flex-shrink-0 p-1.5 rounded-lg hover:bg-apple-sep2 transition-colors text-apple-text3"
                >
                  <X className="w-4 h-4" />
                </button>
              </Dialog.Close>
            </div>

            {/* ── Scrollable body ── */}
            <div className="overflow-y-auto flex-1 px-5 py-4 space-y-4">
              <Dialog.Description className="sr-only">
                {plugin.name} plugin details
              </Dialog.Description>
              {plugin.description ? (
                <p className="text-[13px] text-apple-text2 leading-relaxed">{plugin.description}</p>
              ) : (
                <p className="text-[13px] text-apple-text3 italic">No description provided.</p>
              )}
              {installed ? (
                <InstalledBody plugin={plugin} onAction={onAction} isPending={isPending} />
              ) : (
                <AvailableBody plugin={plugin} onInstall={onInstall} isPending={isPending} />
              )}
            </div>
          </Dialog.Content>
        </div>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
