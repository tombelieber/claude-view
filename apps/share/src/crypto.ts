/**
 * Decode a base64url string (no padding) to Uint8Array.
 */
function base64urlDecode(str: string): Uint8Array {
  const base64 = str.replace(/-/g, '+').replace(/_/g, '/')
  const padded = base64 + '='.repeat((4 - (base64.length % 4)) % 4)
  const binary = atob(padded)
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i)
  }
  return bytes
}

/**
 * Decompress gzip bytes using DecompressionStream (available in all modern browsers).
 */
async function gunzip(data: Uint8Array): Promise<Uint8Array> {
  const ds = new DecompressionStream('gzip')
  const writer = ds.writable.getWriter()
  const reader = ds.readable.getReader()

  writer.write(data.buffer as ArrayBuffer)
  writer.close()

  const chunks: Uint8Array[] = []
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    chunks.push(value)
  }

  const totalLength = chunks.reduce((sum, c) => sum + c.length, 0)
  const result = new Uint8Array(totalLength)
  let offset = 0
  for (const chunk of chunks) {
    result.set(chunk, offset)
    offset += chunk.length
  }
  return result
}

/**
 * Decrypt a share blob using the AES-256-GCM key from the URL fragment.
 *
 * URL format: /s/{token}#k={base64url_key}
 * Blob format: [12 bytes nonce][ciphertext+tag]
 *
 * @returns Parsed session data as a plain object.
 */
export async function decryptShareBlob(blob: ArrayBuffer, keyBase64url: string): Promise<unknown> {
  const keyBytes = base64urlDecode(keyBase64url)

  const cryptoKey = await crypto.subtle.importKey(
    'raw',
    keyBytes.buffer as ArrayBuffer,
    { name: 'AES-GCM' },
    false,
    ['decrypt'],
  )

  const blobBytes = new Uint8Array(blob)
  const iv = blobBytes.slice(0, 12)
  const ciphertext = blobBytes.slice(12)

  const plaintext = await crypto.subtle.decrypt(
    { name: 'AES-GCM', iv: iv.buffer as ArrayBuffer },
    cryptoKey,
    ciphertext.buffer as ArrayBuffer,
  )

  const decompressed = await gunzip(new Uint8Array(plaintext))
  const text = new TextDecoder().decode(decompressed)
  return JSON.parse(text)
}
