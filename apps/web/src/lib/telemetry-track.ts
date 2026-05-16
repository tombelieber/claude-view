import type { ActionId } from '@/types/generated/ActionId'
import type { Surface } from '@/types/generated/Surface'

/**
 * Web journey telemetry. Events POST to the Rust server
 * (`/api/telemetry/event`), NOT posthog-js directly, so (a) ad-blockers on
 * the PostHog domain can't blind us and (b) the closed-enum privacy
 * boundary is enforced server-side — a path/prompt can never be sent
 * because the only callable shapes are these two typed functions.
 *
 * Strictly fire-and-forget: telemetry must never throw, reject, block, or
 * otherwise affect the UX. The server decides (from resolved consent)
 * whether to forward; the client never gates on telemetry state.
 */
function post(body: Record<string, string>): void {
  void fetch('/api/telemetry/event', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  }).catch(() => {
    // Swallowed by design — a failed analytics ping is a non-event.
  })
}

/** Record navigation to a product surface (drives "which features used" + journey paths). */
export function trackFeatureOpened(surface: Surface): void {
  post({ event: 'feature_opened', surface })
}

/** Record a high-intent action (the depth / monetization signal). */
export function trackFeatureAction(action: ActionId): void {
  post({ event: 'feature_action', action })
}
