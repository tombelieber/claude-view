pub struct SentryGuard(pub(crate) sentry::ClientInitGuard);

pub fn init_if_enabled(
    _dsn: Option<String>,
    _service_name: &'static str,
    _mode: &crate::config::DeploymentMode,
) -> Option<SentryGuard> {
    None // Implemented fully in Task 11
}
