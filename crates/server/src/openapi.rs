use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "claude-view API",
        description = "Mission Control for Claude Code",
        version = env!("CARGO_PKG_VERSION"),
        license(name = "MIT"),
    ),
    paths(
        crate::routes::health::health_check,
        crate::routes::config::config,
        crate::routes::status::get_status,
        crate::routes::status::update_git_sync_interval,
        // Batch 1: Sessions
        crate::routes::sessions::list_sessions,
        crate::routes::sessions::get_session_detail,
        crate::routes::sessions::get_session_parsed,
        crate::routes::sessions::get_session_messages_by_id,
        crate::routes::sessions::get_session_rich,
        crate::routes::sessions::list_branches,
        crate::routes::sessions::session_activity,
        crate::routes::sessions::estimate_cost,
        crate::routes::sessions::archive_session_handler,
        crate::routes::sessions::unarchive_session_handler,
        crate::routes::sessions::bulk_archive_handler,
        crate::routes::sessions::bulk_unarchive_handler,
        crate::routes::sessions::get_session_hook_events,
        // Batch 1: Projects
        crate::routes::projects::list_projects,
        crate::routes::projects::list_project_sessions,
        crate::routes::projects::list_project_branches,
        // Batch 1: File History
        crate::routes::file_history::get_file_history,
        crate::routes::file_history::get_file_diff,
        // Batch 2: Live Monitoring
        crate::routes::live::live_stream,
        crate::routes::live::list_live_sessions,
        crate::routes::live::get_live_session,
        crate::routes::live::get_live_session_messages,
        crate::routes::live::get_session_statusline_debug,
        crate::routes::live::kill_session,
        crate::routes::live::bind_control,
        crate::routes::live::unbind_control,
        crate::routes::live::dismiss_session,
        crate::routes::live::dismiss_all_closed,
        crate::routes::live::get_live_summary,
        crate::routes::live::get_pricing,
        crate::routes::hooks::handle_hook,
        crate::routes::statusline::handle_statusline,
        crate::routes::monitor::monitor_stream,
        crate::routes::monitor::monitor_snapshot,
    ),
    components(schemas(
        crate::routes::health::HealthResponse,
        crate::routes::config::ConfigResponse,
        claude_view_core::telemetry_config::TelemetryStatus,
        claude_view_db::trends::IndexMetadata,
        crate::routes::status::UpdateGitSyncIntervalRequest,
        // Batch 1: Sessions schemas
        crate::routes::sessions::SessionsListResponse,
        crate::routes::sessions::SessionActivityResponse,
        crate::routes::sessions::SessionDetail,
        crate::routes::sessions::CommitWithTier,
        crate::routes::sessions::DerivedMetrics,
        crate::routes::sessions::EstimateRequest,
        crate::routes::sessions::CostEstimate,
        claude_view_core::SessionInfo,
        claude_view_core::ToolCounts,
        claude_view_core::task_files::TaskItem,
        // Batch 1: Projects schemas
        crate::routes::projects::BranchesResponse,
        claude_view_core::ProjectSummary,
        claude_view_core::SessionsPage,
        claude_view_db::BranchCount,
        // Batch 1: File History schemas
        claude_view_core::file_history::FileHistoryResponse,
        claude_view_core::file_history::FileChange,
        claude_view_core::file_history::FileVersion,
        claude_view_core::file_history::DiffStats,
        claude_view_core::file_history::DiffSummary,
        claude_view_core::file_history::FileDiffResponse,
        claude_view_core::file_history::DiffHunk,
        claude_view_core::file_history::DiffLine,
        claude_view_core::file_history::DiffLineKind,
        claude_view_db::ActivityPoint,
        // Batch 2: Live/Monitor schemas
        crate::routes::hooks::HookPayload,
        crate::routes::statusline::StatuslinePayload,
        crate::live::monitor::ResourceSnapshot,
        crate::live::monitor::SystemInfo,
        crate::live::monitor::ProcessGroup,
        crate::live::monitor::SessionResource,
    )),
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
