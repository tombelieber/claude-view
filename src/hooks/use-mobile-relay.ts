import { useEffect, useRef, useState } from 'react'
import { decryptMessage, getMacPublicKey, signAuthChallenge } from '../lib/mobile-crypto.ts'
import { getItem } from '../lib/mobile-storage.ts'
import type { LiveSession } from '../components/live/use-live-sessions.ts'

interface UseMobileRelayResult {
  sessions: Map<string, LiveSession>
  isConnected: boolean
  error: string | null
}

export function useMobileRelay(relayUrl: string | null): UseMobileRelayResult {
  const [sessions, setSessions] = useState<Map<string, LiveSession>>(new Map())
  const [isConnected, setIsConnected] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const wsRef = useRef<WebSocket | null>(null)

  useEffect(() => {
    if (!relayUrl) return

    let cancelled = false

    async function connect() {
      const deviceId = await getItem('device_id')
      const macPubKey = await getMacPublicKey()
      if (!deviceId || !macPubKey) {
        setError('Not paired')
        return
      }

      const ws = new WebSocket(relayUrl!)
      wsRef.current = ws

      ws.onopen = async () => {
        if (wsRef.current !== ws) return // Stale guard

        // Send auth
        const auth = await signAuthChallenge(deviceId)
        if (!auth) { ws.close(); return }
        ws.send(JSON.stringify({
          type: 'auth',
          device_id: deviceId,
          timestamp: auth.timestamp,
          signature: auth.signature,
        }))
      }

      ws.onmessage = async (event: MessageEvent) => {
        if (wsRef.current !== ws) return // Stale guard

        const data = JSON.parse(event.data as string)

        if (data.type === 'auth_ok') {
          setIsConnected(true)
          setError(null)
          return
        }

        if (data.error) {
          setError(data.error as string)
          return
        }

        // Decrypt payload
        if (data.payload) {
          const json = await decryptMessage(data.payload as string, macPubKey!)
          if (!json) return

          const parsed = JSON.parse(json)

          if (parsed.type === 'session_completed') {
            setSessions((prev) => {
              const next = new Map(prev)
              next.delete(parsed.session_id as string)
              return next
            })
          } else {
            // LiveSession update
            const session = parsed as LiveSession
            setSessions((prev) => {
              const next = new Map(prev)
              next.set(session.id, session)
              return next
            })
          }
        }
      }

      ws.onclose = () => {
        if (wsRef.current === ws) {
          setIsConnected(false)
          if (!cancelled) {
            // Reconnect with backoff
            setTimeout(() => { void connect() }, 3000)
          }
        }
      }

      ws.onerror = () => {
        if (wsRef.current !== ws) return // Stale guard
        setError('Connection failed')
      }
    }

    void connect()

    return () => {
      cancelled = true
      wsRef.current?.close()
      wsRef.current = null
    }
  }, [relayUrl])

  return { sessions, isConnected, error }
}
