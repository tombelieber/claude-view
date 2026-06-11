// crates/providers/src/parsers/iflow.rs
//
// STUB — replaced by the Iflow parser wave (see plan doc). Discovers
// nothing and refuses to parse, so the provider is simply absent until
// implemented (never wrong, per Trust over Accuracy).

use crate::discover::{DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::ForeignSession;
use std::path::Path;

pub struct IflowProvider;

impl Provider for IflowProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Iflow
    }

    fn discover(&self, _root: &Path) -> Vec<DiscoveredSession> {
        Vec::new()
    }

    fn parse(&self, _path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        anyhow::bail!("{} parser not yet implemented", self.kind().as_str())
    }
}
