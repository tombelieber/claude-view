use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentMode {
    Dev,
    NpxInstall,
    DockerImage,
}

impl DeploymentMode {
    pub fn from_env() -> Self {
        match std::env::var("CLAUDE_VIEW_DEPLOYMENT_MODE").as_deref() {
            Ok("npx") | Ok("npx-install") => Self::NpxInstall,
            Ok("docker") | Ok("docker-image") => Self::DockerImage,
            _ => Self::Dev,
        }
    }
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

impl ServiceConfig {
    pub fn new(service_name: &'static str, service_version: &'static str) -> Self {
        let deployment_mode = DeploymentMode::from_env();
        Self {
            service_name,
            service_version,
            build_sha: option_env!("CLAUDE_VIEW_BUILD_SHA").unwrap_or("dev"),
            log_dir: claude_view_core::paths::log_dir(),
            default_filter: format!(
                "info,{}=debug,sqlx=warn,hyper=warn,tower_http=info,tantivy=warn",
                service_name.replace('-', "_")
            ),
            sink_mode: SinkMode::Both,
            deployment_mode,
            otel_endpoint: std::env::var("CLAUDE_VIEW_OTLP_ENDPOINT").ok(),
            sentry_dsn: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_dev_mode() {
        std::env::remove_var("CLAUDE_VIEW_DEPLOYMENT_MODE");
        std::env::remove_var("CLAUDE_VIEW_OTLP_ENDPOINT");
        let cfg = ServiceConfig::new("claude-view-server", "0.37.1");
        assert_eq!(cfg.service_name, "claude-view-server");
        assert_eq!(cfg.deployment_mode, DeploymentMode::Dev);
        assert_eq!(cfg.sink_mode, SinkMode::Both);
        assert!(cfg.default_filter.contains("claude_view_server=debug"));
        assert!(cfg.otel_endpoint.is_none());
    }
}
