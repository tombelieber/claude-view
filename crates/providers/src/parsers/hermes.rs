// crates/providers/src/parsers/hermes.rs
//
// STUB — replaced by the Hermes parser wave (see plan doc). Discovers
// nothing and refuses to parse, so the provider is simply absent until
// implemented (never wrong, per Trust over Accuracy).

use crate::discover::{DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::ForeignSession;
use std::path::Path;

pub struct HermesProvider;

impl Provider for HermesProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Hermes
    }

    fn discover(&self, _root: &Path) -> Vec<DiscoveredSession> {
        Vec::new()
    }

    fn parse(&self, _path: &Path) -> anyhow::Result<Vec<ForeignSession>> {
        anyhow::bail!("{} parser not yet implemented", self.kind().as_str())
    }
}
