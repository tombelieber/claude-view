// crates/providers/src/discover.rs
//
// Provider trait + discovery types + the static registry.
//
// Discovery is cheap (stat-level walks, no parsing); parsing is on-demand
// and cached by the catalog keyed on (path, mtime, size).

use crate::kind::ProviderKind;
use crate::model::ForeignSession;
use std::path::{Path, PathBuf};

/// A session located on disk, before parsing.
#[derive(Debug, Clone)]
pub struct DiscoveredSession {
    /// Namespaced id `<provider>:<raw>`.
    pub id: String,
    pub provider: ProviderKind,
    /// Real file path, or `<db>#<raw-id>` virtual path (SQLite providers).
    pub path: PathBuf,
    /// Best-effort project hint from the directory layout (parsers may refine).
    pub project_hint: Option<String>,
    /// Epoch seconds of last modification.
    pub mtime: f64,
    pub size_bytes: u64,
}

/// One foreign agent integration: discovery + parsing.
///
/// Implementations are stateless; `parse` returns a Vec because some stores
/// hold multiple sessions per backing file (SQLite DBs, forked transcripts).
pub trait Provider: Send + Sync {
    fn kind(&self) -> ProviderKind;

    /// Enumerate sessions under one root (a dir from
    /// `ProviderKind::session_roots`). Must not parse transcripts.
    fn discover(&self, root: &Path) -> Vec<DiscoveredSession>;

    /// Parse one discovered session source into normalized form.
    /// `path` is the `DiscoveredSession.path` (may be a `db#id` virtual path).
    fn parse(&self, path: &Path) -> anyhow::Result<Vec<ForeignSession>>;
}

/// Split a virtual path `<db>#<raw-id>` into its parts. Returns `None` for
/// plain file paths.
pub fn split_virtual_path(path: &Path) -> Option<(PathBuf, String)> {
    let s = path.to_str()?;
    let (db, id) = s.rsplit_once('#')?;
    Some((PathBuf::from(db), id.to_string()))
}

/// File metadata as (mtime epoch seconds, size). Returns `None` when the
/// file vanished between discovery and stat.
pub fn stat_entry(path: &Path) -> Option<(f64, u64)> {
    let md = std::fs::metadata(path).ok()?;
    let mtime = md
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs_f64();
    Some((mtime, md.len()))
}

/// All registered providers. Order = provider matrix order.
pub fn registry() -> &'static [&'static dyn Provider] {
    static REGISTRY: &[&dyn Provider] = &[
        &crate::parsers::amp::AmpProvider,
        &crate::parsers::codex::CodexProvider,
        &crate::parsers::gemini::GeminiProvider,
        &crate::parsers::copilot::CopilotProvider,
        &crate::parsers::cursor::CursorProvider,
        &crate::parsers::opencode::OpencodeProvider,
        &crate::parsers::hermes::HermesProvider,
        &crate::parsers::qwen::QwenProvider,
        &crate::parsers::iflow::IflowProvider,
        &crate::parsers::openhands::OpenhandsProvider,
        &crate::parsers::zencoder::ZencoderProvider,
        &crate::parsers::pi::PiProvider,
        &crate::parsers::claw::OPENCLAW,
        &crate::parsers::claw::QCLAW,
        &crate::parsers::kimi::KimiProvider,
        &crate::parsers::commandcode::CommandcodeProvider,
        &crate::parsers::cortex::CortexProvider,
        &crate::parsers::workbuddy::WorkbuddyProvider,
        &crate::parsers::zed::ZedProvider,
        &crate::parsers::forge::ForgeProvider,
        &crate::parsers::piebald::PiebaldProvider,
        &crate::parsers::kiro::KiroProvider,
        &crate::parsers::kiro_ide::KiroIdeProvider,
        &crate::parsers::vscode_copilot::VSCODE_COPILOT,
        &crate::parsers::vscode_copilot::POSITRON,
    ];
    REGISTRY
}

/// Find the provider implementation for a kind.
pub fn provider_for(kind: ProviderKind) -> Option<&'static dyn Provider> {
    registry().iter().copied().find(|p| p.kind() == kind)
}
