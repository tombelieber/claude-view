import { loadPhoneKeys, signAuthChallenge } from '@claude-view/shared'
import Constants from 'expo-constants'
import * as Notifications from 'expo-notifications'
import { useEffect, useRef } from 'react'
import { AppState } from 'react-native'
import { secureStoreAdapter } from '../lib/secure-store-adapter'

Notifications.setNotificationHandler({
  handleNotification: async () => ({
    shouldShowAlert: true,
    shouldPlaySound: true,
    shouldSetBadge: true,
    shouldShowBanner: true,
    shouldShowList: true,
  }),
})

export function usePushNotifications() {
  const listenerRef = useRef<Notifications.EventSubscription | undefined>(undefined)

  useEffect(() => {
    registerPushToken()

    listenerRef.current = Notifications.addNotificationResponseReceivedListener((_response) => {
      // Best-effort navigation — user lands on dashboard which shows sessions.
      // Deep-linking to a specific session can be added in a future iteration.
    })

    // Re-register push token on foreground to handle token rotation
    const appStateListener = AppState.addEventListener('change', (state) => {
      if (state === 'active') registerPushToken()
    })

    return () => {
      listenerRef.current?.remove()
      appStateListener.remove()
    }
  }, [])
}

async function registerPushToken() {
  const { status } = await Notifications.requestPermissionsAsync({
    ios: { allowAlert: true, allowBadge: true, allowSound: true },
  })
  if (status !== 'granted') return

  const tokenData = await Notifications.getExpoPushTokenAsync({
    projectId: Constants.expoConfig?.extra?.eas?.projectId,
  })
  const deviceId = await secureStoreAdapter.getItem('device_id')
  const relayUrl = await secureStoreAdapter.getItem('relay_url')
  const keys = await loadPhoneKeys(secureStoreAdapter)

  if (!deviceId || !relayUrl || !keys) return

  const { timestamp, signature } = signAuthChallenge(deviceId, keys.signingKeyPair.secretKey)

  // Convert WS URL to HTTP for the REST endpoint
  const httpUrl = relayUrl
    .replace('wss://', 'https://')
    .replace('ws://', 'http://')
    .replace(/\/ws$/, '')

  await fetch(`${httpUrl}/push-tokens`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      device_id: deviceId,
      token: tokenData.data,
      timestamp,
      signature,
    }),
  }).catch(() => {
    // Silently fail — push is optional, will retry on next foreground
  })
}
