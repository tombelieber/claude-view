import type { KeyStorage } from '@claude-view/shared'
import { Platform } from 'react-native'

const getSecureStore = () => {
  if (Platform.OS === 'web') return null
  return require('expo-secure-store') as typeof import('expo-secure-store')
}

export const secureStoreAdapter: KeyStorage = {
  async getItem(key: string) {
    const store = getSecureStore()
    if (!store) return null
    return store.getItemAsync(key)
  },
  async setItem(key: string, value: string) {
    const store = getSecureStore()
    if (!store) return
    await store.setItemAsync(key, value)
  },
  async removeItem(key: string) {
    const store = getSecureStore()
    if (!store) return
    await store.deleteItemAsync(key)
  },
}
