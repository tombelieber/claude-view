// Audit gap #3: verbatimModuleSyntax requires namespace imports for CJS modules
import * as nacl from 'tweetnacl'
import { decodeBase64, decodeUTF8, encodeBase64 } from 'tweetnacl-util'
import type { KeyStorage } from './storage'

const SIGNING_KEY = 'device_signing_key'
const ENCRYPTION_KEY = 'device_encryption_key'
const DEVICE_ID_KEY = 'device_id'

export interface PhoneKeys {
  signingKeyPair: nacl.SignKeyPair
  boxKeyPair: nacl.BoxKeyPair
  deviceId: string
}

/** Generate and store phone keypair in secure storage. */
export async function generatePhoneKeys(storage: KeyStorage): Promise<PhoneKeys> {
  const signingKeyPair = nacl.sign.keyPair()
  const boxKeyPair = nacl.box.keyPair()
  // React Native doesn't have crypto.randomUUID() -- use nacl.randomBytes instead
  const randomBytes = nacl.randomBytes(4)
  const hex = Array.from(randomBytes, (b) => b.toString(16).padStart(2, '0')).join('')
  const deviceId = `phone-${hex}`

  await storage.setItem(SIGNING_KEY, encodeBase64(signingKeyPair.secretKey))
  await storage.setItem(ENCRYPTION_KEY, encodeBase64(boxKeyPair.secretKey))
  await storage.setItem(DEVICE_ID_KEY, deviceId)

  return { signingKeyPair, boxKeyPair, deviceId }
}

/** Load existing keys from storage, or null if not paired. */
export async function loadPhoneKeys(storage: KeyStorage): Promise<PhoneKeys | null> {
  const [signingB64, encryptionB64, deviceId] = await Promise.all([
    storage.getItem(SIGNING_KEY),
    storage.getItem(ENCRYPTION_KEY),
    storage.getItem(DEVICE_ID_KEY),
  ])
  if (!signingB64 || !encryptionB64 || !deviceId) return null

  const signingSecret = decodeBase64(signingB64)
  const boxSecret = decodeBase64(encryptionB64)
  return {
    signingKeyPair: nacl.sign.keyPair.fromSecretKey(signingSecret),
    boxKeyPair: nacl.box.keyPair.fromSecretKey(boxSecret),
    deviceId,
  }
}

/** Sign auth challenge: "timestamp:device_id" */
export function signAuthChallenge(
  deviceId: string,
  signingSecretKey: Uint8Array,
): { timestamp: number; signature: string } {
  const timestamp = Math.floor(Date.now() / 1000)
  const payload = `${timestamp}:${deviceId}`
  const signature = nacl.sign.detached(decodeUTF8(payload), signingSecretKey)
  return { timestamp, signature: encodeBase64(signature) }
}

/** Decrypt a NaCl box message (nonce || ciphertext) from Mac. */
export function decryptFromDevice(
  encryptedB64: string,
  senderPubkey: Uint8Array,
  recipientSecretKey: Uint8Array,
): Uint8Array | null {
  const wire = decodeBase64(encryptedB64)
  const nonce = wire.slice(0, nacl.box.nonceLength)
  const ciphertext = wire.slice(nacl.box.nonceLength)
  return nacl.box.open(ciphertext, nonce, senderPubkey, recipientSecretKey)
}

/** Encrypt phone pubkey for Mac using NaCl box. */
export function encryptForDevice(
  plaintext: Uint8Array,
  recipientPubkey: Uint8Array,
  senderSecretKey: Uint8Array,
): string {
  const nonce = nacl.randomBytes(nacl.box.nonceLength)
  const ciphertext = nacl.box(plaintext, nonce, recipientPubkey, senderSecretKey)
  const wire = new Uint8Array(nonce.length + ciphertext.length)
  wire.set(nonce)
  wire.set(ciphertext, nonce.length)
  return encodeBase64(wire)
}

export interface ClaimPairingParams {
  macPubkeyB64: string
  token: string
  relayUrl: string
  keys: PhoneKeys
  storage: KeyStorage
  /** Verification secret from QR code (base64, 32 bytes). Used for HMAC anti-MITM binding. */
  verificationSecret?: string
}

/**
 * Compute HMAC-SHA256(secret, data) using Web Crypto API.
 * Returns the HMAC as a base64 string.
 */
async function computeHmacSha256(secretB64: string, data: Uint8Array): Promise<string> {
  const secretBytes = decodeBase64(secretB64)
  const key = await crypto.subtle.importKey(
    'raw',
    secretBytes,
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  )
  const sig = await crypto.subtle.sign('HMAC', key, data)
  return encodeBase64(new Uint8Array(sig))
}

/** Claim a pairing offer at the relay. Stores keys and relay URL in secure storage. */
export async function claimPairing(params: ClaimPairingParams): Promise<void> {
  const { macPubkeyB64, token, relayUrl, keys, storage, verificationSecret } = params
  const macPubkey = decodeBase64(macPubkeyB64)

  // Encrypt phone's X25519 pubkey for Mac
  const encryptedBlob = encryptForDevice(
    keys.boxKeyPair.publicKey,
    macPubkey,
    keys.boxKeyPair.secretKey,
  )

  // Compute HMAC-SHA256(verification_secret, phone_x25519_pubkey) for anti-MITM binding.
  // The verification secret came from the QR code (never sent to relay).
  let verificationHmac: string | undefined
  if (verificationSecret) {
    verificationHmac = await computeHmacSha256(verificationSecret, keys.boxKeyPair.publicKey)
  }

  // Audit gap #30: Use regex to strip only trailing /ws
  const httpUrl = relayUrl
    .replace('wss://', 'https://')
    .replace('ws://', 'http://')
    .replace(/\/ws$/, '')

  const body: Record<string, string> = {
    one_time_token: token,
    device_id: keys.deviceId,
    pubkey: encodeBase64(keys.signingKeyPair.publicKey),
    pubkey_encrypted_blob: encryptedBlob,
    x25519_pubkey: encodeBase64(keys.boxKeyPair.publicKey),
  }
  if (verificationHmac) {
    body.verification_hmac = verificationHmac
  }

  const res = await fetch(`${httpUrl}/pair/claim`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })

  if (!res.ok) {
    const status = res.status
    if (status === 404) throw new Error('Pairing code not found. Try scanning again.')
    if (status === 410) throw new Error('Pairing code expired. Generate a new one on Mac.')
    throw new Error(`Pairing failed (${status})`)
  }

  // Store relay URL and Mac pubkey for future connections
  await storage.setItem('relay_url', relayUrl)
  await storage.setItem('mac_x25519_pubkey', macPubkeyB64)
}

/** Clear all pairing data from storage. */
export async function unpair(storage: KeyStorage): Promise<void> {
  await Promise.all([
    storage.removeItem(SIGNING_KEY),
    storage.removeItem(ENCRYPTION_KEY),
    storage.removeItem(DEVICE_ID_KEY),
    storage.removeItem('relay_url'),
    storage.removeItem('mac_x25519_pubkey'),
  ])
}
