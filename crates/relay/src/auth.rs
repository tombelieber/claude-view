use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
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

// --- Supabase JWT validation ---

#[derive(Debug, Deserialize)]
pub struct SupabaseClaims {
    pub sub: String, // user_id
    pub exp: u64,
    pub iss: String,
}

pub struct SupabaseAuth {
    pub decoding_key: DecodingKey,
    pub algorithm: Algorithm,
    pub issuer: String,
    pub supabase_url: String,
}

/// Parse the `alg` field from a JWK JSON value into a `jsonwebtoken::Algorithm`.
fn jwk_algorithm(key_json: &serde_json::Value) -> anyhow::Result<Algorithm> {
    let alg_str = key_json["alg"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("JWK missing `alg` field"))?;

    match alg_str {
        "ES256" => Ok(Algorithm::ES256),
        "ES384" => Ok(Algorithm::ES384),
        "RS256" => Ok(Algorithm::RS256),
        "RS384" => Ok(Algorithm::RS384),
        "RS512" => Ok(Algorithm::RS512),
        "PS256" => Ok(Algorithm::PS256),
        "PS384" => Ok(Algorithm::PS384),
        "PS512" => Ok(Algorithm::PS512),
        "EdDSA" => Ok(Algorithm::EdDSA),
        other => Err(anyhow::anyhow!("Unsupported JWK algorithm: {other}")),
    }
}

impl SupabaseAuth {
    pub async fn from_supabase_url(supabase_url: &str) -> anyhow::Result<Self> {
        let jwks_url = format!("{}/auth/v1/.well-known/jwks.json", supabase_url);
        let resp: serde_json::Value = reqwest::get(&jwks_url).await?.json().await?;
        let key_json = resp["keys"]
            .as_array()
            .and_then(|k| k.first())
            .ok_or_else(|| anyhow::anyhow!("Empty JWKS"))?;

        let algorithm = jwk_algorithm(key_json)?;
        let jwk: jsonwebtoken::jwk::Jwk = serde_json::from_value(key_json.clone())?;
        let decoding_key = DecodingKey::from_jwk(&jwk)?;

        tracing::info!("Relay JWKS loaded: algorithm={algorithm:?}");

        Ok(Self {
            decoding_key,
            algorithm,
            issuer: format!("{}/auth/v1", supabase_url),
            supabase_url: supabase_url.to_string(),
        })
    }

    pub fn validate(&self, token: &str) -> anyhow::Result<String> {
        let mut v = Validation::new(self.algorithm);
        v.set_issuer(&[&self.issuer]);
        v.set_audience(&["authenticated"]);
        let data = decode::<SupabaseClaims>(token, &self.decoding_key, &v)?;
        Ok(data.claims.sub)
    }

    /// Validate JWT with automatic JWKS rotation on failure.
    /// Returns (user_id, Option<new_auth>) — caller should swap in new_auth if Some.
    pub async fn validate_with_rotation(
        &self,
        token: &str,
    ) -> Result<(String, Option<Self>), anyhow::Error> {
        match self.validate(token) {
            Ok(sub) => Ok((sub, None)),
            Err(first_err) => {
                tracing::info!(
                    "Relay JWT validation failed, re-fetching JWKS (possible key rotation)"
                );
                match Self::from_supabase_url(&self.supabase_url).await {
                    Ok(new_auth) => {
                        let sub = new_auth.validate(token)?;
                        Ok((sub, Some(new_auth)))
                    }
                    Err(fetch_err) => {
                        tracing::error!("Relay JWKS re-fetch failed: {fetch_err}");
                        Err(first_err)
                    }
                }
            }
        }
    }
}
