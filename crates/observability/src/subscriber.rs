use crate::config::ServiceConfig;

pub struct ObservabilityHandle;

pub fn init(_cfg: ServiceConfig) -> anyhow::Result<ObservabilityHandle> {
    Ok(ObservabilityHandle)
}
