// crates/providers/src/parsers/zed.rs
//
// STUB — replaced by the Zed parser wave (see plan doc). Discovers
// nothing and refuses to parse, so the provider is simply absent until
// implemented (never wrong, per Trust over Accuracy).

use crate::discover::{DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::ForeignSession;
use std::path::Path;

pub struct ZedProvider;

impl Provider for ZedProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Zed
    }

    fn discover(&self, _root: &Path) -> Vec<DiscoveredSession> {
        Vec::new()
    }

    fn parse(&self, _path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        anyhow::bail!("{} parser not yet implemented", self.kind().as_str())
    }
}
