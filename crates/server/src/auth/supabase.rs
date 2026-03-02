use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    pub exp: u64,
    pub iss: String,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
    pub email: Option<String>,
}

pub struct JwksCache {
    pub decoding_key: DecodingKey,
    pub algorithm: Algorithm,
    pub issuer: String,
    pub supabase_url: String,
}

pub fn validate_jwt(token: &str, cache: &JwksCache) -> anyhow::Result<AuthUser> {
    let mut validation = Validation::new(cache.algorithm);
    validation.set_issuer(&[&cache.issuer]);
    // Supabase JWTs use aud="authenticated" for logged-in users
    validation.set_audience(&["authenticated"]);

    let data = decode::<Claims>(token, &cache.decoding_key, &validation)?;
    Ok(AuthUser {
        user_id: data.claims.sub,
        email: data.claims.email,
    })
}

pub fn extract_bearer(auth_header: &str) -> Option<&str> {
    auth_header.strip_prefix("Bearer ")
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

pub async fn fetch_decoding_key(supabase_url: &str) -> anyhow::Result<JwksCache> {
    let jwks_url = format!("{}/auth/v1/.well-known/jwks.json", supabase_url);
    let resp: serde_json::Value = reqwest::get(&jwks_url).await?.json().await?;

    let key_json = resp["keys"]
        .as_array()
        .and_then(|k| k.first())
        .ok_or_else(|| anyhow::anyhow!("Empty JWKS"))?;

    let algorithm = jwk_algorithm(key_json)?;
    let jwk: jsonwebtoken::jwk::Jwk = serde_json::from_value(key_json.clone())?;
    let decoding_key = DecodingKey::from_jwk(&jwk)?;

    tracing::info!("JWKS loaded: algorithm={algorithm:?}");

    Ok(JwksCache {
        decoding_key,
        algorithm,
        issuer: format!("{}/auth/v1", supabase_url),
        supabase_url: supabase_url.to_string(),
    })
}

pub async fn validate_jwt_with_rotation(
    token: &str,
    cache: &JwksCache,
) -> Result<(AuthUser, Option<JwksCache>), anyhow::Error> {
    match validate_jwt(token, cache) {
        Ok(user) => Ok((user, None)),
        Err(first_err) => {
            tracing::info!("JWT validation failed, re-fetching JWKS (possible key rotation)");
            match fetch_decoding_key(&cache.supabase_url).await {
                Ok(new_cache) => {
                    let user = validate_jwt(token, &new_cache)?;
                    Ok((user, Some(new_cache)))
                }
                Err(fetch_err) => {
                    tracing::error!("JWKS re-fetch failed: {fetch_err}");
                    Err(first_err)
                }
            }
        }
    }
}
