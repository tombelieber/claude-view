// crates/providers/src/parsers/vscode_copilot.rs
//
// STUB — replaced by the VSCode Copilot parser wave. Positron shares the
// byte-identical chat-session format (different root + id prefix), so both
// providers live here parameterized by kind.

use crate::discover::{DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::ForeignSession;
use std::path::Path;

pub struct VscodeChatProvider {
    kind: ProviderKind,
}

pub static VSCODE_COPILOT: VscodeChatProvider = VscodeChatProvider {
    kind: ProviderKind::VscodeCopilot,
};
pub static POSITRON: VscodeChatProvider = VscodeChatProvider {
    kind: ProviderKind::Positron,
};

impl Provider for VscodeChatProvider {
    fn kind(&self) -> ProviderKind {
        self.kind
    }

    fn discover(&self, _root: &Path) -> Vec<DiscoveredSession> {
        Vec::new()
    }

    fn parse(&self, _path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        anyhow::bail!("{} parser not yet implemented", self.kind.as_str())
    }
}
