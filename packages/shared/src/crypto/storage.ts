/** Abstract key storage -- web uses IndexedDB, mobile uses expo-secure-store */
export interface KeyStorage {
  getItem(key: string): Promise<string | null>
  setItem(key: string, value: string): Promise<void>
  removeItem(key: string): Promise<void>
}
