import { useRelayConnection } from '@claude-view/shared'
import { secureStoreAdapter } from '../lib/secure-store-adapter'

export function useRelaySessions() {
  return useRelayConnection({ storage: secureStoreAdapter })
}
