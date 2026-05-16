import type { Surface } from '@/types/generated/Surface'

/**
 * Map a router pathname to a closed {@link Surface} variant — or `null`
 * when the route is unknown.
 *
 * Pure and total. The `null` case is deliberate: an unrecognised path
 * emits NOTHING rather than leaking a raw URL into telemetry (trust >
 * coverage). Keep the cases in lock-step with `router.tsx`.
 */
export function surfaceForPath(pathname: string): Surface | null {
  const trimmed = pathname.replace(/\/+$/, '')
  if (trimmed === '' || trimmed === '/') return 'live_monitor'
  const seg = trimmed.split('/').filter(Boolean)
  switch (seg[0]) {
    case 'chat':
      return 'chat'
    case 'sessions':
      return seg.length > 1 ? 'session_detail' : 'history'
    case 'analytics':
      return 'analytics'
    case 'activity':
      return 'activity'
    case 'reports':
      return 'reports'
    case 'prompts':
      return 'prompts'
    case 'teams':
      return 'teams'
    case 'workflows':
      return 'workflows'
    case 'plugins':
      return 'plugins'
    case 'memory':
      return 'memory'
    case 'monitor':
      return 'system_monitor'
    case 'settings':
      return 'settings'
    case 'search':
      return 'search'
    case 'insights':
      return 'insights'
    default:
      return null
  }
}
