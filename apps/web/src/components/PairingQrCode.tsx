import { QRCodeSVG } from 'qrcode.react'
import { useCallback, useEffect, useState } from 'react'

interface QrPayload {
  url: string
  r: string
  k: string
  t: string
  s: string
  v: number
}

/**
 * PairingQrCode — displays a QR code for mobile pairing.
 *
 * Calls GET /pairing/qr on the Rust server to obtain a one-time QR payload,
 * then renders the URL as a scannable QR code. The phone scans this to
 * initiate the NaCl + HMAC pairing flow.
 */
export function PairingQrCode() {
  const [payload, setPayload] = useState<QrPayload | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  const fetchQr = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const res = await fetch('/pairing/qr')
      if (!res.ok) {
        if (res.status === 503) {
          throw new Error('Relay server not configured. Set RELAY_URL to enable mobile pairing.')
        }
        throw new Error(`Failed to generate QR code (${res.status})`)
      }
      const data: QrPayload = await res.json()
      setPayload(data)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Unknown error')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchQr()
  }, [fetchQr])

  if (loading) {
    return (
      <div className="flex flex-col items-center gap-3 p-6">
        <div className="h-48 w-48 animate-pulse rounded-lg bg-zinc-200 dark:bg-zinc-700" />
        <p className="text-sm text-zinc-500 dark:text-zinc-400">Generating pairing code...</p>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex flex-col items-center gap-3 p-6">
        <div className="flex h-48 w-48 items-center justify-center rounded-lg border border-red-200 bg-red-50 dark:border-red-800 dark:bg-red-950">
          <p className="px-4 text-center text-sm text-red-600 dark:text-red-400">{error}</p>
        </div>
        <button
          type="button"
          onClick={fetchQr}
          className="rounded-md bg-zinc-800 px-3 py-1.5 text-sm text-white hover:bg-zinc-700 dark:bg-zinc-200 dark:text-zinc-900 dark:hover:bg-zinc-300"
        >
          Retry
        </button>
      </div>
    )
  }

  if (!payload) return null

  return (
    <div className="flex flex-col items-center gap-3 p-6">
      <div className="rounded-lg bg-white p-3">
        <QRCodeSVG value={payload.url} size={192} level="M" />
      </div>
      <p className="text-sm text-zinc-500 dark:text-zinc-400">
        Scan with the Claude View mobile app to pair
      </p>
      <button
        type="button"
        onClick={fetchQr}
        className="rounded-md border border-zinc-300 px-3 py-1.5 text-sm text-zinc-700 hover:bg-zinc-100 dark:border-zinc-600 dark:text-zinc-300 dark:hover:bg-zinc-800"
      >
        Generate new code
      </button>
    </div>
  )
}
