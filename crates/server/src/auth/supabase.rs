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
    pub issuer: String,
    pub supabase_url: String,
}

pub fn validate_jwt(token: &str, cache: &JwksCache) -> anyhow::Result<AuthUser> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[&cache.issuer]);

    let data = decode::<Claims>(token, &cache.decoding_key, &validation)?;
    Ok(AuthUser {
        user_id: data.claims.sub,
        email: data.claims.email,
    })
}

pub fn extract_bearer(auth_header: &str) -> Option<&str> {
    auth_header.strip_prefix("Bearer ")
}

pub async fn fetch_decoding_key(supabase_url: &str) -> anyhow::Result<JwksCache> {
    let jwks_url = format!("{}/auth/v1/.well-known/jwks.json", supabase_url);
    let resp: serde_json::Value = reqwest::get(&jwks_url).await?.json().await?;

    let key_json = resp["keys"]
        .as_array()
        .and_then(|k| k.first())
        .ok_or_else(|| anyhow::anyhow!("Empty JWKS"))?;

    let jwk: jsonwebtoken::jwk::Jwk = serde_json::from_value(key_json.clone())?;
    let decoding_key = DecodingKey::from_jwk(&jwk)?;

    Ok(JwksCache {
        decoding_key,
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
