import { Check, Lock, Shield, ShieldCheck } from 'lucide-react'
import { cn } from '../lib/utils'

interface TelemetrySectionProps {
  telemetryStatus: 'undecided' | 'enabled' | 'disabled'
  hasPosHogKey: boolean
  onEnable: () => void
  onDisable: () => void
}

/** Accessible toggle switch (no dependency on a Switch UI primitive). */
function Toggle({
  checked,
  onChange,
  disabled,
}: {
  checked: boolean
  onChange: (v: boolean) => void
  disabled?: boolean
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        'relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2',
        'disabled:cursor-not-allowed disabled:opacity-50',
        checked ? 'bg-green-500 dark:bg-green-600' : 'bg-gray-300 dark:bg-gray-600',
      )}
    >
      <span
        className={cn(
          'pointer-events-none inline-flex h-5 w-5 transform items-center justify-center rounded-full bg-white shadow-sm ring-0 transition-transform duration-200 ease-in-out',
          checked ? 'translate-x-5' : 'translate-x-0',
        )}
      >
        {checked && <Check className="h-3 w-3 text-green-600" />}
      </span>
    </button>
  )
}

export function TelemetrySection({
  telemetryStatus,
  hasPosHogKey,
  onEnable,
  onDisable,
}: TelemetrySectionProps) {
  const isEnabled = telemetryStatus === 'enabled'
  const isSelfHosted = !hasPosHogKey

  // Self-hosted: no PostHog key compiled in — analytics are structurally impossible
  if (isSelfHosted) {
    return (
      <div className="space-y-3">
        <div className="flex items-start gap-3">
          <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-green-100 dark:bg-green-900/30">
            <Lock className="h-4 w-4 text-green-600 dark:text-green-400" />
          </div>
          <div>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">
              Fully private — no data leaves your machine
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
              This is a local build with no analytics endpoint configured. All session data stays on
              your device.
            </p>
          </div>
        </div>
      </div>
    )
  }

  // Cloud build: user can opt in/out
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-start gap-3">
          <div
            className={cn(
              'mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-full',
              isEnabled ? 'bg-blue-100 dark:bg-blue-900/30' : 'bg-gray-100 dark:bg-gray-800',
            )}
          >
            {isEnabled ? (
              <ShieldCheck className="h-4 w-4 text-blue-600 dark:text-blue-400" />
            ) : (
              <Shield className="h-4 w-4 text-gray-500 dark:text-gray-400" />
            )}
          </div>
          <div>
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100">
              Anonymous Usage Analytics
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
              {isEnabled
                ? 'Helping improve Claude View with anonymous usage patterns.'
                : 'Help improve Claude View by sharing anonymous usage data.'}
            </p>
          </div>
        </div>
        <Toggle checked={isEnabled} onChange={(v) => (v ? onEnable() : onDisable())} />
      </div>

      {/* What's collected — only show when enabled or undecided */}
      {(isEnabled || telemetryStatus === 'undecided') && (
        <div className="ml-11 text-xs text-gray-500 dark:text-gray-400 space-y-1">
          <p className="font-medium text-gray-600 dark:text-gray-300">
            {isEnabled ? 'What we collect:' : 'What would be collected:'}
          </p>
          <ul className="list-disc list-inside space-y-0.5">
            <li>Feature usage (which pages and actions are used)</li>
            <li>Performance metrics (load times, error rates)</li>
          </ul>
          <p className="mt-1.5 text-gray-400 dark:text-gray-500">
            No session content, code, or personal data is ever sent.
          </p>
        </div>
      )}
    </div>
  )
}
