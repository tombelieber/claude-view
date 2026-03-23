use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm,
};
use flate2::{write::GzEncoder, Compression};
use std::io::Write;

use crate::error::{ApiError, ApiResult};
use claude_view_core::types::{SharePayload, ShareSessionMetadata};

pub struct ShareInput {
    pub file_path: std::path::PathBuf,
    pub share_metadata: Option<ShareSessionMetadata>,
}

pub struct EncryptedShare {
    pub blob: Vec<u8>,
    pub key: Vec<u8>,
}

pub async fn serialize_and_encrypt(input: &ShareInput) -> ApiResult<EncryptedShare> {
    let parsed = claude_view_core::parse_session_with_raw(&input.file_path)
        .await
        .map_err(|e| ApiError::Internal(format!("Parse: {e}")))?;

    let payload = SharePayload {
        messages: parsed.messages,
        metadata: parsed.metadata,
        share_metadata: input.share_metadata.clone(),
    };

    // CRITICAL: serialize `payload` (SharePayload), NOT `parsed` (ParsedSession)
    let json =
        serde_json::to_vec(&payload).map_err(|e| ApiError::Internal(format!("Serialize: {e}")))?;

    // --- gzip + encrypt block (unchanged from current implementation) ---
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
