use base64::Engine;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub enum WebhookEventType {
    #[serde(rename = "session.started")]
    SessionStarted,
    #[serde(rename = "session.ended")]
    SessionEnded,
    #[serde(rename = "session.error")]
    SessionError,
    #[serde(rename = "session.updated")]
    SessionUpdated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum WebhookFormat {
    Raw,
    Lark,
    Slack,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebhookConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub format: WebhookFormat,
    pub events: Vec<WebhookEventType>,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default)]
    pub webhooks: Vec<WebhookConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebhookSecrets {
    #[serde(default)]
    pub secrets: std::collections::HashMap<String, String>,
}

// ── Functions ─────────────────────────────────────────────────────────────────

/// Generate a webhook signing secret with the `whsec_` prefix.
///
/// Uses 32 cryptographically random bytes encoded as base64.
pub fn generate_signing_secret() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen::<u8>()).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    format!("whsec_{encoded}")
}

/// Generate a unique webhook ID with the `wh_` prefix.
///
/// Uses 12 random alphanumeric characters.
pub fn generate_webhook_id() -> String {
    let mut rng = rand::thread_rng();
    let suffix: String = (0..12)
        .map(|_| {
            let idx = rng.gen_range(0..36usize);
            if idx < 10 {
                (b'0' + idx as u8) as char
            } else {
                (b'a' + (idx - 10) as u8) as char
            }
        })
        .collect();
    format!("wh_{suffix}")
}

/// Load `NotificationsConfig` from a JSON file.
///
/// Returns an empty default if the file does not exist or cannot be parsed.
pub fn load_config(path: &PathBuf) -> NotificationsConfig {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => NotificationsConfig::default(),
    }
}

/// Persist `NotificationsConfig` to a JSON file.
///
/// Creates parent directories as needed.
pub fn save_config(config: &NotificationsConfig, path: &PathBuf) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, json)
}

/// Load `WebhookSecrets` from a JSON file.
///
/// Returns an empty default if the file does not exist or cannot be parsed.
pub fn load_secrets(path: &PathBuf) -> WebhookSecrets {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => WebhookSecrets::default(),
    }
}

/// Persist `WebhookSecrets` to a JSON file.
///
/// Creates parent directories as needed.
pub fn save_secrets(secrets: &WebhookSecrets, path: &PathBuf) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(secrets)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, json)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn signing_secret_has_whsec_prefix() {
        let secret = generate_signing_secret();
        assert!(
            secret.starts_with("whsec_"),
            "expected whsec_ prefix, got: {secret}"
        );
        assert!(
            secret.len() > 20,
            "expected len > 20, got: {}",
            secret.len()
        );
    }

    #[test]
    fn webhook_id_has_wh_prefix() {
        let id = generate_webhook_id();
        assert!(id.starts_with("wh_"), "expected wh_ prefix, got: {id}");
        assert_eq!(
            id.len(),
            15, // "wh_" (3) + 12 alphanumeric chars
            "expected len 15, got: {}",
            id.len()
        );
    }

    #[test]
    fn config_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("notifications.json");

        let config = NotificationsConfig {
            base_url: Some("https://example.com".to_string()),
            webhooks: vec![WebhookConfig {
                id: "wh_abc123456789".to_string(),
                name: "Test Webhook".to_string(),
                url: "https://example.com/hook".to_string(),
                format: WebhookFormat::Raw,
                events: vec![
                    WebhookEventType::SessionStarted,
                    WebhookEventType::SessionEnded,
                ],
                enabled: true,
                created_at: "2026-04-10T00:00:00Z".to_string(),
            }],
        };

        save_config(&config, &path).unwrap();
        let loaded = load_config(&path);

        assert_eq!(loaded.base_url, config.base_url);
        assert_eq!(loaded.webhooks.len(), 1);
        assert_eq!(loaded.webhooks[0].id, "wh_abc123456789");
        assert_eq!(loaded.webhooks[0].name, "Test Webhook");
        assert_eq!(loaded.webhooks[0].format, WebhookFormat::Raw);
        assert!(loaded.webhooks[0].enabled);
        assert_eq!(loaded.webhooks[0].events.len(), 2);
        assert_eq!(
            loaded.webhooks[0].events[0],
            WebhookEventType::SessionStarted
        );
    }

    #[test]
    fn secrets_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("secrets.json");

        let mut secrets = WebhookSecrets::default();
        secrets
            .secrets
            .insert("wh_abc".to_string(), "whsec_somesecret".to_string());
        secrets
            .secrets
            .insert("wh_xyz".to_string(), "whsec_anothersecret".to_string());

        save_secrets(&secrets, &path).unwrap();
        let loaded = load_secrets(&path);

        assert_eq!(loaded.secrets.len(), 2);
        assert_eq!(loaded.secrets.get("wh_abc").unwrap(), "whsec_somesecret");
        assert_eq!(loaded.secrets.get("wh_xyz").unwrap(), "whsec_anothersecret");
    }

    #[test]
    fn load_missing_config_returns_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");

        let config = load_config(&path);
        assert!(config.webhooks.is_empty());
        assert!(config.base_url.is_none());
    }

    #[test]
    fn load_missing_secrets_returns_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");

        let secrets = load_secrets(&path);
        assert!(secrets.secrets.is_empty());
    }

    #[test]
    fn webhook_event_type_serialization() {
        let event = WebhookEventType::SessionStarted;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#""session.started""#);

        let event = WebhookEventType::SessionEnded;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#""session.ended""#);

        let event = WebhookEventType::SessionError;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#""session.error""#);

        let event = WebhookEventType::SessionUpdated;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#""session.updated""#);
    }

    #[test]
    fn two_secrets_are_unique() {
        let s1 = generate_signing_secret();
        let s2 = generate_signing_secret();
        assert_ne!(s1, s2, "two generated secrets should be different");
    }
}
