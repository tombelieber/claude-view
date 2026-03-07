import type { KeyStorage } from '@claude-view/shared'
import * as SecureStore from 'expo-secure-store'

export const secureStoreAdapter: KeyStorage = {
  async getItem(key: string) {
    return SecureStore.getItemAsync(key)
  },
  async setItem(key: string, value: string) {
    await SecureStore.setItemAsync(key, value)
  },
  async removeItem(key: string) {
    await SecureStore.deleteItemAsync(key)
  },
}
