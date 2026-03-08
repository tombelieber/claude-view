import { useCallback, useEffect, useRef, useState } from 'react'
import { decodeBase64 } from 'tweetnacl-util'
import { type PhoneKeys, decryptFromDevice, loadPhoneKeys, signAuthChallenge } from '../crypto/nacl'
import type { KeyStorage } from '../crypto/storage'
import type { LiveSession } from '../types/generated'

// Audit gap #6: Added 'crypto_error' state for key mismatch detection
export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'crypto_error'

export interface UseRelayConnectionOptions {
  storage: KeyStorage
}

export interface UseRelayConnectionResult {
  sessions: Record<string, LiveSession>
  connectionState: ConnectionState
  disconnect: () => void
}

export function useRelayConnection(opts: UseRelayConnectionOptions): UseRelayConnectionResult {
  const { storage } = opts
  const [sessions, setSessions] = useState<Record<string, LiveSession>>({})
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected')
  const wsRef = useRef<WebSocket | null>(null)
  const keysRef = useRef<PhoneKeys | null>(null)
  const macPubkeyRef = useRef<Uint8Array | null>(null)
  // Audit gap #7: Keep stale sessions during reconnect (Slack desktop pattern)
  const staleSessions = useRef<Record<string, LiveSession>>({})
  // Audit gap #6: Track consecutive decrypt failures
  const decryptFailures = useRef(0)
  const DECRYPT_FAILURE_THRESHOLD = 3

  const disconnect = useCallback(() => {
    wsRef.current?.close()
    wsRef.current = null
    setConnectionState('disconnected')
  }, [])

  // Audit gap #16: Stabilize storage reference to prevent infinite reconnect loops.
  useEffect(() => {
    let cancelled = false
    let reconnectTimer: ReturnType<typeof setTimeout>
    let reconnectAttempt = 0

    async function connect() {
      const keys = await loadPhoneKeys(storage)
      if (!keys || cancelled) return
      keysRef.current = keys

      const relayUrl = await storage.getItem('relay_url')
      const macPubkeyB64 = await storage.getItem('mac_x25519_pubkey')
      if (!relayUrl || !macPubkeyB64 || cancelled) return
      macPubkeyRef.current = decodeBase64(macPubkeyB64)

      setConnectionState('connecting')
      const ws = new WebSocket(relayUrl)
      wsRef.current = ws

      ws.onopen = () => {
        if (cancelled || wsRef.current !== ws) return
        reconnectAttempt = 0
        decryptFailures.current = 0
        const { timestamp, signature } = signAuthChallenge(
          keys.deviceId,
          keys.signingKeyPair.secretKey,
        )
        ws.send(
          JSON.stringify({
            type: 'auth',
            device_id: keys.deviceId,
            timestamp,
            signature,
          }),
        )
      }

      ws.onmessage = (event) => {
        if (cancelled || wsRef.current !== ws) return
        try {
          const raw = typeof event.data === 'string' ? event.data : String(event.data)
          const data = JSON.parse(raw)

          if (data.type === 'auth_ok') {
            setConnectionState('connected')
            return
          }
          // Audit gap #23: Relay sends {"error":"..."}, NOT {"type":"error","message":"..."}.
          if (data.error) {
            console.error('Relay auth error:', data.error)
            ws.close()
            return
          }
          // Audit gap #24: Relay notifies phone when Mac disconnects
          if (data.type === 'mac_offline') {
            setConnectionState('disconnected')
            return
          }

          // Encrypted message from Mac
          if (data.payload && macPubkeyRef.current && keysRef.current) {
            const decrypted = decryptFromDevice(
              data.payload,
              macPubkeyRef.current,
              keysRef.current.boxKeyPair.secretKey,
            )
            // Audit gap #6: Track decrypt failures
            if (!decrypted) {
              decryptFailures.current++
              if (decryptFailures.current >= DECRYPT_FAILURE_THRESHOLD) {
                setConnectionState('crypto_error')
              }
              return
            }
            decryptFailures.current = 0

            const text = new TextDecoder().decode(decrypted)
            const msg = JSON.parse(text)

            // Mac sends individual LiveSession objects (camelCase), not batched envelopes
            if (msg.type === 'session_completed' && msg.sessionId) {
              setSessions((prev) => {
                const next = { ...prev }
                delete next[msg.sessionId]
                return next
              })
              return
            }
            if (msg.id && msg.project) {
              setSessions((prev) => ({ ...prev, [msg.id]: msg as LiveSession }))
            }
          }
        } catch (e) {
          console.warn('Relay message parse error:', e)
        }
      }

      ws.onclose = () => {
        if (cancelled) return
        setConnectionState('disconnected')
        // Audit gap #7: Keep stale sessions during reconnect
        setSessions((prev) => {
          staleSessions.current = prev
          return prev
        })
        // Exponential backoff: 1s, 2s, 4s, 8s, ..., max 30s, with jitter
        const baseDelay = Math.min(1000 * 2 ** reconnectAttempt, 30000)
        const jitter = Math.random() * 1000
        reconnectAttempt++
        reconnectTimer = setTimeout(connect, baseDelay + jitter)
      }

      ws.onerror = () => {
        ws.close()
      }
    }

    connect()

    return () => {
      cancelled = true
      clearTimeout(reconnectTimer)
      wsRef.current?.close()
      wsRef.current = null
    }
  }, [storage])

  return { sessions, connectionState, disconnect }
}
