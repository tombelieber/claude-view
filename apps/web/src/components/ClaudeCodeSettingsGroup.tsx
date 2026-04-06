import { ChevronRight, Key, Loader2, Plug, Settings2, Shield, Webhook } from 'lucide-react'
import { useState } from 'react'
import { useClaudeCodeSettings } from '../hooks/use-claude-code-settings'
import type { ClaudeCodeSettings, PluginEntry, UserHook } from '../hooks/use-claude-code-settings'
import { cn } from '../lib/utils'

// ── Reusable mini-components matching SettingsPage patterns ──

function SettingsSection({
  icon,
  title,
  children,
}: {
  icon: React.ReactNode
  title: string
  children: React.ReactNode
}) {
  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700">
      <div className="px-5 pt-4 pb-1.5">
        <div className="flex items-center gap-2">
          <span className="text-gray-400 dark:text-gray-500">{icon}</span>
          <h2 className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider">
            {title}
          </h2>
        </div>
      </div>
      <div className="px-5 pb-5 pt-1">{children}</div>
    </div>
  )
}

function InfoRow({ label, value }: { label: string; value: string | null }) {
  return (
    <div className="flex items-center justify-between py-1.5">
      <span className="text-sm text-gray-500 dark:text-gray-400">{label}</span>
      <span className="text-sm font-medium text-gray-900 dark:text-gray-100 tabular-nums">
        {value ?? '--'}
      </span>
    </div>
  )
}

// ── Environment Section ──

function EnvironmentSection({ settings }: { settings: ClaudeCodeSettings }) {
  if (settings.env.length === 0) return null
  return (
    <SettingsSection icon={<Key className="w-4 h-4" />} title="Environment">
      <div className="space-y-0">
        {settings.env.map((ev) => (
          <InfoRow key={ev.key} label={ev.key} value={ev.redacted ? '••••••••' : ev.value} />
        ))}
      </div>
    </SettingsSection>
  )
}

// ── Permissions Section ──

function PermissionsSection({ settings }: { settings: ClaudeCodeSettings }) {
  const { permissions, defaultMode } = settings
  const hasAny =
    permissions.allow.length > 0 || permissions.deny.length > 0 || permissions.ask.length > 0

  return (
    <SettingsSection icon={<Shield className="w-4 h-4" />} title="Permissions">
      {defaultMode && <InfoRow label="Default mode" value={defaultMode} />}
      {!hasAny && (
        <p className="text-sm text-gray-500 dark:text-gray-400 py-1">No custom permission rules.</p>
      )}
      {permissions.allow.length > 0 && (
        <div className="mt-2">
          <p className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider mb-1.5">
            Allow ({permissions.allow.length})
          </p>
          <div className="rounded-md border border-gray-100 dark:border-gray-800 overflow-hidden">
            {permissions.allow.map((rule, i) => (
              <code
                key={i}
                className="block px-3 py-1.5 text-xs font-mono text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-900/50 border-b border-gray-100 dark:border-gray-800/50 last:border-b-0 truncate"
              >
                {rule}
              </code>
            ))}
          </div>
        </div>
      )}
      {permissions.deny.length > 0 && (
        <div className="mt-2">
          <p className="text-xs font-medium text-red-400 uppercase tracking-wider mb-1.5">
            Deny ({permissions.deny.length})
          </p>
          <div className="rounded-md border border-red-100 dark:border-red-900/50 overflow-hidden">
            {permissions.deny.map((rule, i) => (
              <code
                key={i}
                className="block px-3 py-1.5 text-xs font-mono text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-900/20 border-b border-red-100 dark:border-red-900/50 last:border-b-0"
              >
                {rule}
              </code>
            ))}
          </div>
        </div>
      )}
    </SettingsSection>
  )
}

// ── Hooks Section ──

function UserHookCard({ hook }: { hook: UserHook }) {
  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-800 p-3 space-y-1">
      <div className="flex items-center gap-2">
        <span className="text-sm font-semibold text-gray-900 dark:text-gray-100">{hook.event}</span>
        {hook.matcher && (
          <span className="text-xs text-gray-400 dark:text-gray-500">matcher: {hook.matcher}</span>
        )}
        {hook.isAsync && (
          <span className="text-xs px-1.5 py-0.5 rounded bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400">
            async
          </span>
        )}
      </div>
      <code className="block text-xs font-mono text-gray-600 dark:text-gray-400 truncate">
        → {hook.command}
      </code>
    </div>
  )
}

function HooksSection({ settings }: { settings: ClaudeCodeSettings }) {
  const [showSystem, setShowSystem] = useState(false)

  return (
    <SettingsSection icon={<Webhook className="w-4 h-4" />} title="Hooks">
      {settings.userHooks.length > 0 && (
        <div className="space-y-2 mb-3">
          <p className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider">
            Your Hooks
          </p>
          {settings.userHooks.map((hook, i) => (
            <UserHookCard key={`${hook.event}-${i}`} hook={hook} />
          ))}
        </div>
      )}
      {settings.userHooks.length === 0 && (
        <p className="text-sm text-gray-500 dark:text-gray-400 py-1 mb-2">No user-defined hooks.</p>
      )}
      {settings.systemHookCount > 0 && (
        <div>
          <button
            type="button"
            onClick={() => setShowSystem(!showSystem)}
            className="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 cursor-pointer transition-colors"
          >
            <ChevronRight
              className={cn('w-3 h-3 transition-transform duration-200', showSystem && 'rotate-90')}
            />
            claude-view monitors {settings.systemHookCount} events
          </button>
          {showSystem && (
            <div className="mt-2 space-y-1.5 ml-5">
              {settings.systemHooks.map((hook, i) => (
                <div
                  key={`sys-${hook.event}-${i}`}
                  className="flex items-center gap-2 text-xs text-gray-400 dark:text-gray-500"
                >
                  <span className="font-mono">{hook.event}</span>
                  {hook.matcher && <span>({hook.matcher})</span>}
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </SettingsSection>
  )
}

// ── Plugins Section ──

function PluginRow({ plugin }: { plugin: PluginEntry }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1.5">
      <span
        className={cn(
          'w-1.5 h-1.5 rounded-full flex-shrink-0',
          plugin.enabled ? 'bg-green-500' : 'bg-gray-400',
        )}
      />
      <span className="text-sm text-gray-900 dark:text-gray-100 font-medium flex-1 truncate">
        {plugin.id}
      </span>
      <span className="text-xs text-gray-400 dark:text-gray-500 truncate max-w-[160px]">
        {plugin.marketplace}
      </span>
      {!plugin.enabled && (
        <span className="text-xs text-gray-400 dark:text-gray-500 italic flex-shrink-0">
          disabled
        </span>
      )}
    </div>
  )
}

function PluginsSection({ settings }: { settings: ClaudeCodeSettings }) {
  const enabledCount = settings.plugins.filter((p) => p.enabled).length
  const disabledCount = settings.plugins.length - enabledCount

  if (settings.plugins.length === 0) return null

  return (
    <SettingsSection icon={<Plug className="w-4 h-4" />} title="Plugins">
      <p className="text-sm text-gray-500 dark:text-gray-400 mb-2">
        {enabledCount} enabled · {disabledCount} disabled
      </p>
      <div className="rounded-md border border-gray-100 dark:border-gray-800 overflow-hidden divide-y divide-gray-50 dark:divide-gray-800/50">
        {settings.plugins.map((plugin) => (
          <PluginRow key={`${plugin.id}@${plugin.marketplace}`} plugin={plugin} />
        ))}
      </div>
    </SettingsSection>
  )
}

// ── Misc Section ──

function MiscSection({ settings }: { settings: ClaudeCodeSettings }) {
  return (
    <SettingsSection icon={<Settings2 className="w-4 h-4" />} title="Misc">
      <div className="space-y-0">
        <InfoRow label="Voice enabled" value={settings.voiceEnabled ? 'yes' : 'no'} />
        <InfoRow
          label="Skip dangerous prompt"
          value={settings.skipDangerousPrompt ? 'yes' : 'no'}
        />
        {settings.statusLine && (
          <InfoRow
            label="Status line"
            value={
              settings.statusLine.length > 40
                ? `${settings.statusLine.slice(settings.statusLine.lastIndexOf('/') + 1)}`
                : settings.statusLine
            }
          />
        )}
        {settings.customMarketplaces.length > 0 && (
          <InfoRow
            label="Custom marketplaces"
            value={`${settings.customMarketplaces.length} (${settings.customMarketplaces.map((m) => m.name).join(', ')})`}
          />
        )}
      </div>
    </SettingsSection>
  )
}

// ── Main Group (exported for SettingsPage.tsx) ──

export function ClaudeCodeSettingsGroup() {
  const { data: settings, isLoading, error } = useClaudeCodeSettings()

  if (isLoading) {
    return (
      <div>
        <h2 className="text-[11px] font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-[0.12em] mb-3 px-1">
          Claude Code
        </h2>
        <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-8 flex items-center justify-center">
          <Loader2 className="w-5 h-5 animate-spin text-gray-400" />
        </div>
      </div>
    )
  }

  if (error || !settings) {
    return (
      <div>
        <h2 className="text-[11px] font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-[0.12em] mb-3 px-1">
          Claude Code
        </h2>
        <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Unable to load Claude Code settings.
          </p>
        </div>
      </div>
    )
  }

  return (
    <div>
      <h2 className="text-[11px] font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-[0.12em] mb-3 px-1">
        Claude Code
      </h2>
      <div className="space-y-3">
        <EnvironmentSection settings={settings} />
        <PermissionsSection settings={settings} />
        <HooksSection settings={settings} />
        <PluginsSection settings={settings} />
        <MiscSection settings={settings} />
      </div>
    </div>
  )
}
