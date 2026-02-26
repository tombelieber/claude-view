import * as Popover from '@radix-ui/react-popover'
import { BellRing, BellOff, Play } from 'lucide-react'
import type { NotificationSoundSettings, SoundPreset } from '../../hooks/use-notification-sound'
import { cn } from '../../lib/utils'

interface NotificationSoundPopoverProps {
  settings: NotificationSoundSettings
  onSettingsChange: (patch: Partial<NotificationSoundSettings>) => void
  onPreview: () => void
  audioUnlocked: boolean
}

const SOUND_PRESETS: { value: SoundPreset; label: string }[] = [
  { value: 'ding', label: '\u266A Ding' },
  { value: 'chime', label: '\u266A Chime' },
  { value: 'bell', label: '\u266A Bell' },
]

export function NotificationSoundPopover({
  settings,
  onSettingsChange,
  onPreview,
  audioUnlocked,
}: NotificationSoundPopoverProps) {
  const isEnabled = settings.enabled

  return (
    <Popover.Root>
      <Popover.Trigger asChild>
        <button
          type="button"
          className="p-1.5 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors duration-150 cursor-pointer"
          aria-label="Notification sound settings"
        >
          {isEnabled ? (
            <BellRing className="w-5 h-5 text-gray-600 dark:text-gray-300" />
          ) : (
            <BellOff className="w-5 h-5 text-gray-400 dark:text-gray-500" />
          )}
        </button>
      </Popover.Trigger>

      <Popover.Portal>
        <Popover.Content
          align="end"
          sideOffset={8}
          className="w-64 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 shadow-lg p-4 z-50"
        >
          {/* Header */}
          <h3 className="text-sm font-semibold text-gray-800 dark:text-gray-100 mb-3">
            Notification Sound
          </h3>

          {/* Toggle row */}
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm text-gray-600 dark:text-gray-300">Sound</span>
            <button
              type="button"
              role="switch"
              aria-checked={isEnabled}
              onClick={() => onSettingsChange({ enabled: !isEnabled })}
              className={cn(
                'relative w-9 h-5 rounded-full transition-colors duration-150 cursor-pointer',
                isEnabled ? 'bg-indigo-500' : 'bg-gray-300 dark:bg-gray-600'
              )}
            >
              <span
                className={cn(
                  'absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white transition-transform duration-150',
                  isEnabled && 'translate-x-4'
                )}
              />
            </button>
          </div>

          {/* Volume row */}
          <div className="flex items-center gap-2 mb-3">
            <span className="text-sm text-gray-600 dark:text-gray-300 shrink-0">Volume</span>
            <input
              type="range"
              min={0}
              max={100}
              step={1}
              value={Math.round(settings.volume * 100)}
              onChange={(e) => onSettingsChange({ volume: parseInt(e.target.value, 10) / 100 })}
              disabled={!isEnabled}
              className={cn(
                'w-full h-1 rounded-full appearance-none accent-indigo-500 cursor-pointer',
                '[&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:bg-indigo-500',
                !isEnabled && 'opacity-40 cursor-not-allowed'
              )}
            />
            <span className="text-xs font-mono tabular-nums text-gray-500 dark:text-gray-400 w-8 text-right">
              {Math.round(settings.volume * 100)}%
            </span>
          </div>

          {/* Sound picker */}
          <div className="flex gap-1.5 mb-3">
            {SOUND_PRESETS.map((preset) => (
              <button
                key={preset.value}
                type="button"
                disabled={!isEnabled}
                onClick={() => onSettingsChange({ sound: preset.value })}
                className={cn(
                  'flex-1 py-1.5 rounded-md text-xs font-medium transition-colors duration-150 cursor-pointer',
                  'border',
                  settings.sound === preset.value
                    ? 'ring-2 ring-indigo-500 bg-indigo-50 dark:bg-indigo-900/30 border-indigo-300 dark:border-indigo-700 text-indigo-700 dark:text-indigo-300'
                    : 'border-gray-200 dark:border-gray-600 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700',
                  !isEnabled && 'opacity-40 cursor-not-allowed'
                )}
              >
                {preset.label}
              </button>
            ))}
          </div>

          {/* Preview button */}
          <button
            type="button"
            onClick={onPreview}
            className="flex items-center justify-center gap-2 w-full py-1.5 rounded-md text-sm bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 text-gray-700 dark:text-gray-200 transition-colors duration-150 cursor-pointer"
          >
            <Play className="w-3.5 h-3.5" />
            Preview
          </button>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
