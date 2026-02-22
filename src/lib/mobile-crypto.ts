import * as nacl from 'tweetnacl'
import * as naclUtil from 'tweetnacl-util'
import { getItem, setItem } from './mobile-storage.ts'

const { decodeBase64, encodeBase64, decodeUTF8, encodeUTF8 } = naclUtil

/** Generate and store phone keypairs in IndexedDB. */
export async function generatePhoneKeys(): Promise<{
  encryptionPublicKey: Uint8Array
  signingPublicKey: Uint8Array
}> {
  const encKp = nacl.box.keyPair()
  const signKp = nacl.sign.keyPair()

  await setItem('enc_secret', encodeBase64(encKp.secretKey))
  await setItem('enc_public', encodeBase64(encKp.publicKey))
  await setItem('sign_secret', encodeBase64(signKp.secretKey))
  await setItem('sign_public', encodeBase64(signKp.publicKey))

  return {
    encryptionPublicKey: encKp.publicKey,
    signingPublicKey: signKp.publicKey,
  }
}

/** Decrypt a NaCl box message from Mac. */
export async function decryptMessage(
  encryptedBase64: string,
  macPublicKeyBase64: string,
): Promise<string | null> {
  const secretKeyB64 = await getItem('enc_secret')
  if (!secretKeyB64) return null

  const secretKey = decodeBase64(secretKeyB64)
  const macPublicKey = decodeBase64(macPublicKeyBase64)
  const wire = decodeBase64(encryptedBase64)

  // Wire format: nonce (24 bytes) || ciphertext
  const nonce = wire.slice(0, 24)
  const ciphertext = wire.slice(24)

  const plaintext = nacl.box.open(ciphertext, nonce, macPublicKey, secretKey)
  if (!plaintext) return null

  return encodeUTF8(plaintext)
}

/** Sign an auth challenge for relay. */
export async function signAuthChallenge(deviceId: string): Promise<{
  timestamp: number
  signature: string
} | null> {
  const secretKeyB64 = await getItem('sign_secret')
  if (!secretKeyB64) return null

  const secretKey = decodeBase64(secretKeyB64)
  const timestamp = Math.floor(Date.now() / 1000)
  const payload = `${timestamp}:${deviceId}`
  const signature = nacl.sign.detached(decodeUTF8(payload), secretKey)

  return { timestamp, signature: encodeBase64(signature) }
}

/** Check if phone has stored keys (= is paired). */
export async function isPaired(): Promise<boolean> {
  const key = await getItem('enc_secret')
  return key !== null
}

/** Get stored Mac public key (set during pairing). */
export async function getMacPublicKey(): Promise<string | null> {
  return getItem('mac_enc_public')
}

/** Store Mac public key after QR scan. */
export async function storeMacPublicKey(pubkeyBase64: string): Promise<void> {
  await setItem('mac_enc_public', pubkeyBase64)
}
