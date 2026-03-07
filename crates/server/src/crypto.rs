//! NaCl box encryption, Ed25519 signing, and file-based key storage.

use base64::{engine::general_purpose::STANDARD, Engine};
use crypto_box::{
    aead::{Aead, AeadCore, OsRng},
    PublicKey as BoxPublicKey, SalsaBox, SecretKey as BoxSecretKey,
};
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

/// Directory for storing keys: ~/.claude-view/
fn storage_dir() -> Result<PathBuf, String> {
    let dir = dirs::home_dir()
        .ok_or("no home directory")?
        .join(".claude-view");
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

/// A paired remote device.
#[derive(Serialize, Deserialize, Clone)]
pub struct PairedDevice {
    pub device_id: String,
    pub x25519_pubkey: String, // base64
    pub name: String,
    pub paired_at: u64, // unix timestamp
}

/// Load or create device identity from ~/.claude-view/identity.json.
pub fn load_or_create_identity() -> Result<DeviceIdentity, String> {
    let path = storage_dir()?.join("identity.json");

    // Try to load existing
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

    // Generate fresh keys
    let signing_key = SigningKey::generate(&mut OsRng);
    let box_secret = BoxSecretKey::generate(&mut OsRng);
    let device_id = format!("mac-{}", &uuid::Uuid::new_v4().to_string()[..8]);

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

/// Load paired devices from ~/.claude-view/paired-devices.json.
pub fn load_paired_devices() -> Vec<PairedDevice> {
    let path = match storage_dir() {
        Ok(d) => d.join("paired-devices.json"),
        Err(_) => return Vec::new(),
    };
    match fs::read(&path) {
        Ok(data) => match serde_json::from_slice(&data) {
            Ok(devices) => devices,
            Err(e) => {
                tracing::error!("paired-devices.json corrupted, cannot load devices: {e}");
                Vec::new()
            }
        },
        Err(_) => Vec::new(),
    }
}

/// Save paired devices to ~/.claude-view/paired-devices.json.
pub fn save_paired_devices(devices: &[PairedDevice]) -> Result<(), String> {
    let path = storage_dir()?.join("paired-devices.json");
    let json = serde_json::to_vec_pretty(devices).map_err(|e| e.to_string())?;
    fs::write(&path, &json).map_err(|e| format!("write paired devices: {e}"))?;
    Ok(())
}

/// Add a newly paired device.
pub fn add_paired_device(device: PairedDevice) -> Result<(), String> {
    let mut devices = load_paired_devices();
    // Replace if already exists
    devices.retain(|d| d.device_id != device.device_id);
    devices.push(device);
    save_paired_devices(&devices)
}

/// Remove a paired device by ID.
pub fn remove_paired_device(device_id: &str) -> Result<(), String> {
    let mut devices = load_paired_devices();
    devices.retain(|d| d.device_id != device_id);
    save_paired_devices(&devices)
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

    // Wire format: nonce (24 bytes) || ciphertext
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

/// Sign an auth challenge for relay authentication.
pub fn sign_auth_challenge(identity: &DeviceIdentity) -> Result<(u64, String), String> {
    let signing_bytes = STANDARD
        .decode(&identity.signing_key)
        .map_err(|e| format!("bad signing key: {e}"))?;
    let signing_key = SigningKey::from_bytes(
        &<[u8; 32]>::try_from(signing_bytes.as_slice())
            .map_err(|_| "signing key must be 32 bytes")?,
    );

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs();

    let payload = format!("{}:{}", timestamp, identity.device_id);
    let signature = signing_key.sign(payload.as_bytes());

    Ok((timestamp, STANDARD.encode(signature.to_bytes())))
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

// ---------------------------------------------------------------------------
// HMAC verification secret storage (anti-MITM pairing binding)
// ---------------------------------------------------------------------------

/// Store a pairing verification secret (keyed by one-time token).
/// The secret is known only to the Mac and encoded into the QR URL.
/// It is never sent to the relay server.
pub fn store_verification_secret(token: &str, secret: &[u8; 32]) -> Result<(), String> {
    let dir = storage_dir()?.join("pairing_secrets");
    fs::create_dir_all(&dir).map_err(|e| format!("create pairing_secrets dir: {e}"))?;
    fs::write(dir.join(token), secret).map_err(|e| format!("write verification secret: {e}"))?;
    Ok(())
}

/// Verify an HMAC-SHA256(verification_secret, phone_x25519_pubkey) against
/// all stored pairing secrets. On match, the secret file is consumed (deleted).
///
/// This prevents relay key substitution attacks: the relay never sees the
/// verification secret (it's in the QR URL only), so it cannot forge the HMAC.
pub fn find_and_verify_hmac(phone_x25519_pubkey_b64: &str, hmac_b64: &str) -> Result<bool, String> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let dir = storage_dir()?.join("pairing_secrets");
    if !dir.exists() {
        return Ok(false);
    }
    let expected_hmac = STANDARD
        .decode(hmac_b64)
        .map_err(|e| format!("bad hmac base64: {e}"))?;
    let pubkey_bytes = STANDARD
        .decode(phone_x25519_pubkey_b64)
        .map_err(|e| format!("bad pubkey base64: {e}"))?;

    let entries = fs::read_dir(&dir).map_err(|e| format!("read pairing_secrets dir: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read dir entry: {e}"))?;
        let secret_bytes = fs::read(entry.path()).map_err(|e| format!("read secret: {e}"))?;
        if secret_bytes.len() != 32 {
            continue;
        }
        let mut mac =
            HmacSha256::new_from_slice(&secret_bytes).map_err(|e| format!("hmac init: {e}"))?;
        mac.update(&pubkey_bytes);
        if mac.verify_slice(&expected_hmac).is_ok() {
            // Consume the one-time secret
            let _ = fs::remove_file(entry.path());
            return Ok(true);
        }
    }
    Ok(false)
}
