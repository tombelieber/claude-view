import * as Popover from '@radix-ui/react-popover'
import type { DockviewApi, SerializedDockview } from 'dockview-react'
import { ChevronDown, Maximize2, Save, Trash2 } from 'lucide-react'
import { type KeyboardEvent, useCallback, useState } from 'react'
import { cn } from '../../lib/utils'
import type { DisplayMode } from '../../store/monitor-store'
import type { LiveSession } from './use-live-sessions'

export interface LayoutPresetsProps {
  sessions: LiveSession[]
  dockviewApi: DockviewApi | null
  activePreset: string | null
  onSelectPreset: (presetName: string) => void
  onSavePreset: (name: string) => void
  onDeletePreset: (name: string) => void
  customPresets: Record<string, SerializedDockview>
  displayMode: DisplayMode
}

// ---------------------------------------------------------------------------
// Preset definitions
// ---------------------------------------------------------------------------

const BUILT_IN_PRESETS = ['2x2', '3+1', 'focus'] as const
type BuiltInPreset = (typeof BUILT_IN_PRESETS)[number]

const PRESET_CAPACITY: Record<string, number> = {
  '2x2': 4,
  '3+1': 4,
  focus: Number.POSITIVE_INFINITY,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function sessionTitle(id: string, sessions: LiveSession[]): string {
  const session = sessions.find((s) => s.id === id)
  return session?.projectDisplayName ?? id.slice(0, 8)
}

function panelParams(id: string, sessions: LiveSession[], displayMode: DisplayMode) {
  const s = sessions.find((sess) => sess.id === id)
  return { sessionId: id, displayMode, status: s?.status ?? 'done' }
}

// ---------------------------------------------------------------------------
// applyPreset — programmatically builds a dockview layout from sessions
// ---------------------------------------------------------------------------

export function applyPreset(
  api: DockviewApi,
  presetName: string,
  sessions: LiveSession[],
  displayMode: DisplayMode,
) {
  if (sessions.length === 0) return

  const sessionIds = sessions.map((s) => s.id)
  const capacity = PRESET_CAPACITY[presetName] ?? 4
  const positioned = sessionIds.slice(0, capacity)
  const overflow = sessionIds.slice(capacity)

  api.clear()

  switch (presetName) {
    case '2x2': {
      for (const [i, id] of positioned.entries()) {
        api.addPanel({
          id,
          component: 'session',
          title: sessionTitle(id, sessions),
          params: panelParams(id, sessions, displayMode),
          position:
            i === 0
              ? undefined
              : i === 1
                ? { referencePanel: positioned[0], direction: 'right' as const }
                : i === 2
                  ? { referencePanel: positioned[0], direction: 'below' as const }
                  : { referencePanel: positioned[1], direction: 'below' as const },
        })
      }
      break
    }
    case '3+1': {
      // Right (large) panel first
      api.addPanel({
        id: positioned[0],
        component: 'session',
        title: sessionTitle(positioned[0], sessions),
        params: panelParams(positioned[0], sessions, displayMode),
      })
      // Left stack
      for (let i = 1; i < positioned.length; i++) {
        api.addPanel({
          id: positioned[i],
          component: 'session',
          title: sessionTitle(positioned[i], sessions),
          params: panelParams(positioned[i], sessions, displayMode),
          position:
            i === 1
              ? { referencePanel: positioned[0], direction: 'left' as const }
              : { referencePanel: positioned[i - 1], direction: 'below' as const },
        })
      }
      break
    }
    case 'focus': {
      api.addPanel({
        id: sessionIds[0],
        component: 'session',
        title: sessionTitle(sessionIds[0], sessions),
        params: panelParams(sessionIds[0], sessions, displayMode),
      })
      for (let i = 1; i < sessionIds.length; i++) {
        api.addPanel({
          id: sessionIds[i],
          component: 'session',
          title: sessionTitle(sessionIds[i], sessions),
          params: panelParams(sessionIds[i], sessions, displayMode),
          position: { referencePanel: sessionIds[0] },
        })
      }
      break
    }
  }

  // Overflow: extra sessions beyond preset capacity added as tabs in last group
  if (overflow.length > 0 && positioned.length > 0) {
    const lastPositionedId = positioned[positioned.length - 1]
    for (const id of overflow) {
      api.addPanel({
        id,
        component: 'session',
        title: sessionTitle(id, sessions),
        params: panelParams(id, sessions, displayMode),
        position: { referencePanel: lastPositionedId },
      })
    }
  }
}

// ---------------------------------------------------------------------------
// Small layout icon SVGs
// ---------------------------------------------------------------------------

function Icon2x2({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      className={cn('w-3.5 h-3.5', className)}
      fill="currentColor"
      role="img"
      aria-label="2x2 grid layout"
    >
      <title>2x2 grid layout</title>
      <rect x="1" y="1" width="6" height="6" rx="1" opacity={0.7} />
      <rect x="9" y="1" width="6" height="6" rx="1" opacity={0.7} />
      <rect x="1" y="9" width="6" height="6" rx="1" opacity={0.7} />
      <rect x="9" y="9" width="6" height="6" rx="1" opacity={0.7} />
    </svg>
  )
}

function Icon3Plus1({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      className={cn('w-3.5 h-3.5', className)}
      fill="currentColor"
      role="img"
      aria-label="3+1 layout"
    >
      <title>3+1 layout</title>
      <rect x="1" y="1" width="4" height="4" rx="0.5" opacity={0.7} />
      <rect x="1" y="6" width="4" height="4" rx="0.5" opacity={0.7} />
      <rect x="1" y="11" width="4" height="4" rx="0.5" opacity={0.7} />
      <rect x="7" y="1" width="8" height="14" rx="1" opacity={0.7} />
    </svg>
  )
}

const PRESET_META: Record<
  BuiltInPreset,
  { label: string; icon: React.FC<{ className?: string }> }
> = {
  '2x2': { label: '2x2', icon: Icon2x2 },
  '3+1': { label: '3+1', icon: Icon3Plus1 },
  focus: {
    label: 'Focus',
    icon: ({ className }: { className?: string }) => (
      <Maximize2 className={cn('w-3.5 h-3.5', className)} />
    ),
  },
}

// ---------------------------------------------------------------------------
// LayoutPresets component
// ---------------------------------------------------------------------------

export function LayoutPresets({
  sessions,
  dockviewApi,
  activePreset,
  onSelectPreset,
  onSavePreset,
  onDeletePreset,
  customPresets,
  displayMode,
}: LayoutPresetsProps) {
  const [open, setOpen] = useState(false)
  const [saving, setSaving] = useState(false)
  const [saveName, setSaveName] = useState('')

  const customPresetNames = Object.keys(customPresets)
  const hasCustomPresets = customPresetNames.length > 0

  const handleSelectBuiltIn = useCallback(
    (name: BuiltInPreset) => {
      if (!dockviewApi) return
      applyPreset(dockviewApi, name, sessions, displayMode)
      onSelectPreset(name)
      setOpen(false)
    },
    [dockviewApi, sessions, displayMode, onSelectPreset],
  )

  const handleSelectCustom = useCallback(
    (name: string) => {
      if (!dockviewApi) return
      const layout = customPresets[name]
      if (layout) {
        dockviewApi.fromJSON(layout)
      }
      onSelectPreset(name)
      setOpen(false)
    },
    [dockviewApi, customPresets, onSelectPreset],
  )

  const handleSave = useCallback(() => {
    const trimmed = saveName.trim()
    if (trimmed.length === 0) return
    onSavePreset(trimmed)
    setSaveName('')
    setSaving(false)
  }, [saveName, onSavePreset])

  const handleSaveKeyDown = useCallback(
    (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Enter') {
        e.preventDefault()
        handleSave()
      } else if (e.key === 'Escape') {
        e.preventDefault()
        setSaving(false)
        setSaveName('')
      }
    },
    [handleSave],
  )

  const triggerLabel = activePreset ?? 'Presets'

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          className="flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors duration-150 cursor-pointer text-gray-600 dark:text-gray-300"
        >
          {triggerLabel}
          <ChevronDown className="w-3 h-3" />
        </button>
      </Popover.Trigger>

      <Popover.Portal>
        <Popover.Content
          align="start"
          sideOffset={6}
          className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg p-1 min-w-[200px] z-50"
        >
          {/* Built-in presets */}
          <div className="px-2 py-1 text-xs uppercase tracking-wider text-gray-400 dark:text-gray-500 font-medium">
            Built-in
          </div>
          {BUILT_IN_PRESETS.map((name) => {
            const meta = PRESET_META[name]
            const Icon = meta.icon
            const isActive = activePreset === name
            return (
              <button
                key={name}
                type="button"
                onClick={() => handleSelectBuiltIn(name)}
                className={cn(
                  'flex items-center gap-2 px-2 py-1.5 rounded-md text-xs cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800 w-full text-left transition-colors duration-100',
                  isActive && 'bg-indigo-500/10 text-indigo-400',
                )}
              >
                <Icon
                  className={isActive ? 'text-indigo-400' : 'text-gray-500 dark:text-gray-400'}
                />
                <span>{meta.label}</span>
              </button>
            )
          })}

          {/* Custom presets */}
          {hasCustomPresets && (
            <>
              <div className="h-px bg-gray-200 dark:bg-gray-700 my-1" />
              <div className="px-2 py-1 text-xs uppercase tracking-wider text-gray-400 dark:text-gray-500 font-medium">
                Saved
              </div>
              {customPresetNames.map((name) => {
                const isActive = activePreset === name
                return (
                  <div
                    key={name}
                    className={cn(
                      'flex items-center gap-2 px-2 py-1.5 rounded-md text-xs cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800 group',
                      isActive && 'bg-indigo-500/10 text-indigo-400',
                    )}
                  >
                    <button
                      type="button"
                      onClick={() => handleSelectCustom(name)}
                      className="flex-1 text-left truncate"
                    >
                      {name}
                    </button>
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation()
                        onDeletePreset(name)
                      }}
                      className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-red-100 dark:hover:bg-red-900/30 text-gray-400 hover:text-red-500 transition-opacity duration-100"
                      aria-label={`Delete preset ${name}`}
                    >
                      <Trash2 className="w-3 h-3" />
                    </button>
                  </div>
                )
              })}
            </>
          )}

          {/* Save current layout */}
          <div className="h-px bg-gray-200 dark:bg-gray-700 my-1" />
          {saving ? (
            <div className="flex items-center gap-1 px-2 py-1">
              <input
                ref={(el) => el?.focus()}
                type="text"
                value={saveName}
                onChange={(e) => setSaveName(e.target.value)}
                onKeyDown={handleSaveKeyDown}
                placeholder="Preset name..."
                className="flex-1 px-1.5 py-1 rounded text-xs bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 text-gray-800 dark:text-gray-200 placeholder:text-gray-400 outline-none focus:ring-1 focus:ring-indigo-500"
              />
              <button
                type="button"
                onClick={handleSave}
                disabled={saveName.trim().length === 0}
                className="px-1.5 py-1 rounded text-xs font-medium bg-indigo-500 text-white hover:bg-indigo-600 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer transition-colors duration-100"
              >
                Save
              </button>
            </div>
          ) : (
            <button
              type="button"
              onClick={() => setSaving(true)}
              className="flex items-center gap-2 px-2 py-1.5 rounded-md text-xs cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800 w-full text-left text-gray-600 dark:text-gray-300 transition-colors duration-100"
            >
              <Save className="w-3.5 h-3.5 text-gray-500 dark:text-gray-400" />
              <span>Save Current Layout</span>
            </button>
          )}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
