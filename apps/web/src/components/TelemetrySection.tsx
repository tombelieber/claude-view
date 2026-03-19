interface TelemetrySectionProps {
  telemetryStatus: 'undecided' | 'enabled' | 'disabled'
  hasPosHogKey: boolean
  onEnable: () => void
  onDisable: () => void
}

export function TelemetrySection({
  telemetryStatus,
  hasPosHogKey,
  onEnable,
  onDisable,
}: TelemetrySectionProps) {
  const isEnabled = telemetryStatus === 'enabled'
  const isSelfHosted = !hasPosHogKey

  return (
    <div className="flex items-center justify-between py-2">
      <div>
        <p className="text-sm font-medium">Anonymous Usage Analytics</p>
        <p className="text-xs text-muted-foreground">
          {isSelfHosted
            ? 'Analytics not available in self-hosted builds'
            : 'Help improve claude-view by sending anonymous usage data'}
        </p>
      </div>
      <input
        type="checkbox"
        checked={isEnabled}
        disabled={isSelfHosted}
        onChange={(e) => (e.target.checked ? onEnable() : onDisable())}
        className="h-4 w-4 rounded border-border"
      />
    </div>
  )
}
