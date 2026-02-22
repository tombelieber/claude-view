import { useState } from 'react'
import { Smartphone, X, Trash2, Loader2 } from 'lucide-react'
import { QRCodeSVG } from 'qrcode.react'
import * as Popover from '@radix-ui/react-popover'
import { useQrCode, usePairedDevices, useUnpairDevice } from '../hooks/use-pairing'

export function PairingPanel() {
  const [open, setOpen] = useState(false)
  const [showQr, setShowQr] = useState(false)
  const { data: devices = [] } = usePairedDevices()
  const { data: qr, isLoading: qrLoading } = useQrCode(
    open && (devices.length === 0 || showQr),
  )
  const unpair = useUnpairDevice()

  const hasPairedDevices = devices.length > 0

  return (
    <Popover.Root
      open={open}
      onOpenChange={(o) => {
        setOpen(o)
        if (!o) setShowQr(false)
      }}
    >
      <Popover.Trigger asChild>
        <button
          aria-label="Mobile devices"
          className="relative p-2 text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 rounded-md"
        >
          <Smartphone className="w-5 h-5" />
          {!hasPairedDevices && (
            <span className="absolute top-1 right-1 w-2 h-2 bg-green-500 rounded-full" />
          )}
        </button>
      </Popover.Trigger>

      <Popover.Portal>
        <Popover.Content
          align="end"
          sideOffset={8}
          className="w-72 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg p-4 z-50"
        >
          <div className="flex items-center justify-between mb-3">
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
              Mobile Access
            </h3>
            <Popover.Close asChild>
              <button className="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 cursor-pointer rounded">
                <X className="w-4 h-4" />
              </button>
            </Popover.Close>
          </div>

          {/* QR Code Section */}
          {(!hasPairedDevices || showQr) && (
            <div className="mb-4">
              <div className="bg-white rounded-lg p-3 flex items-center justify-center min-h-[160px]">
                {qrLoading ? (
                  <Loader2 className="w-8 h-8 text-gray-400 animate-spin" />
                ) : qr ? (
                  <div className="text-center">
                    <QRCodeSVG
                      value={JSON.stringify(qr)}
                      size={144}
                      level="M"
                      className="mx-auto"
                    />
                    <p className="text-xs text-gray-500 mt-2">
                      Scan with your phone camera
                    </p>
                  </div>
                ) : (
                  <p className="text-sm text-gray-500">Failed to generate QR</p>
                )}
              </div>
            </div>
          )}

          {/* Paired Devices */}
          {hasPairedDevices && (
            <div>
              <h4 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-2">
                Paired Devices
              </h4>
              {devices.map((d) => (
                <div
                  key={d.device_id}
                  className="flex items-center justify-between py-2 border-t border-gray-100 dark:border-gray-800"
                >
                  <div>
                    <p className="text-sm text-gray-900 dark:text-gray-100">
                      {d.name || d.device_id}
                    </p>
                    <p className="text-xs text-gray-500">
                      {d.paired_at > 0
                        ? new Date(d.paired_at * 1000).toLocaleDateString()
                        : 'Unknown'}
                    </p>
                  </div>
                  <button
                    onClick={() => unpair.mutate(d.device_id)}
                    className="p-1.5 text-gray-400 hover:text-red-500 cursor-pointer rounded transition-colors"
                    aria-label={`Remove ${d.name || d.device_id}`}
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              ))}
              {!showQr && (
                <button
                  onClick={() => setShowQr(true)}
                  className="mt-2 text-xs text-blue-500 hover:text-blue-400 cursor-pointer"
                >
                  + Pair another device
                </button>
              )}
            </div>
          )}

          {!hasPairedDevices && !qrLoading && !qr && (
            <p className="text-xs text-gray-500 text-center">No devices paired</p>
          )}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
