/**
 * HookEventItem type — used by HookEventRow for rendering hook events
 * in both the action log timeline and inline within MessageTyped.
 */
export interface HookEventItem {
  id: string
  timestamp: number
  type: 'hook_event'
  eventName: string
  toolName?: string
  label: string
  group: 'autonomous' | 'needs_you' | 'delivered'
  context?: string
}
