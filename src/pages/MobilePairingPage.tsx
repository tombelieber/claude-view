import { useEffect, useRef, useState, useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { Camera, AlertTriangle, Loader } from 'lucide-react'
import jsQR from 'jsqr'
import { generatePhoneKeys, storeMacPublicKey } from '../lib/mobile-crypto.ts'
import { setItem } from '../lib/mobile-storage.ts'
import * as naclUtil from 'tweetnacl-util'

type ScanState = 'init' | 'scanning' | 'processing' | 'error'

interface QRPayload {
  /** relay URL */
  r: string
  /** Mac encryption public key (base64) */
  k: string
  /** pairing token (one-time) */
  t: string
  /** protocol version */
  v: number
}

export function MobilePairingPage() {
  const navigate = useNavigate()
  const videoRef = useRef<HTMLVideoElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const animRef = useRef<number>(0)
  const [scanState, setScanState] = useState<ScanState>('init')
  const [errorMsg, setErrorMsg] = useState<string>('')
  const processingRef = useRef(false)

  const stopCamera = useCallback(() => {
    if (animRef.current) {
      cancelAnimationFrame(animRef.current)
      animRef.current = 0
    }
    if (streamRef.current) {
      streamRef.current.getTracks().forEach((t) => t.stop())
      streamRef.current = null
    }
  }, [])

  const handleQRPayload = useCallback(
    async (payload: QRPayload) => {
      if (processingRef.current) return
      processingRef.current = true
      setScanState('processing')

      try {
        // Generate phone keypairs
        const { encryptionPublicKey, signingPublicKey } = await generatePhoneKeys()

        // Store relay URL and Mac public key
        await setItem('relay_url', payload.r)
        await storeMacPublicKey(payload.k)

        // Generate a device ID
        const deviceId = crypto.randomUUID()
        await setItem('device_id', deviceId)

        // Claim the pairing token on the relay
        const claimUrl = new URL('/pair/claim', payload.r.replace('wss://', 'https://').replace('ws://', 'http://'))
        const response = await fetch(claimUrl.toString(), {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            token: payload.t,
            device_id: deviceId,
            encryption_public_key: naclUtil.encodeBase64(encryptionPublicKey),
            signing_public_key: naclUtil.encodeBase64(signingPublicKey),
            device_name: navigator.userAgent.includes('iPhone')
              ? 'iPhone'
              : navigator.userAgent.includes('Android')
                ? 'Android'
                : 'Mobile',
          }),
        })

        if (!response.ok) {
          const body = await response.text().catch(() => '')
          throw new Error(body || `Claim failed (${response.status})`)
        }

        stopCamera()
        navigate('/mobile/monitor')
      } catch (err) {
        processingRef.current = false
        setScanState('error')
        setErrorMsg(err instanceof Error ? err.message : 'Pairing failed')
      }
    },
    [navigate, stopCamera],
  )

  const startCamera = useCallback(async () => {
    setScanState('init')
    setErrorMsg('')
    processingRef.current = false

    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: 'environment', width: { ideal: 640 }, height: { ideal: 480 } },
      })
      streamRef.current = stream

      const video = videoRef.current
      if (!video) return

      video.srcObject = stream
      await video.play()
      setScanState('scanning')

      const canvas = canvasRef.current
      if (!canvas) return
      const ctx = canvas.getContext('2d', { willReadFrequently: true })
      if (!ctx) return

      function scan() {
        if (!video || video.readyState < video.HAVE_ENOUGH_DATA || processingRef.current) {
          animRef.current = requestAnimationFrame(scan)
          return
        }

        canvas!.width = video.videoWidth
        canvas!.height = video.videoHeight
        ctx!.drawImage(video, 0, 0)

        const imageData = ctx!.getImageData(0, 0, canvas!.width, canvas!.height)
        const code = jsQR(imageData.data, imageData.width, imageData.height, {
          inversionAttempts: 'dontInvert',
        })

        if (code?.data) {
          try {
            const payload = JSON.parse(code.data) as QRPayload
            if (payload.r && payload.k && payload.t && payload.v) {
              void handleQRPayload(payload)
              return // Stop scanning
            }
          } catch {
            // Not a valid QR payload, keep scanning
          }
        }

        animRef.current = requestAnimationFrame(scan)
      }

      animRef.current = requestAnimationFrame(scan)
    } catch (err) {
      setScanState('error')
      if (err instanceof DOMException && err.name === 'NotAllowedError') {
        setErrorMsg('Camera permission denied. Please allow camera access and try again.')
      } else if (err instanceof DOMException && err.name === 'NotFoundError') {
        setErrorMsg('No camera found on this device.')
      } else {
        setErrorMsg(err instanceof Error ? err.message : 'Failed to access camera')
      }
    }
  }, [handleQRPayload])

  useEffect(() => {
    void startCamera()
    return stopCamera
  }, [startCamera, stopCamera])

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-6 bg-gray-950">
      {/* Hidden canvas for frame capture */}
      <canvas ref={canvasRef} className="hidden" />

      {scanState === 'scanning' && (
        <>
          <div className="relative w-full max-w-xs aspect-square rounded-2xl overflow-hidden border-2 border-gray-700 mb-6">
            <video
              ref={videoRef}
              className="w-full h-full object-cover"
              playsInline
              muted
              autoPlay
            />
            {/* Scan overlay corners */}
            <div className="absolute inset-4 pointer-events-none">
              <div className="absolute top-0 left-0 w-8 h-8 border-t-2 border-l-2 border-green-400 rounded-tl" />
              <div className="absolute top-0 right-0 w-8 h-8 border-t-2 border-r-2 border-green-400 rounded-tr" />
              <div className="absolute bottom-0 left-0 w-8 h-8 border-b-2 border-l-2 border-green-400 rounded-bl" />
              <div className="absolute bottom-0 right-0 w-8 h-8 border-b-2 border-r-2 border-green-400 rounded-br" />
            </div>
          </div>
          <p className="text-gray-400 text-center text-sm">
            Point your camera at the QR code on your desktop claude-view
          </p>
        </>
      )}

      {scanState === 'processing' && (
        <div className="flex flex-col items-center gap-4">
          <Loader className="w-12 h-12 text-green-400 animate-spin" />
          <p className="text-gray-300 text-lg font-medium">Pairing...</p>
        </div>
      )}

      {scanState === 'init' && (
        <div className="flex flex-col items-center gap-4">
          <Camera className="w-16 h-16 text-gray-500" />
          <p className="text-gray-400">Starting camera...</p>
          {/* Hidden video element for init state */}
          <video ref={videoRef} className="hidden" playsInline muted autoPlay />
        </div>
      )}

      {scanState === 'error' && (
        <div className="flex flex-col items-center gap-4 max-w-xs">
          <AlertTriangle className="w-16 h-16 text-red-400" />
          <p className="text-red-300 text-center">{errorMsg}</p>
          <button
            className="mt-4 px-6 py-3 bg-green-600 hover:bg-green-500 active:bg-green-700 text-white rounded-lg font-medium transition-colors cursor-pointer min-h-[44px] min-w-[44px]"
            onClick={() => void startCamera()}
          >
            Try Again
          </button>
        </div>
      )}
    </div>
  )
}
