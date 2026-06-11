// crates/providers/src/kind.rs
//
// ProviderKind — the closed set of supported foreign agents, plus the
// per-provider metadata the discovery layer needs (display name, session-id
// prefix, env-var override, default session roots).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A foreign AI coding agent whose sessions we can ingest.
///
/// Claude Code itself is NOT a variant: CC sessions flow through the existing
/// core pipeline untouched. Foreign session ids are namespaced
/// `<prefix>:<raw-id>`; CC ids stay bare (colon-free).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    Codex,
    Gemini,
    Copilot,
    Cursor,
    Opencode,
    Hermes,
    Amp,
    Qwen,
    Iflow,
    Openhands,
    Zencoder,
    Pi,
    Openclaw,
    Qclaw,
    Kimi,
    Commandcode,
    Cortex,
    Workbuddy,
    Zed,
    Forge,
    Piebald,
    Kiro,
    KiroIde,
    VscodeCopilot,
    Positron,
}

impl ProviderKind {
    /// Stable string id — used as the session-id prefix, the
    /// `SessionInfo.provider` value, and the frontend chip key.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Copilot => "copilot",
            Self::Cursor => "cursor",
            Self::Opencode => "opencode",
            Self::Hermes => "hermes",
            Self::Amp => "amp",
            Self::Qwen => "qwen",
            Self::Iflow => "iflow",
            Self::Openhands => "openhands",
            Self::Zencoder => "zencoder",
            Self::Pi => "pi",
            Self::Openclaw => "openclaw",
            Self::Qclaw => "qclaw",
            Self::Kimi => "kimi",
            Self::Commandcode => "commandcode",
            Self::Cortex => "cortex",
            Self::Workbuddy => "workbuddy",
            Self::Zed => "zed",
            Self::Forge => "forge",
            Self::Piebald => "piebald",
            Self::Kiro => "kiro",
            Self::KiroIde => "kiro-ide",
            Self::VscodeCopilot => "vscode-copilot",
            Self::Positron => "positron",
        }
    }

    /// Human-facing label for badges and filters.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Gemini => "Gemini CLI",
            Self::Copilot => "Copilot CLI",
            Self::Cursor => "Cursor",
            Self::Opencode => "OpenCode",
            Self::Hermes => "Hermes",
            Self::Amp => "Amp",
            Self::Qwen => "Qwen Code",
            Self::Iflow => "iFlow",
            Self::Openhands => "OpenHands",
            Self::Zencoder => "Zencoder",
            Self::Pi => "Pi",
            Self::Openclaw => "OpenClaw",
            Self::Qclaw => "QClaw",
            Self::Kimi => "Kimi",
            Self::Commandcode => "Command Code",
            Self::Cortex => "Cortex Code",
            Self::Workbuddy => "WorkBuddy",
            Self::Zed => "Zed",
            Self::Forge => "Forge",
            Self::Piebald => "Piebald",
            Self::Kiro => "Kiro CLI",
            Self::KiroIde => "Kiro IDE",
            Self::VscodeCopilot => "VS Code Copilot",
            Self::Positron => "Positron",
        }
    }

    /// Env var that overrides the default session root(s).
    pub fn env_var(self) -> &'static str {
        match self {
            Self::Codex => "CODEX_SESSIONS_DIR",
            Self::Gemini => "GEMINI_DIR",
            Self::Copilot => "COPILOT_DIR",
            Self::Cursor => "CURSOR_PROJECTS_DIR",
            Self::Opencode => "OPENCODE_DIR",
            Self::Hermes => "HERMES_SESSIONS_DIR",
            Self::Amp => "AMP_DIR",
            Self::Qwen => "QWEN_PROJECTS_DIR",
            Self::Iflow => "IFLOW_DIR",
            Self::Openhands => "OPENHANDS_CONVERSATIONS_DIR",
            Self::Zencoder => "ZENCODER_DIR",
            Self::Pi => "PI_DIR",
            Self::Openclaw => "OPENCLAW_DIR",
            Self::Qclaw => "QCLAW_DIR",
            Self::Kimi => "KIMI_DIR",
            Self::Commandcode => "COMMANDCODE_PROJECTS_DIR",
            Self::Cortex => "CORTEX_DIR",
            Self::Workbuddy => "WORKBUDDY_PROJECTS_DIR",
            Self::Zed => "ZED_DIR",
            Self::Forge => "FORGE_DIR",
            Self::Piebald => "PIEBALD_DIR",
            Self::Kiro => "KIRO_SESSIONS_DIR",
            Self::KiroIde => "KIRO_IDE_DIR",
            Self::VscodeCopilot => "VSCODE_COPILOT_DIR",
            Self::Positron => "POSITRON_DIR",
        }
    }

    /// Default session roots, relative to `$HOME` unless absolute.
    /// Multiple entries = platform variants or multi-generation stores;
    /// discovery probes each and uses whichever exists.
    pub fn default_roots(self) -> &'static [&'static str] {
        match self {
            Self::Codex => &[".codex/sessions", ".codex/archived_sessions"],
            Self::Gemini => &[".gemini"],
            Self::Copilot => &[".copilot"],
            Self::Cursor => &[".cursor/projects"],
            Self::Opencode => &[".local/share/opencode"],
            Self::Hermes => &[".hermes/sessions"],
            Self::Amp => &[".local/share/amp"],
            Self::Qwen => &[".qwen/projects"],
            Self::Iflow => &[".iflow/projects"],
            Self::Openhands => &[".openhands/conversations"],
            Self::Zencoder => &[".zencoder/sessions"],
            Self::Pi => &[".pi/agent/sessions"],
            Self::Openclaw => &[".openclaw/agents"],
            Self::Qclaw => &[".qclaw/agents"],
            Self::Kimi => &[".kimi/sessions"],
            Self::Commandcode => &[".commandcode/projects"],
            Self::Cortex => &[".snowflake/cortex/conversations"],
            Self::Workbuddy => &[".workbuddy/projects"],
            #[cfg(target_os = "macos")]
            Self::Zed => &["Library/Application Support/Zed"],
            #[cfg(not(target_os = "macos"))]
            Self::Zed => &[".local/share/zed"],
            Self::Forge => &[".forge"],
            #[cfg(target_os = "macos")]
            Self::Piebald => &["Library/Application Support/piebald"],
            #[cfg(not(target_os = "macos"))]
            Self::Piebald => &[".local/share/piebald"],
            Self::Kiro => &[".kiro/sessions/cli", ".local/share/kiro-cli"],
            #[cfg(target_os = "macos")]
            Self::KiroIde => &["Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent"],
            #[cfg(not(target_os = "macos"))]
            Self::KiroIde => &[".config/Kiro/User/globalStorage/kiro.kiroagent"],
            #[cfg(target_os = "macos")]
            Self::VscodeCopilot => &["Library/Application Support/Code/User"],
            #[cfg(not(target_os = "macos"))]
            Self::VscodeCopilot => &[".config/Code/User"],
            #[cfg(target_os = "macos")]
            Self::Positron => &["Library/Application Support/Positron/User"],
            #[cfg(not(target_os = "macos"))]
            Self::Positron => &[".config/Positron/User"],
        }
    }

    /// Resolve session roots: env override (single dir) wins, else the
    /// default roots joined onto `$HOME`. Only existing dirs are returned.
    pub fn session_roots(self) -> Vec<PathBuf> {
        if let Ok(v) = std::env::var(self.env_var()) {
            if !v.is_empty() {
                let p = PathBuf::from(v);
                return if p.exists() { vec![p] } else { Vec::new() };
            }
        }
        let Some(home) = dirs::home_dir() else {
            return Vec::new();
        };
        self.default_roots()
            .iter()
            .map(|rel| home.join(rel))
            .filter(|p| p.exists())
            .collect()
    }

    /// Parse a namespaced session id (`<prefix>:<raw>`). Returns `None` for
    /// bare (Claude Code) ids or unknown prefixes.
    pub fn from_session_id(id: &str) -> Option<(Self, &str)> {
        let (prefix, raw) = id.split_once(':')?;
        let kind = ALL.iter().copied().find(|k| k.as_str() == prefix)?;
        Some((kind, raw))
    }

    /// Build the namespaced session id for a raw provider-local id.
    pub fn session_id(self, raw: &str) -> String {
        format!("{}:{}", self.as_str(), raw)
    }
}

/// All supported providers (matrix order: wave 1, 2, 3).
pub const ALL: &[ProviderKind] = &[
    ProviderKind::Codex,
    ProviderKind::Gemini,
    ProviderKind::Copilot,
    ProviderKind::Cursor,
    ProviderKind::Opencode,
    ProviderKind::Hermes,
    ProviderKind::Amp,
    ProviderKind::Qwen,
    ProviderKind::Iflow,
    ProviderKind::Openhands,
    ProviderKind::Zencoder,
    ProviderKind::Pi,
    ProviderKind::Openclaw,
    ProviderKind::Qclaw,
    ProviderKind::Kimi,
    ProviderKind::Commandcode,
    ProviderKind::Cortex,
    ProviderKind::Workbuddy,
    ProviderKind::Zed,
    ProviderKind::Forge,
    ProviderKind::Piebald,
    ProviderKind::Kiro,
    ProviderKind::KiroIde,
    ProviderKind::VscodeCopilot,
    ProviderKind::Positron,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_roundtrip() {
        let id = ProviderKind::Codex.session_id("abc-123");
        assert_eq!(id, "codex:abc-123");
        let (kind, raw) = ProviderKind::from_session_id(&id).unwrap();
        assert_eq!(kind, ProviderKind::Codex);
        assert_eq!(raw, "abc-123");
    }

    #[test]
    fn bare_ids_are_not_foreign() {
        assert!(ProviderKind::from_session_id("8f3a2b").is_none());
        assert!(ProviderKind::from_session_id("not-a-provider:x").is_none());
    }

    #[test]
    fn ids_are_unique_and_kebab() {
        let mut seen = std::collections::HashSet::new();
        for k in ALL {
            assert!(seen.insert(k.as_str()), "duplicate id {}", k.as_str());
            assert!(!k.as_str().contains(':'));
            assert_eq!(k.as_str(), k.as_str().to_lowercase());
        }
    }
}
