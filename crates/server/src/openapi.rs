use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "claude-view API",
        description = "Mission Control for Claude Code",
        version = env!("CARGO_PKG_VERSION"),
        license(name = "MIT"),
    ),
    tags(
        (name = "health", description = "Health checks and server status"),
        (name = "sessions", description = "Session CRUD, filtering, and export"),
        (name = "projects", description = "Project summaries and branches"),
        (name = "live", description = "Live session monitoring and control"),
        (name = "monitor", description = "System resource monitoring"),
        (name = "insights", description = "Behavioral insights and patterns"),
        (name = "contributions", description = "Contribution metrics"),
        (name = "stats", description = "Dashboard statistics and trends"),
        (name = "search", description = "Full-text search"),
        (name = "turns", description = "Per-turn session breakdown"),
        (name = "plans", description = "Session plans"),
        (name = "coaching", description = "Coaching rules management"),
        (name = "facets", description = "Session quality facets"),
        (name = "classify", description = "Session classification"),
        (name = "models", description = "Observed model usage"),
        (name = "export", description = "Data export (JSON/CSV)"),
        (name = "share", description = "Session sharing"),
        (name = "teams", description = "Team management"),
        (name = "plugins", description = "Plugin management"),
        (name = "oauth", description = "OAuth usage and identity"),
        (name = "settings", description = "App settings"),
        (name = "system", description = "System operations"),
        (name = "sync", description = "Git sync and indexing"),
        (name = "reports", description = "Generated reports"),
        (name = "prompts", description = "Prompt history and templates"),
        (name = "workflows", description = "Workflow management"),
        (name = "ide", description = "IDE integration"),
        (name = "pairing", description = "Device pairing"),
        (name = "jobs", description = "Background job tracking"),
        (name = "telemetry", description = "Telemetry consent"),
        (name = "processes", description = "Process management"),
    )
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_openapi_spec() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_string_pretty(&spec).unwrap();
        let out_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/plugin/scripts/openapi.json"
        );
        std::fs::write(out_path, json).unwrap();
        println!("OpenAPI spec written to {out_path}");
    }
}
