import * as Dialog from '@radix-ui/react-dialog'
import { DialogContent, DialogOverlay } from './ui/CenteredDialog'

interface TelemetryBannerProps {
  onEnable: () => void
  onDisable: () => void
  isPending?: boolean
}

export function TelemetryBanner({ onEnable, onDisable, isPending }: TelemetryBannerProps) {
  return (
    <Dialog.Root open>
      <Dialog.Portal>
        <DialogOverlay className="bg-black/50" />
        <DialogContent
          className="max-w-sm rounded-xl border border-border bg-background p-6 shadow-2xl"
          onInteractOutside={(e) => e.preventDefault()}
          onEscapeKeyDown={(e) => e.preventDefault()}
        >
          <Dialog.Title className="text-base font-semibold text-foreground">
            Help shape claude-view
          </Dialog.Title>
          <Dialog.Description className="mt-2 text-sm text-muted-foreground leading-relaxed">
            You're one of 2,000+ daily users. Enable anonymous analytics so we can build what
            matters most to you.
          </Dialog.Description>
          <ul className="mt-3 space-y-1 text-xs text-muted-foreground">
            <li>No session content, messages, or code — ever</li>
            <li>Only feature usage counts and page views</li>
            <li>Toggle off anytime in Settings</li>
          </ul>
          <a
            href="https://claudeview.ai/telemetry"
            className="mt-2 inline-block text-xs text-muted-foreground underline"
            target="_blank"
            rel="noopener noreferrer"
          >
            Learn more
          </a>
          <div className="mt-5 flex gap-3">
            <button
              type="button"
              className="flex-1 rounded-lg border border-border px-4 py-2 text-sm text-muted-foreground hover:bg-accent disabled:opacity-50"
              onClick={onDisable}
              disabled={isPending}
            >
              No thanks
            </button>
            <button
              type="button"
              className="flex-1 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
              onClick={onEnable}
              disabled={isPending}
            >
              {isPending ? 'Saving…' : 'Enable analytics'}
            </button>
          </div>
        </DialogContent>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
