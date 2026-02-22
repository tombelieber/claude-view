//! NaCl box encryption, Ed25519 signing, and macOS Keychain key storage.

use base64::{engine::general_purpose::STANDARD, Engine};
use crypto_box::{
    aead::{Aead, AeadCore, OsRng},
    PublicKey as BoxPublicKey, SalsaBox, SecretKey as BoxSecretKey,
};
use ed25519_dalek::{Signer, SigningKey};
use security_framework::passwords::{delete_generic_password, set_generic_password};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

const KEYCHAIN_SERVICE: &str = "com.claude-view";
const KEYCHAIN_ACCOUNT_IDENTITY: &str = "identity-keys";
const KEYCHAIN_ACCOUNT_DEVICES: &str = "paired-devices";

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

/// Load or create device identity from macOS Keychain.
pub fn load_or_create_identity() -> Result<DeviceIdentity, String> {
    // Try to load existing
    if let Ok(data) = security_framework::passwords::get_generic_password(
        KEYCHAIN_SERVICE,
        KEYCHAIN_ACCOUNT_IDENTITY,
    ) {
        if let Ok(identity) = serde_json::from_slice::<DeviceIdentity>(&data) {
            info!("loaded device identity from Keychain");
            return Ok(identity);
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

    let json = serde_json::to_vec(&identity).map_err(|e| e.to_string())?;
    set_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT_IDENTITY, &json)
        .map_err(|e| format!("Keychain write failed: {e}"))?;

    info!(device_id = %identity.device_id, "created new device identity in Keychain");
    Ok(identity)
}

/// Load paired devices from Keychain.
pub fn load_paired_devices() -> Vec<PairedDevice> {
    match security_framework::passwords::get_generic_password(
        KEYCHAIN_SERVICE,
        KEYCHAIN_ACCOUNT_DEVICES,
    ) {
        Ok(data) => serde_json::from_slice(&data).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Save paired devices to Keychain.
pub fn save_paired_devices(devices: &[PairedDevice]) -> Result<(), String> {
    let json = serde_json::to_vec(devices).map_err(|e| e.to_string())?;
    // Delete then set (Keychain doesn't have upsert)
    let _ = delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT_DEVICES);
    set_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT_DEVICES, &json)
        .map_err(|e| format!("Keychain write failed: {e}"))?;
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
        .unwrap_or_default()
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
        <[u8; 32]>::try_from(bytes.as_slice())
            .map_err(|_| "encryption key must be 32 bytes")?,
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
