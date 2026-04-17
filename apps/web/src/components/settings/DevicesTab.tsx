import * as Dialog from '@radix-ui/react-dialog'
import { Smartphone, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { useAuth } from '../../hooks/use-auth'
import { useDevices, useRevokeDevice, useTerminateOtherDevices } from '../../hooks/use-devices'
import { usePairingFlow } from '../../hooks/use-pairing-flow'
import { PairingQrCode } from '../PairingQrCode'
import { DialogContent, DialogOverlay } from '../ui/CenteredDialog'
import { DeviceConfirmDialog } from './DeviceConfirmDialog'
import { DeviceList } from './DeviceList'

/**
 * DevicesTab — wraps the full device-management UX:
 * - Shows the list of paired devices (live via Supabase Realtime)
 * - Opens a modal pairing dialog with a QR code
 * - Confirms revoke / sign-out-others via {@link DeviceConfirmDialog}
 *
 * Requires the user to be signed in. If not, renders a sign-in nudge.
 */
export function DevicesTab() {
  const { user, loading: authLoading, openSignIn } = useAuth()
  const { data: devices, isLoading, error } = useDevices()
  const revoke = useRevokeDevice()
  const terminateOthers = useTerminateOtherDevices()
  const pairing = usePairingFlow()

  const [pairOpen, setPairOpen] = useState(false)
  const [revokeTarget, setRevokeTarget] = useState<{ deviceId: string; name: string } | null>(null)
  const [terminateOthersOpen, setTerminateOthersOpen] = useState(false)

  // Detect a successful pair by watching the device list for a new row that
  // appears while the QR is showing. `markPaired` is stable from the hook.
  const previousIdsRef = useRef<Set<string> | null>(null)
  useEffect(() => {
    if (!devices) return
    const currentIds = new Set(devices.filter((d) => d.revoked_at === null).map((d) => d.device_id))
    const previous = previousIdsRef.current
    previousIdsRef.current = currentIds
    if (previous === null) return
    if (pairing.state.kind !== 'showing-qr') return
    for (const id of currentIds) {
      if (!previous.has(id)) {
        pairing.markPaired(id)
        toast.success('Device paired', { description: `New device linked: ${id}` })
        return
      }
    }
  }, [devices, pairing])

  const handleStartPair = () => {
    setPairOpen(true)
    pairing.start()
  }

  const handleClosePair = () => {
    setPairOpen(false)
    pairing.reset()
  }

  const handleRevokeConfirmed = async () => {
    if (!revokeTarget) return
    const { deviceId, name } = revokeTarget
    setRevokeTarget(null)
    try {
      await revoke.mutateAsync({ deviceId })
      toast.success('Device revoked', { description: name })
    } catch (e) {
      toast.error('Revoke failed', {
        description: e instanceof Error ? e.message : 'Unknown error',
      })
    }
  }

  const handleTerminateOthersConfirmed = async () => {
    setTerminateOthersOpen(false)
    try {
      const result = await terminateOthers.mutateAsync({ callingDeviceId: '' })
      toast.success(
        `Signed out ${result.revoked_count} device${result.revoked_count === 1 ? '' : 's'}`,
      )
    } catch (e) {
      toast.error('Sign out failed', {
        description: e instanceof Error ? e.message : 'Unknown error',
      })
    }
  }

  if (authLoading) {
    return <p className="text-sm text-gray-500 dark:text-gray-400">Loading…</p>
  }

  if (!user) {
    return (
      <div className="flex flex-col items-start gap-3 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-5">
        <div className="flex items-center gap-2">
          <Smartphone className="w-4 h-4 text-gray-400" />
          <h3 className="text-sm font-medium text-gray-900 dark:text-gray-100">Devices</h3>
        </div>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Sign in to pair your phone and manage linked devices.
        </p>
        <button
          type="button"
          onClick={() => openSignIn()}
          className="inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 hover:bg-gray-800 dark:hover:bg-gray-200 transition-colors"
        >
          Sign in
        </button>
      </div>
    )
  }

  return (
    <div className="space-y-5">
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 px-5 py-4">
        <div className="flex items-center gap-2 mb-3">
          <Smartphone className="w-4 h-4 text-gray-400 dark:text-gray-500" />
          <h2 className="text-xs font-medium text-gray-400 dark:text-gray-500 uppercase tracking-wider">
            Linked Devices
          </h2>
        </div>
        <DeviceList
          devices={devices}
          isLoading={isLoading}
          error={error as Error | null}
          onRevoke={(deviceId) => {
            const match = devices?.find((d) => d.device_id === deviceId)
            setRevokeTarget({ deviceId, name: match?.display_name ?? deviceId })
          }}
          revokingId={revoke.isPending ? (revoke.variables?.deviceId ?? null) : null}
          onPairNew={handleStartPair}
          onTerminateOthers={() => setTerminateOthersOpen(true)}
          terminateOthersPending={terminateOthers.isPending}
        />
      </div>

      {/* Pairing modal */}
      <Dialog.Root
        open={pairOpen}
        onOpenChange={(open) => {
          if (!open) handleClosePair()
        }}
      >
        <Dialog.Portal>
          <DialogOverlay className="bg-black/50" />
          <DialogContent className="bg-white dark:bg-gray-900 rounded-lg max-w-md shadow-xl">
            <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-gray-100 px-6 pt-5 pb-1">
              Pair a new device
            </Dialog.Title>
            <Dialog.Description className="sr-only">
              Scan the QR code from the Claude View mobile app to pair it with your account.
            </Dialog.Description>
            <Dialog.Close
              className="absolute top-4 right-4 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
              aria-label="Close pairing dialog"
            >
              <X className="w-4 h-4" />
            </Dialog.Close>
            <PairingQrCode
              state={pairing.state}
              onStart={pairing.start}
              onReset={handleClosePair}
            />
          </DialogContent>
        </Dialog.Portal>
      </Dialog.Root>

      {/* Revoke confirm */}
      <DeviceConfirmDialog
        open={revokeTarget !== null}
        onOpenChange={(open) => {
          if (!open) setRevokeTarget(null)
        }}
        title="Revoke this device?"
        description={`${
          revokeTarget?.name ?? 'This device'
        } will be signed out immediately. You can pair it again any time.`}
        confirmLabel="Revoke"
        onConfirm={handleRevokeConfirmed}
      />

      {/* Terminate others confirm */}
      <DeviceConfirmDialog
        open={terminateOthersOpen}
        onOpenChange={setTerminateOthersOpen}
        title="Sign out all other devices?"
        description="Every paired device except this one will be revoked. You'll need to pair them again."
        confirmLabel="Sign out all others"
        onConfirm={handleTerminateOthersConfirmed}
      />
    </div>
  )
}
