import { CheckCircle2, Loader2, RefreshCw, XCircle } from 'lucide-react'
import { QRCodeSVG } from 'qrcode.react'
import { useEffect, useState } from 'react'
import type { PairingState } from '../hooks/use-pairing-flow'

interface PairingQrCodeProps {
  state: PairingState
  onStart: () => void
  onReset: () => void
}

/**
 * Build the `claude-view-pair://` deep link encoded in the QR code. The
 * mobile app scans this URL and extracts the token, then calls the
 * `pair-claim` Edge Function to complete pairing.
 */
function buildPairingUrl(token: string): string {
  return `claude-view-pair://${token}`
}

/**
 * Pure presentational component: renders the current pairing-flow state.
 *
 * - `idle`       → "Pair a new device" button
 * - `creating`   → spinner + "Generating code…"
 * - `showing-qr` → QR code + countdown + cancel
 * - `success`    → green checkmark + "Device paired"
 * - `expired`    → "Code expired — try again"
 * - `error`      → plain-language error + retry button
 */
export function PairingQrCode({ state, onStart, onReset }: PairingQrCodeProps) {
  if (state.kind === 'idle') {
    return (
      <button
        type="button"
        onClick={onStart}
        className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 hover:bg-gray-800 dark:hover:bg-gray-200 transition-colors"
      >
        Pair a new device
      </button>
    )
  }

  if (state.kind === 'creating') {
    return (
      <div className="flex flex-col items-center gap-3 p-6" aria-live="polite">
        <div className="h-48 w-48 animate-pulse rounded-lg bg-gray-100 dark:bg-gray-800" />
        <div className="inline-flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
          <Loader2 className="w-4 h-4 animate-spin" /> Generating pairing code…
        </div>
      </div>
    )
  }

  if (state.kind === 'showing-qr') {
    return <ShowingQr token={state.token} expiresAt={state.expiresAt} onReset={onReset} />
  }

  if (state.kind === 'success') {
    return (
      <div className="flex flex-col items-center gap-3 p-6" aria-live="polite">
        <CheckCircle2 className="w-12 h-12 text-green-500" />
        <p className="text-sm text-gray-700 dark:text-gray-200 font-medium">
          Device paired successfully
        </p>
        <p className="text-xs text-gray-500 dark:text-gray-400 font-mono break-all max-w-xs text-center">
          {state.deviceId}
        </p>
        <button
          type="button"
          onClick={onReset}
          className="text-sm text-gray-600 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200"
        >
          Done
        </button>
      </div>
    )
  }

  if (state.kind === 'expired') {
    return (
      <div className="flex flex-col items-center gap-3 p-6" aria-live="polite">
        <div className="flex h-48 w-48 items-center justify-center rounded-lg border border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-950">
          <p className="px-4 text-center text-sm text-amber-700 dark:text-amber-300">
            This pairing code expired. Ask for a new one.
          </p>
        </div>
        <button
          type="button"
          onClick={onStart}
          className="inline-flex items-center gap-2 px-3 py-1.5 text-sm rounded-md bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 hover:bg-gray-800 dark:hover:bg-gray-200"
        >
          <RefreshCw className="w-4 h-4" /> Generate new code
        </button>
      </div>
    )
  }

  // error state
  return (
    <div className="flex flex-col items-center gap-3 p-6" aria-live="polite">
      <div className="flex h-48 w-48 items-center justify-center rounded-lg border border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-950">
        <div className="flex flex-col items-center gap-2 px-4 text-center">
          <XCircle className="w-6 h-6 text-red-500" />
          <p className="text-sm text-red-600 dark:text-red-300">{state.message}</p>
          {state.code && (
            <p className="text-[11px] font-mono text-red-500 dark:text-red-400/80">{state.code}</p>
          )}
        </div>
      </div>
      <button
        type="button"
        onClick={onStart}
        className="rounded-md bg-gray-900 dark:bg-gray-100 px-3 py-1.5 text-sm text-white dark:text-gray-900 hover:bg-gray-800 dark:hover:bg-gray-200"
      >
        Try again
      </button>
    </div>
  )
}

function ShowingQr({
  token,
  expiresAt,
  onReset,
}: {
  token: string
  expiresAt: Date
  onReset: () => void
}) {
  const [remainingMs, setRemainingMs] = useState(() =>
    Math.max(0, expiresAt.getTime() - Date.now()),
  )

  useEffect(() => {
    const tick = () => setRemainingMs(Math.max(0, expiresAt.getTime() - Date.now()))
    tick()
    const id = setInterval(tick, 1000)
    return () => clearInterval(id)
  }, [expiresAt])

  const totalSeconds = Math.ceil(remainingMs / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  const countdown = `${minutes}:${seconds.toString().padStart(2, '0')}`
  const urgent = totalSeconds <= 30

  return (
    <div className="flex flex-col items-center gap-3 p-6" aria-live="polite">
      <div className="rounded-lg bg-white p-3 border border-gray-200 dark:border-gray-700">
        <QRCodeSVG value={buildPairingUrl(token)} size={192} level="M" />
      </div>
      <p className="text-sm text-gray-600 dark:text-gray-400 text-center max-w-xs">
        Open the Claude View mobile app and scan this code to pair.
      </p>
      <p
        className={
          urgent
            ? 'text-xs font-mono tabular-nums text-red-600 dark:text-red-400'
            : 'text-xs font-mono tabular-nums text-gray-500 dark:text-gray-400'
        }
      >
        Expires in {countdown}
      </p>
      <button
        type="button"
        onClick={onReset}
        className="text-sm text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200"
      >
        Cancel
      </button>
    </div>
  )
}
