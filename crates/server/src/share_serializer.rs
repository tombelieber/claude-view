use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm,
};
use flate2::{write::GzEncoder, Compression};
use std::io::Write;
use std::path::Path;

use crate::error::{ApiError, ApiResult};

pub struct EncryptedShare {
    pub blob: Vec<u8>,
    pub key: Vec<u8>,
}

pub async fn serialize_and_encrypt(file_path: &Path) -> ApiResult<EncryptedShare> {
    let parsed = claude_view_core::parse_session(file_path)
        .await
        .map_err(|e| ApiError::Internal(format!("Parse: {e}")))?;

    let json =
        serde_json::to_vec(&parsed).map_err(|e| ApiError::Internal(format!("Serialize: {e}")))?;

    let compressed = {
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&json)
            .map_err(|e| ApiError::Internal(format!("Gzip write: {e}")))?;
        enc.finish()
            .map_err(|e| ApiError::Internal(format!("Gzip finish: {e}")))?
    };

    let key_bytes = Aes256Gcm::generate_key(&mut OsRng);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let cipher = Aes256Gcm::new(&key_bytes);

    let ciphertext = cipher
        .encrypt(&nonce, compressed.as_ref())
        .map_err(|e| ApiError::Internal(format!("Encrypt: {e}")))?;

    // Wire format: [12-byte nonce][ciphertext]
    let mut blob = Vec::with_capacity(12 + ciphertext.len());
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ciphertext);

    Ok(EncryptedShare {
        blob,
        key: key_bytes.to_vec(),
    })
}

pub fn key_to_base64url(key: &[u8]) -> String {
    use base64::engine::{general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.encode(key)
}
