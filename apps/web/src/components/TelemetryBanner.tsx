interface TelemetryBannerProps {
  onEnable: () => void
  onDisable: () => void
}

export function TelemetryBanner({ onEnable, onDisable }: TelemetryBannerProps) {
  return (
    <div className="bg-muted border-b px-4 py-2 flex items-center justify-between">
      <span className="text-sm text-muted-foreground">
        Help improve claude-view by sending anonymous usage data.{' '}
        <a
          href="https://claudeview.ai/telemetry"
          className="underline"
          target="_blank"
          rel="noopener noreferrer"
        >
          Learn more
        </a>
      </span>
      <div className="flex gap-2">
        <button
          type="button"
          className="text-sm px-3 py-1 rounded border border-border hover:bg-accent"
          onClick={onDisable}
        >
          No thanks
        </button>
        <button
          type="button"
          className="text-sm px-3 py-1 rounded bg-primary text-primary-foreground hover:bg-primary/90"
          onClick={onEnable}
        >
          Enable
        </button>
      </div>
    </div>
  )
}
