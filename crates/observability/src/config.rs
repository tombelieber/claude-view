use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentMode {
    Dev,
    NpxInstall,
    DockerImage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkMode {
    DevOnly,
    ProdOnly,
    Both,
}

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub service_name: &'static str,
    pub service_version: &'static str,
    pub build_sha: &'static str,
    pub log_dir: PathBuf,
    pub default_filter: String,
    pub sink_mode: SinkMode,
    pub deployment_mode: DeploymentMode,
    pub otel_endpoint: Option<String>,
    pub sentry_dsn: Option<String>,
}
