// crates/server/src/routes/coaching.rs
//! Coaching rules API routes.
//!
//! Manages coaching rule files in `~/.claude/rules/coaching-*.md`.
//! Rules are generated from behavioral pattern insights and written
//! as Markdown files that Claude Code can pick up as custom instructions.
//!
//! - GET    /coaching/rules      — List all coaching rules
//! - POST   /coaching/rules      — Apply (create) a coaching rule
//! - DELETE  /coaching/rules/{id} — Remove a coaching rule

use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use ts_rs::TS;

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of coaching rules allowed at once.
const MAX_RULES: usize = 8;

// ============================================================================
// Request / Response Types
// ============================================================================

/// Request body for POST /api/coaching/rules.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyRuleRequest {
    pub pattern_id: String,
    pub recommendation: String,
    pub title: String,
    pub impact_score: f64,
    pub sample_size: usize,
    pub scope: String, // "user" | "project"
}

/// A coaching rule parsed from a `coaching-*.md` file.
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(Deserialize))]
pub struct CoachingRule {
    pub id: String,
    pub pattern_id: String,
    pub title: String,
    pub body: String,
    pub scope: String,
    pub applied_at: String,
    pub file_path: String,
}

/// Response for GET /api/coaching/rules.
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(Deserialize))]
pub struct ListRulesResponse {
    pub rules: Vec<CoachingRule>,
    pub count: usize,
    pub max_rules: usize,
}

/// Response for DELETE /api/coaching/rules/{id}.
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(Deserialize))]
pub struct RemoveRuleResponse {
    pub removed: bool,
    pub id: String,
}

// ============================================================================
// Router
// ============================================================================

/// Create the coaching routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/coaching/rules", get(list_rules))
        .route("/coaching/rules", post(apply_rule))
        .route("/coaching/rules/{id}", delete(remove_rule))
}

// ============================================================================
// Validation
// ============================================================================

/// Validate a pattern ID: non-empty, max 10 chars, starts with alpha,
/// only alphanumeric and hyphens.
fn is_valid_pattern_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 10
        && id.starts_with(|c: char| c.is_ascii_alphabetic())
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Map an impact score (0.0–1.0) to a confidence label.
fn confidence_label(impact: f64) -> &'static str {
    if impact >= 0.7 {
        "high confidence"
    } else if impact >= 0.4 {
        "moderate confidence"
    } else {
        "low confidence"
    }
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/coaching/rules — List all coaching rules from the rules directory.
async fn list_rules(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ListRulesResponse>> {
    let rules = read_all_rules(&state.rules_dir);
    let count = rules.len();
    Ok(Json(ListRulesResponse {
        rules,
        count,
        max_rules: MAX_RULES,
    }))
}

/// POST /api/coaching/rules — Create a new coaching rule file.
async fn apply_rule(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApplyRuleRequest>,
) -> ApiResult<Json<CoachingRule>> {
    // Validate pattern ID
    if !is_valid_pattern_id(&req.pattern_id) {
        return Err(ApiError::BadRequest("Invalid pattern ID".to_string()));
    }

    // Check budget (only count existing rules, excluding the one we might overwrite)
    let existing_rules = read_all_rules(&state.rules_dir);
    let is_overwrite = existing_rules.iter().any(|r| r.id == req.pattern_id);
    if !is_overwrite && existing_rules.len() >= MAX_RULES {
        return Err(ApiError::Conflict(
            "Maximum 8 coaching rules. Remove one first.".to_string(),
        ));
    }

    // Ensure directory exists
    std::fs::create_dir_all(&state.rules_dir).map_err(|e| {
        tracing::error!(path = %state.rules_dir.display(), error = %e, "Failed to create rules directory");
        ApiError::Internal(format!("Failed to create rules directory: {}", e))
    })?;

    // Build file content
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let confidence = confidence_label(req.impact_score);
    let impact_pct = (req.impact_score * 100.0).round() as i64;
    let content = format!(
        "---\n\
         # Auto-generated by claude-view coaching engine\n\
         # Pattern: {} — {}\n\
         # Applied: {}\n\
         # Impact: {}% ({}, {} sessions)\n\
         ---\n\
         \n\
         {}\n",
        req.pattern_id, req.title, date, impact_pct, confidence, req.sample_size, req.recommendation,
    );

    // Write file
    let file_path = state
        .rules_dir
        .join(format!("coaching-{}.md", req.pattern_id));
    std::fs::write(&file_path, &content).map_err(|e| {
        tracing::error!(path = %file_path.display(), error = %e, "Failed to write coaching rule file");
        ApiError::Internal(format!("Failed to write rule file: {}", e))
    })?;

    tracing::info!(
        pattern_id = %req.pattern_id,
        title = %req.title,
        file = %file_path.display(),
        "Coaching rule applied"
    );

    Ok(Json(CoachingRule {
        id: req.pattern_id.clone(),
        pattern_id: req.pattern_id,
        title: req.title,
        body: req.recommendation,
        scope: req.scope,
        applied_at: date,
        file_path: file_path.display().to_string(),
    }))
}

/// DELETE /api/coaching/rules/{id} — Remove a coaching rule file.
async fn remove_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<RemoveRuleResponse>> {
    // Validate the ID to prevent path traversal
    if !is_valid_pattern_id(&id) {
        return Err(ApiError::BadRequest("Invalid pattern ID".to_string()));
    }

    let file_path = state.rules_dir.join(format!("coaching-{}.md", id));

    if !file_path.exists() {
        return Err(ApiError::BadRequest(format!("Rule not found: {}", id)));
    }

    std::fs::remove_file(&file_path).map_err(|e| {
        tracing::error!(path = %file_path.display(), error = %e, "Failed to remove coaching rule file");
        ApiError::Internal(format!("Failed to remove rule file: {}", e))
    })?;

    tracing::info!(pattern_id = %id, file = %file_path.display(), "Coaching rule removed");

    Ok(Json(RemoveRuleResponse {
        removed: true,
        id,
    }))
}

// ============================================================================
// File Parsing
// ============================================================================

/// Read all coaching rule files from the rules directory.
fn read_all_rules(rules_dir: &PathBuf) -> Vec<CoachingRule> {
    if !rules_dir.exists() {
        return Vec::new();
    }

    let entries = match std::fs::read_dir(rules_dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(path = %rules_dir.display(), error = %e, "Failed to read rules directory");
            return Vec::new();
        }
    };

    let mut rules: Vec<CoachingRule> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| name.starts_with("coaching-") && name.ends_with(".md"))
                .unwrap_or(false)
        })
        .filter_map(|entry| parse_rule_file(&entry.path()))
        .collect();

    // Sort by ID for stable ordering
    rules.sort_by(|a, b| a.id.cmp(&b.id));
    rules
}

/// Parse a single coaching rule Markdown file into a `CoachingRule`.
fn parse_rule_file(path: &std::path::Path) -> Option<CoachingRule> {
    let content = std::fs::read_to_string(path).ok()?;
    let filename = path.file_stem()?.to_str()?;
    let id = filename.strip_prefix("coaching-")?;

    let mut title = String::new();
    let mut applied_at = String::new();
    let mut in_header = false;
    let mut body_lines = Vec::new();

    for line in content.lines() {
        if line.starts_with("---") {
            in_header = !in_header;
            continue;
        }
        if in_header {
            if line.starts_with("# Pattern:") {
                if let Some(rest) = line.split('\u{2014}').nth(1) {
                    title = rest.trim().to_string();
                }
            } else if line.starts_with("# Applied:") {
                applied_at = line.trim_start_matches("# Applied:").trim().to_string();
            }
        } else if !line.is_empty() {
            body_lines.push(line);
        }
    }

    Some(CoachingRule {
        id: id.to_string(),
        pattern_id: id.to_string(),
        title,
        body: body_lines.join(" "),
        scope: "user".to_string(),
        applied_at,
        file_path: path.display().to_string(),
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
        Router,
    };
    use tower::ServiceExt;
    use tempfile::TempDir;
    use vibe_recall_db::Database;

    async fn test_db() -> Database {
        Database::new_in_memory().await.expect("in-memory DB")
    }

    fn build_app_with_rules_dir(db: Database, rules_dir: PathBuf) -> Router {
        let mut state = AppState::new(db);
        Arc::get_mut(&mut state).unwrap().rules_dir = rules_dir;
        Router::new()
            .nest("/api", router())
            .with_state(state)
    }

    async fn do_request(
        app: Router,
        method: Method,
        uri: &str,
        body: Option<&str>,
    ) -> (StatusCode, String) {
        let mut builder = Request::builder().method(method).uri(uri);
        let body = if let Some(json) = body {
            builder = builder.header("content-type", "application/json");
            Body::from(json.to_string())
        } else {
            Body::empty()
        };
        let response = app.oneshot(builder.body(body).unwrap()).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    // --- Validation unit tests ---

    #[test]
    fn test_valid_pattern_ids() {
        assert!(is_valid_pattern_id("P1"));
        assert!(is_valid_pattern_id("abc-def"));
        assert!(is_valid_pattern_id("a1b2c3d4e5")); // 10 chars, max length
    }

    #[test]
    fn test_invalid_pattern_ids() {
        assert!(!is_valid_pattern_id(""));
        assert!(!is_valid_pattern_id("1abc")); // starts with digit
        assert!(!is_valid_pattern_id("../etc/passwd")); // path traversal
        assert!(!is_valid_pattern_id("a b")); // space
        assert!(!is_valid_pattern_id("abcdefghijk")); // 11 chars, too long
        assert!(!is_valid_pattern_id("a_b")); // underscore not allowed
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_rules_empty() {
        let tmp = TempDir::new().unwrap();
        let rules_dir = tmp.path().join("rules");
        // Don't create the dir — list should handle missing dir gracefully
        let app = build_app_with_rules_dir(test_db().await, rules_dir);

        let (status, body) = do_request(app, Method::GET, "/api/coaching/rules", None).await;
        assert_eq!(status, StatusCode::OK);

        let resp: ListRulesResponse = serde_json::from_str(&body).unwrap();
        assert_eq!(resp.count, 0);
        assert_eq!(resp.max_rules, 8);
        assert!(resp.rules.is_empty());
    }

    #[tokio::test]
    async fn test_apply_rule_creates_file() {
        let tmp = TempDir::new().unwrap();
        let rules_dir = tmp.path().join("rules");
        let app = build_app_with_rules_dir(test_db().await, rules_dir.clone());

        let payload = serde_json::json!({
            "patternId": "P1",
            "recommendation": "Use focused prompts for better results.",
            "title": "Prompt Clarity",
            "impactScore": 0.75,
            "sampleSize": 42,
            "scope": "user"
        });

        let (status, body) = do_request(
            app,
            Method::POST,
            "/api/coaching/rules",
            Some(&payload.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let rule: CoachingRule = serde_json::from_str(&body).unwrap();
        assert_eq!(rule.id, "P1");
        assert_eq!(rule.title, "Prompt Clarity");
        assert_eq!(rule.scope, "user");

        // Verify file was actually created on disk
        let file_path = rules_dir.join("coaching-P1.md");
        assert!(file_path.exists(), "Rule file should exist on disk");

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("Prompt Clarity"));
        assert!(content.contains("75%"));
        assert!(content.contains("high confidence"));
        assert!(content.contains("42 sessions"));
        assert!(content.contains("Use focused prompts"));
    }

    #[tokio::test]
    async fn test_apply_rule_budget_cap() {
        let tmp = TempDir::new().unwrap();
        let rules_dir = tmp.path().join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        // Pre-create 8 rule files to fill the budget
        for i in 0..8 {
            let filename = format!("coaching-R{}.md", i);
            std::fs::write(
                rules_dir.join(&filename),
                format!(
                    "---\n# Auto-generated by claude-view coaching engine\n# Pattern: R{} \u{2014} Rule {}\n# Applied: 2026-02-15\n# Impact: 50% (moderate confidence, 20 sessions)\n---\n\nSome rule body.\n",
                    i, i
                ),
            )
            .unwrap();
        }

        let app = build_app_with_rules_dir(test_db().await, rules_dir);

        let payload = serde_json::json!({
            "patternId": "P9",
            "recommendation": "This should fail.",
            "title": "Over Budget",
            "impactScore": 0.5,
            "sampleSize": 10,
            "scope": "user"
        });

        let (status, _body) = do_request(
            app,
            Method::POST,
            "/api/coaching/rules",
            Some(&payload.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_remove_rule_deletes_file() {
        let tmp = TempDir::new().unwrap();
        let rules_dir = tmp.path().join("rules");

        // First, create a rule
        let app = build_app_with_rules_dir(test_db().await, rules_dir.clone());

        let payload = serde_json::json!({
            "patternId": "P1",
            "recommendation": "Test rule body.",
            "title": "Test Rule",
            "impactScore": 0.5,
            "sampleSize": 10,
            "scope": "user"
        });

        let (status, _) = do_request(
            app,
            Method::POST,
            "/api/coaching/rules",
            Some(&payload.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let file_path = rules_dir.join("coaching-P1.md");
        assert!(file_path.exists());

        // Now delete it
        let app = build_app_with_rules_dir(test_db().await, rules_dir.clone());
        let (status, body) =
            do_request(app, Method::DELETE, "/api/coaching/rules/P1", None).await;
        assert_eq!(status, StatusCode::OK);

        let resp: RemoveRuleResponse = serde_json::from_str(&body).unwrap();
        assert!(resp.removed);
        assert_eq!(resp.id, "P1");

        // File should be gone
        assert!(!file_path.exists(), "Rule file should be deleted from disk");
    }

    #[tokio::test]
    async fn test_list_rules_reflects_filesystem() {
        let tmp = TempDir::new().unwrap();
        let rules_dir = tmp.path().join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        // Manually write a coaching file (simulating external creation)
        std::fs::write(
            rules_dir.join("coaching-X1.md"),
            "---\n# Auto-generated by claude-view coaching engine\n# Pattern: X1 \u{2014} External Rule\n# Applied: 2026-01-01\n# Impact: 60% (moderate confidence, 15 sessions)\n---\n\nExternal recommendation text.\n",
        )
        .unwrap();

        let app = build_app_with_rules_dir(test_db().await, rules_dir);

        let (status, body) = do_request(app, Method::GET, "/api/coaching/rules", None).await;
        assert_eq!(status, StatusCode::OK);

        let resp: ListRulesResponse = serde_json::from_str(&body).unwrap();
        assert_eq!(resp.count, 1);
        assert_eq!(resp.rules[0].id, "X1");
        assert_eq!(resp.rules[0].title, "External Rule");
        assert_eq!(resp.rules[0].applied_at, "2026-01-01");
        assert!(resp.rules[0].body.contains("External recommendation"));
    }

    #[tokio::test]
    async fn test_apply_rule_idempotent() {
        let tmp = TempDir::new().unwrap();
        let rules_dir = tmp.path().join("rules");

        let payload = serde_json::json!({
            "patternId": "P1",
            "recommendation": "First version.",
            "title": "Test Rule",
            "impactScore": 0.5,
            "sampleSize": 10,
            "scope": "user"
        });

        // Apply once
        let app = build_app_with_rules_dir(test_db().await, rules_dir.clone());
        let (status, _) = do_request(
            app,
            Method::POST,
            "/api/coaching/rules",
            Some(&payload.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // Apply again with same ID (overwrite)
        let payload2 = serde_json::json!({
            "patternId": "P1",
            "recommendation": "Updated version.",
            "title": "Test Rule v2",
            "impactScore": 0.8,
            "sampleSize": 20,
            "scope": "user"
        });

        let app = build_app_with_rules_dir(test_db().await, rules_dir.clone());
        let (status, _) = do_request(
            app,
            Method::POST,
            "/api/coaching/rules",
            Some(&payload2.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // Verify only one file exists
        let files: Vec<_> = std::fs::read_dir(&rules_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files.len(), 1, "Should have exactly 1 file, not 2");

        // Verify content is the updated version
        let content =
            std::fs::read_to_string(rules_dir.join("coaching-P1.md")).unwrap();
        assert!(content.contains("Updated version."));
    }

    #[tokio::test]
    async fn test_invalid_pattern_id_rejected() {
        let tmp = TempDir::new().unwrap();
        let rules_dir = tmp.path().join("rules");
        let app = build_app_with_rules_dir(test_db().await, rules_dir);

        let payload = serde_json::json!({
            "patternId": "../etc/passwd",
            "recommendation": "Malicious.",
            "title": "Evil",
            "impactScore": 0.5,
            "sampleSize": 1,
            "scope": "user"
        });

        let (status, _) = do_request(
            app,
            Method::POST,
            "/api/coaching/rules",
            Some(&payload.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_remove_nonexistent_rule() {
        let tmp = TempDir::new().unwrap();
        let rules_dir = tmp.path().join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();

        let app = build_app_with_rules_dir(test_db().await, rules_dir);

        let (status, _) =
            do_request(app, Method::DELETE, "/api/coaching/rules/nope", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
