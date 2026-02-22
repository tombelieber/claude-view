use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Deserialize)]
pub struct AuthMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub device_id: String,
    pub timestamp: u64,
    #[serde(with = "base64_bytes")]
    pub signature: Vec<u8>,
}

/// Verify an Ed25519 auth challenge. Returns true if valid.
pub fn verify_auth(msg: &AuthMessage, verifying_key: &VerifyingKey) -> bool {
    // Check timestamp freshness (60s window)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if now.abs_diff(msg.timestamp) > 60 {
        return false;
    }

    // Verify signature over "timestamp:device_id"
    let payload = format!("{}:{}", msg.timestamp, msg.device_id);
    let Ok(signature) = Signature::from_slice(&msg.signature) else {
        return false;
    };
    verifying_key.verify(payload.as_bytes(), &signature).is_ok()
}

mod base64_bytes {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        STANDARD.decode(s).map_err(serde::de::Error::custom)
    }
}
