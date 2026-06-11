// crates/providers/src/parsers/claw.rs
//
// STUB — replaced by the generic claw-gateway parser (OpenClaw + QClaw share
// a byte-identical format; ONE parser parameterized by kind).

use crate::discover::{DiscoveredSession, Provider};
use crate::kind::ProviderKind;
use crate::model::ForeignSession;
use std::path::Path;

pub struct ClawProvider {
    kind: ProviderKind,
}

pub static OPENCLAW: ClawProvider = ClawProvider {
    kind: ProviderKind::Openclaw,
};
pub static QCLAW: ClawProvider = ClawProvider {
    kind: ProviderKind::Qclaw,
};

impl Provider for ClawProvider {
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
