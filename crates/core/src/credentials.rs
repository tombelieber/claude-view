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

/// Read credentials JSON from OS keychain.
///
/// - macOS: `security find-generic-password` (Keychain Access)
/// - Linux: `secret-tool lookup` (freedesktop.org Secret Service via D-Bus)
///
/// Returns raw JSON bytes. Handles both plain-text and hex-encoded
/// responses from the `security` command on macOS.
pub fn read_keychain_credentials() -> Option<Vec<u8>> {
    #[cfg(target_os = "macos")]
    {
        read_macos_keychain()
    }
    #[cfg(target_os = "linux")]
    {
        read_linux_secret_service()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn read_macos_keychain() -> Option<Vec<u8>> {
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
    decode_hex_json(&raw)
}

#[cfg(target_os = "linux")]
fn read_linux_secret_service() -> Option<Vec<u8>> {
    use std::time::{Duration, Instant};

    let mut child = std::process::Command::new("secret-tool")
        .args(["lookup", "service", KEYCHAIN_SERVICE])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + Duration::from_secs(3);
    let output = loop {
        match child.try_wait() {
            Ok(Some(_)) => break child.wait_with_output().ok()?,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    tracing::debug!("secret-tool timed out (no D-Bus session?)");
                    return None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return None,
        }
    };

    if !output.status.success() {
        tracing::debug!("secret-tool lookup failed (not installed or no matching entry)");
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() || !raw.starts_with('{') {
        return None;
    }

    Some(raw.into_bytes())
}

/// Decode hex-encoded JSON (macOS Keychain sometimes returns hex-encoded UTF-8).
fn decode_hex_json(raw: &str) -> Option<Vec<u8>> {
    let hex = raw
        .strip_prefix("0x")
        .or(raw.strip_prefix("0X"))
        .unwrap_or(raw);
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

/// Load credentials bytes: file first, then macOS Keychain fallback.
pub fn load_credentials_bytes(home: &std::path::Path) -> Option<Vec<u8>> {
    let creds_path = home.join(".claude").join(".credentials.json");

    if let Ok(bytes) = std::fs::read(&creds_path) {
        tracing::debug!("Loaded credentials from file");
        return Some(bytes);
    }

    if let Some(bytes) = read_keychain_credentials() {
        tracing::debug!("Loaded credentials from OS keychain");
        return Some(bytes);
    }

    tracing::debug!("No credentials found (file or keychain)");
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile;

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

    #[test]
    fn test_load_credentials_bytes_prefers_file_over_keychain() {
        let tmp = tempfile::tempdir().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let creds_path = claude_dir.join(".credentials.json");
        std::fs::write(
            &creds_path,
            r#"{"claudeAiOauth":{"accessToken":"file-token","expiresAt":9999999999999}}"#,
        )
        .unwrap();

        let result = load_credentials_bytes(tmp.path());
        assert!(result.is_some(), "file credentials must be found");

        let oauth = parse_credentials(&result.unwrap());
        assert!(oauth.is_some());
        assert_eq!(oauth.unwrap().access_token, "file-token");
    }

    #[test]
    fn test_load_credentials_bytes_returns_none_when_no_file_no_keychain() {
        let tmp = tempfile::tempdir().unwrap();
        let result = load_credentials_bytes(tmp.path());
        let _ = result;
    }

    #[test]
    fn test_decode_hex_json_valid_json() {
        let hex = "7b2261223a317d";
        let result = decode_hex_json(hex);
        assert!(result.is_some(), "valid hex-encoded JSON must decode");
        let bytes = result.unwrap();
        assert_eq!(bytes, b"{\"a\":1}");
    }

    #[test]
    fn test_decode_hex_json_with_0x_prefix() {
        let result = decode_hex_json("0x7b7d");
        assert!(result.is_some(), "0x-prefixed hex must decode");
        assert_eq!(result.unwrap(), b"{}");
    }

    #[test]
    fn test_decode_hex_json_rejects_odd_length() {
        let result = decode_hex_json("7b2");
        assert!(result.is_none(), "odd-length hex must be rejected");
    }

    #[test]
    fn test_decode_hex_json_rejects_non_hex_chars() {
        let result = decode_hex_json("zzzz");
        assert!(result.is_none(), "non-hex characters must be rejected");
    }

    #[test]
    fn test_decode_hex_json_rejects_non_json() {
        let result = decode_hex_json("48656c6c6f");
        assert!(
            result.is_none(),
            "hex that doesn't decode to JSON must be rejected"
        );
    }
}
