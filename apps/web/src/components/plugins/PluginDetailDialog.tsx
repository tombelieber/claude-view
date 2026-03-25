import * as Dialog from '@radix-ui/react-dialog'
import { Bot, ExternalLink, FileText, Server, Terminal, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { cn } from '../../lib/utils'
import type { AvailablePlugin, PluginInfo, PluginItem } from '../../types/generated'
import { DialogContent, DialogOverlay } from '../ui/CenteredDialog'
import { AppleToggle } from './AppleToggle'
import { formatInstallCount, formatRelativeTime } from './format-helpers'
import { marketplaceDotColor } from './marketplace-colors'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function isInstalled(p: PluginInfo | AvailablePlugin): p is PluginInfo {
  return 'scope' in p
}

function itemFileName(item: PluginItem): string {
  if (item.kind === 'mcp_tool') return item.name
  return `${item.name.replace(/\s+/g, '-').toLowerCase()}.md`
}

function ItemIcon({ kind }: { kind: string }) {
  if (kind === 'command') return <Terminal className="w-3 h-3 flex-shrink-0" />
  if (kind === 'agent') return <Bot className="w-3 h-3 flex-shrink-0" />
  if (kind === 'mcp_tool') return <Server className="w-3 h-3 flex-shrink-0" />
  return <FileText className="w-3 h-3 flex-shrink-0" />
}

function kindDotColor(kind: string): string {
  if (kind === 'skill') return 'bg-indigo-400'
  if (kind === 'command') return 'bg-emerald-400'
  if (kind === 'agent') return 'bg-blue-400'
  return 'bg-amber-400'
}

function kindBadgeClass(kind: string): string {
  if (kind === 'skill') return 'bg-indigo-50 text-indigo-600'
  if (kind === 'command') return 'bg-emerald-50 text-emerald-700'
  if (kind === 'agent') return 'bg-blue-50 text-blue-600'
  return 'bg-amber-50 text-amber-600'
}

// ---------------------------------------------------------------------------
// Header
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
  const scopeIsProject = scope?.toLowerCase() === 'project'
  return (
    <div className="flex items-center justify-between gap-3 px-4 pt-4 pb-3 border-b border-apple-sep2 flex-shrink-0">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5 flex-wrap">
          <Dialog.Title className="text-sm font-bold text-apple-text1">{plugin.name}</Dialog.Title>
          {scope && (
            <span
              className={cn(
                'text-xs px-1.5 py-0.5 rounded font-bold uppercase tracking-[0.05em]',
                scopeIsProject ? 'bg-green-50 text-green-700' : 'bg-blue-50 text-blue-600',
              )}
            >
              {scope}
            </span>
          )}
          {!installed && (plugin as AvailablePlugin).alreadyInstalled && (
            <span className="text-xs font-bold uppercase px-1.5 py-0.5 rounded-[5px] bg-[rgba(52,199,89,0.1)] text-[#248A3D] border border-[rgba(52,199,89,0.2)]">
              INSTALLED
            </span>
          )}
        </div>
        <div className="flex items-center gap-1 mt-0.5 text-xs text-apple-text3">
          <span
            className={cn(
              'w-1.5 h-1.5 rounded-full flex-shrink-0',
              marketplaceDotColor(marketplace),
            )}
          />
          <span>{marketplace}</span>
          <span className="text-apple-sep">·</span>
          <span className="font-mono">{version ?? '—'}</span>
          <span className="text-apple-sep">·</span>
          <span>{formatInstallCount(plugin.installCount)} installs</span>
        </div>
      </div>

      <div className="flex items-center gap-1 flex-shrink-0">
        {githubUrl && (
          <a
            href={githubUrl}
            target="_blank"
            rel="noopener noreferrer"
            title="View on GitHub"
            onClick={(e) => e.stopPropagation()}
            className="p-1.5 rounded-lg hover:bg-apple-sep2 transition-colors text-apple-text3"
          >
            <ExternalLink className="w-3.5 h-3.5" />
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
            <span className="text-xs text-apple-text3 leading-none select-none">
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
// Left panel: meta + contents list combined
// ---------------------------------------------------------------------------

interface LeftPanelProps {
  plugin: PluginInfo
  selectedIdx: number
  onSelect: (idx: number) => void
  onAction?: (action: string, name: string, scope?: string, projectPath?: string | null) => void
  isPending: boolean
  itemRefs: React.MutableRefObject<(HTMLButtonElement | null)[]>
}

function LeftPanel({
  plugin,
  selectedIdx,
  onSelect,
  onAction,
  isPending,
  itemRefs,
}: LeftPanelProps) {
  const invocations = Number(plugin.totalInvocations)
  const sessions = Number(plugin.sessionCount)

  return (
    <div className="w-[36%] flex-shrink-0 flex flex-col border-r border-apple-sep2 overflow-hidden">
      <Dialog.Description className="sr-only">{plugin.name} plugin details</Dialog.Description>

      {/* Meta section — tight spacing, zero waste */}
      <div className="px-4 pt-3 pb-3 space-y-1.5 flex-shrink-0">
        {plugin.description ? (
          <p className="text-xs text-apple-text2 leading-relaxed">{plugin.description}</p>
        ) : (
          <p className="text-xs text-apple-text3 italic">No description.</p>
        )}

        <div className="flex flex-wrap gap-x-2 text-xs text-apple-text3">
          <span>Installed {plugin.installedAt.split('T')[0]}</span>
          {plugin.lastUpdated && <span>· Updated {plugin.lastUpdated.split('T')[0]}</span>}
          {plugin.gitSha && (
            <span className="font-mono text-xs">SHA {plugin.gitSha.slice(0, 8)}</span>
          )}
        </div>

        {invocations > 0 && (
          <div className="text-xs text-apple-text3">
            {invocations.toLocaleString()}× · {sessions} session{sessions !== 1 ? 's' : ''}
            {plugin.lastUsedAt && ` · ${formatRelativeTime(Number(plugin.lastUsedAt))}`}
          </div>
        )}

        {plugin.duplicateMarketplaces.length > 0 && (
          <div className="px-2.5 py-1.5 rounded-lg bg-[rgba(255,149,0,0.07)] border border-[rgba(255,149,0,0.2)]">
            <span className="text-xs text-apple-text2">
              <strong className="text-[#B45309]">Conflict:</strong> also in{' '}
              {plugin.duplicateMarketplaces.join(', ')}
            </span>
          </div>
        )}

        {plugin.errors.length > 0 && (
          <div className="px-2.5 py-2 rounded-lg bg-[rgba(255,59,48,0.05)] border border-[rgba(255,59,48,0.18)]">
            <div className="text-xs font-bold text-[#C0392B]">
              {plugin.sourceExists ? 'CLI verification issue' : 'Orphaned install'}
            </div>
            <div className="flex gap-1.5 mt-1.5">
              {!plugin.sourceExists && (
                <button
                  type="button"
                  disabled={isPending}
                  onClick={(e) => {
                    e.stopPropagation()
                    onAction?.('install', plugin.name, plugin.scope)
                  }}
                  className="text-xs px-2.5 py-0.5 rounded-[6px] border border-[rgba(255,59,48,0.3)] text-[#C0392B] hover:bg-[rgba(255,59,48,0.07)] disabled:opacity-50"
                >
                  Reinstall
                </button>
              )}
              <button
                type="button"
                disabled={isPending}
                onClick={(e) => {
                  e.stopPropagation()
                  onAction?.('uninstall', plugin.name, plugin.scope, plugin.projectPath)
                }}
                className="text-xs px-2.5 py-0.5 rounded-[6px] bg-[rgba(255,59,48,0.1)] text-[#C0392B] hover:bg-[rgba(255,59,48,0.18)] disabled:opacity-50"
              >
                Remove
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Contents list — scrollable, fills remaining height */}
      {plugin.items.length > 0 && (
        <>
          <div className="px-4 py-1.5 border-t border-apple-sep2 flex-shrink-0">
            <span className="text-xs font-bold uppercase tracking-wide text-apple-text3">
              Contents ({plugin.items.length})
            </span>
          </div>
          <div className="flex-1 overflow-y-auto">
            {plugin.items.map((item, idx) => (
              <button
                key={item.id}
                ref={(el) => {
                  itemRefs.current[idx] = el
                }}
                type="button"
                onClick={(e) => {
                  e.stopPropagation()
                  onSelect(idx)
                }}
                className={cn(
                  'w-full flex items-center gap-2 py-1.5 px-4 text-left transition-colors',
                  idx === selectedIdx
                    ? 'bg-[rgba(0,122,255,0.08)] text-apple-blue'
                    : 'hover:bg-apple-sep2 text-apple-text2',
                )}
              >
                <span
                  className={cn('w-1.5 h-1.5 rounded-full flex-shrink-0', kindDotColor(item.kind))}
                />
                <ItemIcon kind={item.kind} />
                <span className="text-xs font-mono truncate">{itemFileName(item)}</span>
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Right panel — markdown file viewer (skill / command / agent)
// ---------------------------------------------------------------------------

const mdComponents = {
  h1: ({ children }: { children?: React.ReactNode }) => (
    <h1 className="text-sm font-bold text-apple-text1 mt-3 mb-1.5">{children}</h1>
  ),
  h2: ({ children }: { children?: React.ReactNode }) => (
    <h2 className="text-xs font-semibold text-apple-text1 mt-2.5 mb-1">{children}</h2>
  ),
  h3: ({ children }: { children?: React.ReactNode }) => (
    <h3 className="text-xs font-semibold text-apple-text2 mt-2 mb-0.5">{children}</h3>
  ),
  p: ({ children }: { children?: React.ReactNode }) => (
    <p className="text-xs text-apple-text2 leading-relaxed mb-2">{children}</p>
  ),
  ul: ({ children }: { children?: React.ReactNode }) => (
    <ul className="list-disc list-inside text-xs text-apple-text2 space-y-0.5 mb-2 pl-1">
      {children}
    </ul>
  ),
  ol: ({ children }: { children?: React.ReactNode }) => (
    <ol className="list-decimal list-inside text-xs text-apple-text2 space-y-0.5 mb-2 pl-1">
      {children}
    </ol>
  ),
  li: ({ children }: { children?: React.ReactNode }) => (
    <li className="leading-relaxed">{children}</li>
  ),
  code: ({ children, className }: { children?: React.ReactNode; className?: string }) => {
    const isBlock = className?.startsWith('language-')
    return isBlock ? (
      <code className="block bg-[#f0f2f5] border border-apple-sep2 rounded-lg p-3 text-xs font-mono text-apple-text1 whitespace-pre-wrap overflow-x-auto mb-2">
        {children}
      </code>
    ) : (
      <code className="bg-apple-sep2 text-apple-text1 rounded px-1 py-0.5 text-xs font-mono">
        {children}
      </code>
    )
  },
  pre: ({ children }: { children?: React.ReactNode }) => <>{children}</>,
  blockquote: ({ children }: { children?: React.ReactNode }) => (
    <blockquote className="border-l-2 border-apple-sep pl-3 text-xs text-apple-text3 italic my-2">
      {children}
    </blockquote>
  ),
  hr: () => <hr className="border-apple-sep2 my-3" />,
  strong: ({ children }: { children?: React.ReactNode }) => (
    <strong className="font-semibold text-apple-text1">{children}</strong>
  ),
  a: ({ href, children }: { href?: string; children?: React.ReactNode }) => (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="text-apple-blue hover:underline"
    >
      {children}
    </a>
  ),
}

function FileContentViewer({ item }: { item: PluginItem | null }) {
  const invocations = item ? Number(item.invocationCount) : 0
  const lastUsed = item?.lastUsedAt ? formatRelativeTime(Number(item.lastUsedAt)) : null

  if (!item) return null

  // MCP server items get a dedicated structured config viewer
  if (item.kind === 'mcp_tool') {
    return <McpServerViewer item={item} invocations={invocations} lastUsed={lastUsed} />
  }

  const content = item.content?.trim() || null

  return (
    <div className="flex-1 flex flex-col min-h-0 overflow-hidden">
      {/* Breadcrumb bar */}
      <div className="px-4 py-2 border-b border-apple-sep2 flex-shrink-0 flex items-center gap-2">
        <span className="text-xs font-mono text-apple-text2 truncate">{itemFileName(item)}</span>
        <span
          className={cn(
            'text-xs font-bold uppercase tracking-wide px-1.5 py-0.5 rounded flex-shrink-0',
            kindBadgeClass(item.kind),
          )}
        >
          {item.kind.replace('_', ' ')}
        </span>
        {invocations > 0 && (
          <span className="text-xs text-apple-text3 tabular-nums flex-shrink-0">
            {invocations}×{lastUsed && ` · ${lastUsed}`}
          </span>
        )}
      </div>

      {/* Markdown content */}
      <div className="flex-1 overflow-y-auto bg-[#fafafa] px-4 py-3">
        {content ? (
          <Markdown remarkPlugins={[remarkGfm]} components={mdComponents}>
            {content}
          </Markdown>
        ) : (
          <p className="text-xs text-apple-text3 italic">No content available.</p>
        )}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// MCP server viewer — structured config display
// ---------------------------------------------------------------------------

interface McpConfig {
  command?: string
  args?: string[]
  env?: Record<string, string>
  url?: string
  type?: string
  [key: string]: unknown
}

function McpServerViewer({
  item,
  invocations,
  lastUsed,
}: {
  item: PluginItem
  invocations: number
  lastUsed: string | null
}) {
  let config: McpConfig | null = null
  try {
    if (item.content) config = JSON.parse(item.content) as McpConfig
  } catch {
    // malformed JSON — fall back to raw display
  }

  const isHttp = config?.url || config?.type === 'http' || config?.type === 'sse'
  const cmdParts = config?.command ? [config.command, ...(config.args ?? [])].join(' ') : null
  const envVars = config?.env ? Object.entries(config.env) : []
  const otherKeys = config
    ? Object.keys(config).filter((k) => !['command', 'args', 'env', 'url', 'type'].includes(k))
    : []

  return (
    <div className="flex-1 flex flex-col min-h-0 overflow-hidden">
      {/* Header bar */}
      <div className="px-4 py-2 border-b border-apple-sep2 flex-shrink-0 flex items-center gap-2">
        <Server className="w-3.5 h-3.5 text-amber-500 flex-shrink-0" />
        <span className="text-xs font-mono text-apple-text2 truncate">{item.name}</span>
        <span className="text-xs font-bold uppercase tracking-wide px-1.5 py-0.5 rounded flex-shrink-0 bg-amber-50 text-amber-600">
          {isHttp ? (config?.type ?? 'http') : 'stdio'}
        </span>
        {invocations > 0 && (
          <span className="text-xs text-apple-text3 tabular-nums flex-shrink-0">
            {invocations}×{lastUsed && ` · ${lastUsed}`}
          </span>
        )}
      </div>

      {/* Config body */}
      <div className="flex-1 overflow-y-auto bg-[#fafafa] px-4 py-3 space-y-3">
        {!config ? (
          <p className="text-xs text-apple-text3 italic">No server configuration available.</p>
        ) : (
          <>
            {/* Command */}
            {cmdParts && (
              <div>
                <div className="text-xs font-bold uppercase tracking-wide text-apple-text3 mb-1">
                  Command
                </div>
                <div className="bg-[#f0f2f5] border border-apple-sep2 rounded-lg px-3 py-2 font-mono text-xs text-apple-text1 whitespace-pre-wrap break-all">
                  {cmdParts}
                </div>
              </div>
            )}

            {/* URL */}
            {config.url && (
              <div>
                <div className="text-xs font-bold uppercase tracking-wide text-apple-text3 mb-1">
                  URL
                </div>
                <div className="bg-[#f0f2f5] border border-apple-sep2 rounded-lg px-3 py-2 font-mono text-xs text-apple-text1 break-all">
                  {config.url}
                </div>
              </div>
            )}

            {/* Environment variables */}
            {envVars.length > 0 && (
              <div>
                <div className="text-xs font-bold uppercase tracking-wide text-apple-text3 mb-1">
                  Environment ({envVars.length})
                </div>
                <div className="border border-apple-sep2 rounded-lg overflow-hidden">
                  {envVars.map(([key, val], i) => (
                    <div
                      key={key}
                      className={cn(
                        'flex items-start gap-3 px-3 py-1.5 text-xs',
                        i > 0 && 'border-t border-apple-sep2',
                      )}
                    >
                      <span className="font-mono text-apple-text1 font-semibold flex-shrink-0 min-w-0 w-[40%] truncate">
                        {key}
                      </span>
                      <span className="font-mono text-apple-text3 min-w-0 break-all">{val}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Other top-level keys */}
            {otherKeys.length > 0 && (
              <div>
                <div className="text-xs font-bold uppercase tracking-wide text-apple-text3 mb-1">
                  Config
                </div>
                <div className="border border-apple-sep2 rounded-lg overflow-hidden">
                  {otherKeys.map((key, i) => (
                    <div
                      key={key}
                      className={cn(
                        'flex items-start gap-3 px-3 py-1.5 text-xs',
                        i > 0 && 'border-t border-apple-sep2',
                      )}
                    >
                      <span className="font-mono text-apple-text3 font-medium flex-shrink-0 w-[40%] truncate">
                        {key}
                      </span>
                      <span className="font-mono text-apple-text2 min-w-0 break-all">
                        {typeof config[key] === 'string'
                          ? config[key]
                          : JSON.stringify(config[key])}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Available plugin body
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
    <div className="overflow-y-auto flex-1 px-5 py-4 space-y-3">
      <Dialog.Description className="sr-only">{plugin.name} plugin details</Dialog.Description>
      {plugin.description ? (
        <p className="text-xs text-apple-text2 leading-relaxed">{plugin.description}</p>
      ) : (
        <p className="text-xs text-apple-text3 italic">No description provided.</p>
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

  const [selectedIdx, setSelectedIdx] = useState(0)
  // Ref array for each list item — used for auto-scroll on keyboard nav
  const itemRefs = useRef<(HTMLButtonElement | null)[]>([])

  // Reset selection when plugin changes
  // biome-ignore lint/correctness/useExhaustiveDependencies: plugin identity is the intentional trigger
  useEffect(() => {
    setSelectedIdx(0)
    itemRefs.current = []
  }, [plugin])

  // Auto-scroll selected item into view when index changes via keyboard
  useEffect(() => {
    itemRefs.current[selectedIdx]?.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
  }, [selectedIdx])

  // Keyboard navigation
  useEffect(() => {
    if (!open || !installed || plugin.items.length === 0) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setSelectedIdx((i) => Math.min(i + 1, plugin.items.length - 1))
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault()
        setSelectedIdx((i) => Math.max(i - 1, 0))
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open, installed, plugin])

  const contentClass = cn(
    'w-[90vw] max-h-[85vh] flex flex-col rounded-2xl border border-apple-sep bg-white shadow-2xl overflow-hidden',
    installed ? 'max-w-4xl' : 'max-w-lg',
  )

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <DialogOverlay className="bg-black/30" />
        <DialogContent className={contentClass} onClick={(e) => e.stopPropagation()}>
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

          <div className="flex flex-1 min-h-0">
            {installed ? (
              <>
                <LeftPanel
                  plugin={plugin}
                  selectedIdx={selectedIdx}
                  onSelect={setSelectedIdx}
                  onAction={onAction}
                  isPending={isPending}
                  itemRefs={itemRefs}
                />
                <FileContentViewer item={plugin.items[selectedIdx] ?? null} />
              </>
            ) : (
              <AvailableBody
                plugin={plugin as AvailablePlugin}
                onInstall={onInstall}
                isPending={isPending}
              />
            )}
          </div>
        </DialogContent>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
