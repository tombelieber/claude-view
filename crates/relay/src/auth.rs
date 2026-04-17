//! JWT validation for the relay.
//!
//! Phase 1 stripped out the Ed25519 challenge-response auth (that was for the
//! pre-Supabase pairing model, now replaced by JWT + device ownership via
//! Supabase). The Ed25519 helpers remain exported for the ws/client path, but
//! the live WS handshake uses Supabase JWTs validated against the JWKS.

use std::sync::Arc;

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

// --- Supabase JWT validation ---

#[derive(Debug, Deserialize)]
pub struct SupabaseClaims {
    pub sub: String, // user_id
    pub exp: u64,
    pub iss: String,
}

/// Supabase JWT validator. In tests we substitute a no-verify mock — see
/// `SupabaseAuth::mock_for_test()`.
pub struct SupabaseAuth {
    pub kind: SupabaseAuthKind,
}

pub enum SupabaseAuthKind {
    /// Real validator with a JWKS-provided decoding key.
    Real {
        decoding_key: DecodingKey,
        algorithm: Algorithm,
        issuer: String,
        supabase_url: String,
    },
    /// Test mock — base64-decodes the payload and trusts the `sub` field
    /// without verifying the signature. Only used in unit/integration tests.
    /// Only constructed via `mock_for_test()` — safe to keep in prod builds
    /// since nothing constructs it there.
    Mock,
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
            kind: SupabaseAuthKind::Real {
                decoding_key,
                algorithm,
                issuer: format!("{}/auth/v1", supabase_url),
                supabase_url: supabase_url.to_string(),
            },
        })
    }

    /// Test-only mock: trusts any JWT whose payload decodes to `{"sub": "..."}`.
    /// Always available (no feature gate) so integration tests in `tests/`
    /// can wire it without a crate-self dev-dependency dance.
    pub fn mock_for_test() -> Self {
        Self {
            kind: SupabaseAuthKind::Mock,
        }
    }

    pub fn validate(&self, token: &str) -> anyhow::Result<String> {
        match &self.kind {
            SupabaseAuthKind::Real {
                decoding_key,
                algorithm,
                issuer,
                ..
            } => {
                let mut v = Validation::new(*algorithm);
                v.set_issuer(&[issuer]);
                v.set_audience(&["authenticated"]);
                let data = decode::<SupabaseClaims>(token, decoding_key, &v)?;
                Ok(data.claims.sub)
            }
            SupabaseAuthKind::Mock => {
                // Parse the JWT payload manually, no signature verification.
                use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
                let mut parts = token.split('.');
                let _ = parts.next().ok_or_else(|| anyhow::anyhow!("no header"))?;
                let payload_b64 = parts.next().ok_or_else(|| anyhow::anyhow!("no payload"))?;
                let payload_bytes = URL_SAFE_NO_PAD
                    .decode(payload_b64)
                    .map_err(|e| anyhow::anyhow!("bad payload b64: {e}"))?;
                let payload: serde_json::Value = serde_json::from_slice(&payload_bytes)
                    .map_err(|e| anyhow::anyhow!("bad payload json: {e}"))?;
                let sub = payload
                    .get("sub")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("no sub claim"))?
                    .to_string();
                Ok(sub)
            }
        }
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
                let supabase_url = match &self.kind {
                    SupabaseAuthKind::Real { supabase_url, .. } => supabase_url.clone(),
                    SupabaseAuthKind::Mock => return Err(first_err),
                };
                tracing::info!(
                    "Relay JWT validation failed, re-fetching JWKS (possible key rotation)"
                );
                match Self::from_supabase_url(&supabase_url).await {
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

/// Free-function wrapper: validate a JWT and return the user_id (sub claim).
///
/// Returns Err if `supabase_auth` is None (auth required but not configured)
/// or if the JWT fails validation.
pub fn validate_jwt(
    jwt: &str,
    supabase_auth: Option<&Arc<SupabaseAuth>>,
) -> Result<String, String> {
    let auth = supabase_auth.ok_or_else(|| "Supabase auth not configured".to_string())?;
    auth.validate(jwt).map_err(|e| e.to_string())
}
