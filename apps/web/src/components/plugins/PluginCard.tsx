import { ExternalLink } from 'lucide-react'
import { useEffect, useState } from 'react'
import { cn } from '../../lib/utils'
import type { PluginInfo } from '../../types/generated'
import { AppleToggle } from './AppleToggle'
import { PluginActionMenu } from './PluginActionMenu'
import { PluginDetailDialog } from './PluginDetailDialog'
import { formatInstallCount, formatRelativeTime } from './format-helpers'
import { marketplaceDotColor } from './marketplace-colors'

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function ScopeBadge({ scope }: { scope: string }) {
  const isProject = scope.toLowerCase() === 'project'
  return (
    <span
      className={cn(
        'text-xs px-1.5 py-0.5 rounded font-medium uppercase',
        isProject ? 'bg-green-50 text-green-700' : 'bg-blue-50 text-blue-600',
      )}
    >
      {scope}
    </span>
  )
}

function ContentsLine({ plugin }: { plugin: PluginInfo }) {
  const parts: string[] = []
  if (plugin.skillCount > 0)
    parts.push(`${plugin.skillCount} skill${plugin.skillCount > 1 ? 's' : ''}`)
  if (plugin.agentCount > 0)
    parts.push(`${plugin.agentCount} agent${plugin.agentCount > 1 ? 's' : ''}`)
  if (plugin.commandCount > 0)
    parts.push(`${plugin.commandCount} cmd${plugin.commandCount > 1 ? 's' : ''}`)
  if (plugin.mcpCount > 0) parts.push(`${plugin.mcpCount} MCP`)
  if (parts.length === 0) return null
  return <span className="text-xs text-apple-text2">{parts.join(' \u00b7 ')}</span>
}

function UsageLine({ plugin }: { plugin: PluginInfo }) {
  const invocations = Number(plugin.totalInvocations)
  const sessions = Number(plugin.sessionCount)
  const lastUsed = plugin.lastUsedAt ? formatRelativeTime(Number(plugin.lastUsedAt)) : null
  if (invocations === 0) {
    return <span className="text-xs text-apple-text3">No usage recorded</span>
  }
  return (
    <span className="text-xs text-apple-text3">
      {invocations.toLocaleString()}&times; across {sessions} session{sessions !== 1 ? 's' : ''}
      {lastUsed && ` · ${lastUsed}`}
    </span>
  )
}

function DuplicateWarning({ marketplaces }: { marketplaces: string[] }) {
  if (marketplaces.length === 0) return null
  return (
    <div className="mt-2.5 p-2.5 rounded-lg bg-[rgba(255,149,0,0.07)] border border-[rgba(255,149,0,0.2)]">
      <div className="flex items-start gap-1.5">
        <span className="text-xs flex-shrink-0 mt-px">⚑</span>
        <div>
          <span className="text-xs text-apple-text2 leading-relaxed">
            <strong className="text-[#B45309] font-semibold">Conflict:</strong> also in{' '}
            {marketplaces.join(', ')}. This version runs; marketplace updates won't apply.
          </span>
          <span className="block mt-1 text-xs text-apple-blue font-medium cursor-pointer">
            Resolve →
          </span>
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Main card
// ---------------------------------------------------------------------------

export interface PluginCardProps {
  plugin: PluginInfo
  onAction: (action: string, name: string, scope?: string, projectPath?: string | null) => void
  isPending: boolean
  githubUrl?: string | null
}

export function PluginCard({ plugin, onAction, isPending, githubUrl }: PluginCardProps) {
  const [dialogOpen, setDialogOpen] = useState(false)

  // Optimistic toggle — flip immediately, sync back when server data arrives
  const [optimisticEnabled, setOptimisticEnabled] = useState(plugin.enabled)
  useEffect(() => {
    setOptimisticEnabled(plugin.enabled)
  }, [plugin.enabled])

  const version = plugin.gitSha ? plugin.gitSha.slice(0, 6) : plugin.version
  const installCount = formatInstallCount(plugin.installCount)

  return (
    <>
      <div
        className={cn(
          'group w-full text-left rounded-xl border p-3 transition-all duration-200 cursor-pointer',
          'border-apple-sep2 bg-white',
          'hover:border-apple-sep hover:shadow-[0_3px_10px_rgba(0,0,0,0.08)]',
          'shadow-[0_1px_2px_rgba(0,0,0,0.04)]',
          plugin.errors.length > 0 &&
            'border-[rgba(255,59,48,0.22)] bg-[rgba(255,59,48,0.025)] hover:border-[rgba(255,59,48,0.45)]',
        )}
        onClick={() => setDialogOpen(true)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') setDialogOpen(true)
        }}
      >
        {/* Row 1: name + scope badge + disabled badge | github link + toggle + menu */}
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2 min-w-0">
            <h3 className="text-sm font-semibold text-apple-text truncate">{plugin.name}</h3>
            <ScopeBadge scope={plugin.scope} />
            {!optimisticEnabled && (
              <span className="text-xs px-1.5 py-0.5 rounded font-medium uppercase bg-apple-fill2 text-apple-text2 flex-shrink-0">
                Disabled
              </span>
            )}
          </div>
          <div className="flex items-center gap-1.5 flex-shrink-0">
            {githubUrl && (
              <a
                href={githubUrl}
                target="_blank"
                rel="noopener noreferrer"
                title="View on GitHub"
                onClick={(e) => e.stopPropagation()}
                className="p-1 rounded hover:bg-apple-sep2 text-apple-text3 hover:text-apple-text2 transition-colors"
              >
                <ExternalLink className="w-3.5 h-3.5" />
              </a>
            )}
            <span onClick={(e) => e.stopPropagation()}>
              <AppleToggle
                checked={optimisticEnabled}
                size="sm"
                onChange={() => {
                  setOptimisticEnabled(!optimisticEnabled)
                  onAction(
                    optimisticEnabled ? 'disable' : 'enable',
                    plugin.id,
                    plugin.scope,
                    plugin.projectPath,
                  )
                }}
                disabled={isPending}
              />
            </span>
            <span onClick={(e) => e.stopPropagation()}>
              <PluginActionMenu plugin={plugin} onAction={onAction} isPending={isPending} />
            </span>
          </div>
        </div>

        {/* Row 2: marketplace + version + install count */}
        <div className="flex items-center gap-2 mt-1 text-xs text-apple-text2">
          <span className="flex items-center gap-1">
            <span
              className={cn(
                'w-2 h-2 rounded-full inline-block',
                marketplaceDotColor(plugin.marketplace),
              )}
            />
            {plugin.marketplace}
          </span>
          {version && (
            <>
              <span className="text-apple-sep">&middot;</span>
              <span className="font-mono">{version}</span>
            </>
          )}
          {installCount != null && (
            <>
              <span className="text-apple-sep">&middot;</span>
              <span>{installCount} installs</span>
            </>
          )}
        </div>

        {/* Row 3: description */}
        {plugin.description && (
          <p className="text-xs text-apple-text2 mt-1.5 line-clamp-2">{plugin.description}</p>
        )}

        {/* Row 4: contents + usage */}
        <div className="mt-2 flex flex-col gap-0.5">
          <ContentsLine plugin={plugin} />
          <UsageLine plugin={plugin} />
        </div>

        {/* Duplicate warning */}
        {plugin.duplicateMarketplaces.length > 0 && (
          <div className="mt-2">
            <DuplicateWarning marketplaces={plugin.duplicateMarketplaces} />
          </div>
        )}

        {/* Error block — source_exists from backend is the authoritative signal:
            false = files missing (truly orphaned), true = CLI validation failure (files intact) */}
        {plugin.errors.length > 0 && (
          <div className="mt-2.5 p-2.5 rounded-lg bg-[rgba(255,59,48,0.05)] border border-[rgba(255,59,48,0.18)]">
            {plugin.sourceExists ? (
              <>
                <div className="text-xs font-bold text-[#C0392B] mb-0.5">
                  CLI verification issue
                </div>
                <div className="text-xs text-apple-text2 mb-2 leading-relaxed">
                  Plugin files are intact. The CLI can't verify it against a marketplace catalog.
                </div>
              </>
            ) : (
              <>
                <div className="text-xs font-bold text-[#C0392B] mb-0.5">Orphaned install</div>
                <div className="text-xs text-apple-text2 mb-2 leading-relaxed">
                  Source directory missing. Can't update or run.
                </div>
              </>
            )}
            <div className="flex gap-1.5">
              {!plugin.sourceExists && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation()
                    onAction('install', plugin.name, plugin.scope)
                  }}
                  disabled={isPending}
                  className="text-xs font-medium px-3 py-1 rounded-[7px] border border-[rgba(255,59,48,0.3)] bg-transparent text-[#C0392B] hover:bg-[rgba(255,59,48,0.07)] transition-colors disabled:opacity-50"
                >
                  Reinstall
                </button>
              )}
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation()
                  onAction('uninstall', plugin.name, plugin.scope, plugin.projectPath)
                }}
                disabled={isPending}
                className="text-xs font-medium px-3 py-1 rounded-[7px] border-none bg-[rgba(255,59,48,0.1)] text-[#C0392B] hover:bg-[rgba(255,59,48,0.18)] transition-colors disabled:opacity-50"
              >
                Remove
              </button>
            </div>
          </div>
        )}
      </div>

      <PluginDetailDialog
        plugin={plugin}
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        onAction={onAction}
        isPending={isPending}
        githubUrl={githubUrl}
      />
    </>
  )
}
