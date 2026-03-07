const BASE62 = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz'

/** Generate a 22-char base62 token (131 bits of entropy). */
export function generateToken(): string {
  const bytes = new Uint8Array(22)
  crypto.getRandomValues(bytes)
  let result = ''
  for (const byte of bytes) {
    result += BASE62[byte % 62]
  }
  return result
}
