import { claimPairing, generatePhoneKeys } from '@claude-view/shared'
import { CameraView, useCameraPermissions } from 'expo-camera'
import * as Haptics from 'expo-haptics'
import { router } from 'expo-router'
import { useState } from 'react'
import { Button, Text, YStack } from 'tamagui'
import { secureStoreAdapter } from '../lib/secure-store-adapter'

export default function PairScreen() {
  const [permission, requestPermission] = useCameraPermissions()
  const [scanned, setScanned] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleBarCodeScanned = async ({ data }: { data: string }) => {
    if (scanned) return
    setScanned(true)
    setError(null)
    await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success)

    try {
      const url = new URL(data)
      const macPubkeyB64 = url.searchParams.get('k')
      const token = url.searchParams.get('t')

      if (!macPubkeyB64 || !token) throw new Error('Invalid QR code')

      // Read relay WSS URL from `r` query param (explicit).
      // Falls back to origin-based derivation for backwards compat with older QR codes.
      const rParam = url.searchParams.get('r')
      const relayUrl =
        rParam ?? `${url.origin.replace('https://', 'wss://').replace('http://', 'ws://')}/ws`

      // Read verification secret from QR `s` param for HMAC anti-MITM binding
      const verificationSecret = url.searchParams.get('s')

      const keys = await generatePhoneKeys(secureStoreAdapter)

      await claimPairing({
        macPubkeyB64,
        token,
        relayUrl,
        verificationSecret: verificationSecret ?? undefined,
        keys,
        storage: secureStoreAdapter,
      })

      router.replace('/(tabs)')
    } catch (e) {
      setScanned(false)
      setError(e instanceof Error ? e.message : 'Pairing failed')
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error)
    }
  }

  if (!permission?.granted) {
    return (
      <YStack flex={1} bg="$gray900" items="center" justify="center" p="$8">
        <Text color="$gray50" fontSize="$lg" text="center" mb="$6">
          Camera access needed to scan QR code
        </Text>
        <Button onPress={requestPermission} bg="$primary600" size="$5">
          Grant Camera Access
        </Button>
      </YStack>
    )
  }

  return (
    <YStack flex={1} bg="$gray900">
      <CameraView
        style={{ flex: 1 }}
        barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
        onBarcodeScanned={scanned ? undefined : handleBarCodeScanned}
      />
      <YStack position="absolute" b={0} l={0} r={0} p="$8" items="center">
        <Text color="$gray50" fontSize="$lg" text="center">
          Scan the QR code from your Mac's Claude View
        </Text>
        <Text color="$gray400" fontSize="$sm" mt="$2" text="center">
          One scan. No account. No password. Ever.
        </Text>
        {error && (
          <Text color="#ef4444" fontSize="$sm" mt="$4" text="center">
            {error}
          </Text>
        )}
      </YStack>
    </YStack>
  )
}
