//! Shared credential loading — file + macOS Keychain fallback.
//!
//! Used by both `cli.rs` (auth detection) and `server/routes/oauth.rs` (usage API).

use serde::Deserialize;

/// Keychain service name used by Claude Code.
pub const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

/// Top-level `~/.claude/.credentials.json` (or Keychain equivalent).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialsFile {
    claude_ai_oauth: Option<OAuthSection>,
}

/// The `claudeAiOauth` section of the credentials.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthSection {
    #[serde(default)]
    pub access_token: String,
    /// Unix milliseconds
    #[serde(default)]
    pub expires_at: Option<u64>,
    #[serde(default)]
    pub subscription_type: Option<String>,
    // NOTE: refresh_token intentionally omitted — claude-view never refreshes tokens.
    // Token lifecycle is Claude Code's responsibility.
}

/// Check whether the token has expired (expiresAt is milliseconds since epoch).
pub fn is_token_expired(expires_at: Option<u64>) -> bool {
    let Some(exp) = expires_at else { return false };
    if exp == 0 {
        return false;
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    exp < now_ms
}

/// Parse credentials bytes into the OAuth section.
/// Returns `None` if missing, malformed, or access_token is empty.
pub fn parse_credentials(bytes: &[u8]) -> Option<OAuthSection> {
    let file: CredentialsFile = serde_json::from_slice(bytes).ok()?;
    let oauth = file.claude_ai_oauth?;
    if oauth.access_token.is_empty() {
        return None;
    }
    Some(oauth)
}

/// Read credentials JSON from macOS Keychain.
///
/// Returns raw JSON bytes. Handles both plain-text and hex-encoded
/// responses from the `security` command.
pub fn read_keychain_credentials() -> Option<Vec<u8>> {
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("security")
            .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if raw.is_empty() {
            return None;
        }

        // Try plain JSON first.
        if raw.starts_with('{') {
            return Some(raw.into_bytes());
        }

        // Try hex-decoding (macOS Keychain sometimes returns hex-encoded UTF-8).
        let hex = raw
            .strip_prefix("0x")
            .or(raw.strip_prefix("0X"))
            .unwrap_or(&raw);
        if !hex.len().is_multiple_of(2) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
            .collect();
        if bytes.is_empty() || bytes[0] != b'{' {
            return None;
        }
        Some(bytes)
    }
}

/// Load credentials bytes: file first, then macOS Keychain fallback.
pub fn load_credentials_bytes(home: &std::path::Path) -> Option<Vec<u8>> {
    let creds_path = home.join(".claude").join(".credentials.json");

    if let Ok(bytes) = std::fs::read(&creds_path) {
        tracing::debug!("Loaded credentials from file");
        return Some(bytes);
    }

    if let Some(bytes) = read_keychain_credentials() {
        tracing::debug!("Loaded credentials from macOS Keychain");
        return Some(bytes);
    }

    tracing::debug!("No credentials found (file or keychain)");
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_credentials_valid() {
        let json = br#"{"claudeAiOauth":{"accessToken":"sk-xxx","subscriptionType":"max","expiresAt":9999999999999}}"#;
        let creds = parse_credentials(json).unwrap();
        assert_eq!(creds.subscription_type.as_deref(), Some("max"));
        assert!(!creds.access_token.is_empty());
    }

    #[test]
    fn test_parse_credentials_no_oauth() {
        let json = br#"{"mcpOAuth":{}}"#;
        assert!(parse_credentials(json).is_none());
    }

    #[test]
    fn test_parse_credentials_empty_token() {
        let json = br#"{"claudeAiOauth":{"accessToken":"","subscriptionType":"max"}}"#;
        assert!(parse_credentials(json).is_none());
    }

    #[test]
    fn test_is_expired_future() {
        assert!(!is_token_expired(Some(9999999999999)));
    }

    #[test]
    fn test_is_expired_past() {
        assert!(is_token_expired(Some(1000)));
    }

    #[test]
    fn test_is_expired_none() {
        assert!(!is_token_expired(None));
    }

    #[test]
    fn test_is_expired_zero() {
        assert!(!is_token_expired(Some(0)));
    }
}
