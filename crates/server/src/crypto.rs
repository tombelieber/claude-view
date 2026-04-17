//! NaCl box encryption, Ed25519 signing, and device-key storage.
//!
//! Post-Phase-2: every legacy HMAC/pairing helper was removed. What remains
//! is the per-device identity plus the E2EE helpers used by relay_client
//! to encrypt `LiveSession` payloads for paired phones.

use base64::{engine::general_purpose::STANDARD, Engine};
use crypto_box::{
    aead::{Aead, AeadCore, OsRng},
    PublicKey as BoxPublicKey, SalsaBox, SecretKey as BoxSecretKey,
};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::info;

/// Directory for storing keys — respects `CLAUDE_VIEW_DATA_DIR`.
fn storage_dir() -> Result<PathBuf, String> {
    let dir = claude_view_core::paths::crypto_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// All keys for this device.
#[derive(Serialize, Deserialize)]
pub struct DeviceIdentity {
    /// Ed25519 signing key (32 bytes, base64).
    pub signing_key: String,
    /// X25519 encryption secret key (32 bytes, base64).
    pub encryption_key: String,
    /// Unique device ID.
    pub device_id: String,
}

/// Load or create device identity from ~/.claude-view/identity.json.
pub fn load_or_create_identity() -> Result<DeviceIdentity, String> {
    let path = storage_dir()?.join("identity.json");

    if path.exists() {
        let data = fs::read(&path).map_err(|e| format!("read identity: {e}"))?;
        match serde_json::from_slice::<DeviceIdentity>(&data) {
            Ok(identity) => {
                info!("loaded device identity from {}", path.display());
                return Ok(identity);
            }
            Err(e) => {
                return Err(format!(
                    "identity.json exists but is corrupt ({}). \
                     Remove {} manually to regenerate keys (WARNING: this invalidates all pairings).",
                    e,
                    path.display()
                ));
            }
        }
    }

    let signing_key = SigningKey::generate(&mut OsRng);
    let box_secret = BoxSecretKey::generate(&mut OsRng);
    let device_id = format!(
        "mac-{}",
        &uuid::Uuid::new_v4().to_string()[..16].replace('-', "")
    );

    let identity = DeviceIdentity {
        signing_key: STANDARD.encode(signing_key.to_bytes()),
        encryption_key: STANDARD.encode(box_secret.to_bytes()),
        device_id,
    };

    let json = serde_json::to_vec_pretty(&identity).map_err(|e| e.to_string())?;
    fs::write(&path, &json).map_err(|e| format!("write identity: {e}"))?;

    info!(device_id = %identity.device_id, "created new device identity at {}", path.display());
    Ok(identity)
}

/// Encrypt a message for a paired device using NaCl box.
pub fn encrypt_for_device(
    plaintext: &[u8],
    recipient_pubkey_b64: &str,
    sender_secret: &BoxSecretKey,
) -> Result<String, String> {
    let recipient_pubkey_bytes = STANDARD
        .decode(recipient_pubkey_b64)
        .map_err(|e| format!("bad pubkey base64: {e}"))?;
    let recipient_pubkey = BoxPublicKey::from(
        <[u8; 32]>::try_from(recipient_pubkey_bytes.as_slice())
            .map_err(|_| "pubkey must be 32 bytes")?,
    );

    let salsa_box = SalsaBox::new(&recipient_pubkey, sender_secret);
    let nonce = SalsaBox::generate_nonce(&mut OsRng);
    let ciphertext = salsa_box
        .encrypt(&nonce, plaintext)
        .map_err(|e| format!("encryption failed: {e}"))?;

    let mut wire = nonce.to_vec();
    wire.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(wire))
}

/// Decrypt a NaCl box message from a paired device.
/// Wire format: nonce (24 bytes) || ciphertext, base64-encoded.
pub fn decrypt_from_device(
    encrypted_b64: &str,
    sender_pubkey_b64: &str,
    recipient_secret: &BoxSecretKey,
) -> Result<Vec<u8>, String> {
    let wire = STANDARD
        .decode(encrypted_b64)
        .map_err(|e| format!("bad encrypted base64: {e}"))?;
    if wire.len() < 24 {
        return Err("encrypted data too short (need at least nonce)".into());
    }

    let sender_pubkey_bytes = STANDARD
        .decode(sender_pubkey_b64)
        .map_err(|e| format!("bad sender pubkey base64: {e}"))?;
    let sender_pubkey = BoxPublicKey::from(
        <[u8; 32]>::try_from(sender_pubkey_bytes.as_slice())
            .map_err(|_| "sender pubkey must be 32 bytes")?,
    );

    let nonce = crypto_box::Nonce::from_slice(&wire[..24]);
    let ciphertext = &wire[24..];

    let salsa_box = SalsaBox::new(&sender_pubkey, recipient_secret);
    salsa_box
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("decryption failed: {e}"))
}

/// Get the X25519 BoxSecretKey from the identity.
pub fn box_secret_key(identity: &DeviceIdentity) -> Result<BoxSecretKey, String> {
    let bytes = STANDARD
        .decode(&identity.encryption_key)
        .map_err(|e| format!("bad encryption key: {e}"))?;
    Ok(BoxSecretKey::from(
        <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| "encryption key must be 32 bytes")?,
    ))
}

/// Get the Ed25519 verifying (public) key bytes from the identity.
pub fn verifying_key_bytes(identity: &DeviceIdentity) -> Result<Vec<u8>, String> {
    let signing_bytes = STANDARD
        .decode(&identity.signing_key)
        .map_err(|e| format!("bad signing key: {e}"))?;
    let signing_key = SigningKey::from_bytes(
        &<[u8; 32]>::try_from(signing_bytes.as_slice())
            .map_err(|_| "signing key must be 32 bytes")?,
    );
    Ok(signing_key.verifying_key().to_bytes().to_vec())
}

/// Best-effort cleanup of the Phase-1 legacy on-disk artifacts. Failures
/// are logged and ignored — we never want startup to die because of a
/// stale file. Callers should invoke this once at server startup.
pub fn cleanup_legacy_pairing_artifacts() {
    let legacy: [PathBuf; 2] = [
        claude_view_core::paths::config_dir().join("paired-devices.json"),
        claude_view_core::paths::config_dir().join("pairing_secrets"),
    ];
    for path in legacy {
        if !path.exists() {
            continue;
        }
        let result = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        match result {
            Ok(()) => info!(path = %path.display(), "cleaned up legacy pairing artifact"),
            Err(e) => {
                tracing::warn!(path = %path.display(), "failed to clean legacy artifact: {e}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encrypt_decrypt() {
        // Two fresh identities stand in for paired mac+phone.
        let alice_secret = BoxSecretKey::generate(&mut OsRng);
        let alice_pub_b64 = STANDARD.encode(alice_secret.public_key().as_bytes());
        let bob_secret = BoxSecretKey::generate(&mut OsRng);
        let bob_pub_b64 = STANDARD.encode(bob_secret.public_key().as_bytes());

        let msg = b"hello, paired device";
        let ciphertext = encrypt_for_device(msg, &bob_pub_b64, &alice_secret).unwrap();
        let plain = decrypt_from_device(&ciphertext, &alice_pub_b64, &bob_secret).unwrap();
        assert_eq!(plain, msg);
    }
}
