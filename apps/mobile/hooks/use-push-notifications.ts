import Constants from 'expo-constants'
import { useEffect } from 'react'
import { OneSignal } from 'react-native-onesignal'
import { secureStoreAdapter } from '../lib/secure-store-adapter'

const ONESIGNAL_APP_ID = Constants.expoConfig?.extra?.oneSignalAppId || 'YOUR_ONESIGNAL_APP_ID'

export function usePushNotifications() {
  useEffect(() => {
    OneSignal.initialize(ONESIGNAL_APP_ID)
    OneSignal.Notifications.requestPermission(false)

    syncExternalUserId()
  }, [])
}

/**
 * Set the OneSignal external user ID to our device_id so the relay
 * can target push notifications to this specific device.
 */
async function syncExternalUserId() {
  const deviceId = await secureStoreAdapter.getItem('device_id')
  if (!deviceId) return
  OneSignal.login(deviceId)
}
