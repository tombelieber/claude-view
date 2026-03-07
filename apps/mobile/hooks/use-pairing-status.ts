import { useCallback, useEffect, useState } from 'react'
import { secureStoreAdapter } from '../lib/secure-store-adapter'

export function usePairingStatus() {
  const [isPaired, setIsPaired] = useState<boolean | null>(null)

  const check = useCallback(async () => {
    const url = await secureStoreAdapter.getItem('relay_url')
    setIsPaired(url !== null)
  }, [])

  useEffect(() => {
    check()
  }, [check])

  return { isPaired, refresh: check }
}
