// crates/server/src/routes/ide.rs
//! IDE detection and "open in IDE" endpoints.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{extract::State, routing, Json, Router};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Wire types (exported to TypeScript via ts-rs)
// ---------------------------------------------------------------------------

/// Describes a single detected IDE.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "camelCase")]
pub struct IdeInfo {
    /// Machine-readable identifier (e.g. "vscode", "cursor").
    pub id: String,
    /// Human-readable display name (e.g. "VS Code", "Cursor").
    pub name: String,
}

/// Response for `GET /api/ide/detect`.
#[derive(Debug, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[cfg_attr(test, derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct IdeDetectResponse {
    pub available: Vec<IdeInfo>,
}

/// Request body for `POST /api/ide/open`.
#[derive(Debug, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct OpenInIdeRequest {
    /// The IDE id to open (must match an id from the detect response).
    pub ide: String,
    /// Absolute path to the project directory.
    pub project_path: String,
    /// Relative path to the file within the project (optional).
    #[serde(default)]
    pub file_path: Option<String>,
}

/// Response for `POST /api/ide/open`.
#[derive(Debug, Serialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[cfg_attr(test, derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct OpenInIdeResponse {
    pub ok: bool,
}

// ---------------------------------------------------------------------------
// Internal IDE registry
// ---------------------------------------------------------------------------

/// Static definition of a known IDE.
struct IdeDefinition {
    id: &'static str,
    name: &'static str,
    /// CLI command names to search for via `which`.
    commands: &'static [&'static str],
    /// How to format the file:line:col argument.
    open_style: OpenStyle,
}

#[derive(Clone, Copy)]
enum OpenStyle {
    /// VS Code family: `cmd --goto <file>:<line>:<col> <project>`
    VsCodeGoto,
    /// Zed: `zed <project> <file>:<line>:<col>` (positional)
    Positional,
    /// JetBrains: `cmd <project> --line <line> --column <col> <file>`
    JetBrains,
}

static KNOWN_IDES: &[IdeDefinition] = &[
    IdeDefinition {
        id: "vscode",
        name: "VS Code",
        commands: &["code"],
        open_style: OpenStyle::VsCodeGoto,
    },
    IdeDefinition {
        id: "cursor",
        name: "Cursor",
        commands: &["cursor"],
        open_style: OpenStyle::VsCodeGoto,
    },
    IdeDefinition {
        id: "windsurf",
        name: "Windsurf",
        commands: &["windsurf"],
        open_style: OpenStyle::VsCodeGoto,
    },
    IdeDefinition {
        id: "zed",
        name: "Zed",
        commands: &["zed"],
        open_style: OpenStyle::Positional,
    },
    IdeDefinition {
        id: "webstorm",
        name: "WebStorm",
        commands: &["webstorm"],
        open_style: OpenStyle::JetBrains,
    },
    IdeDefinition {
        id: "intellij",
        name: "IntelliJ IDEA",
        commands: &["idea"],
        open_style: OpenStyle::JetBrains,
    },
];

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Detect installed IDEs by running `which` for each known command.
///
/// Returns a list of `(IdeInfo, resolved_command_path)` pairs.
pub fn detect_installed_ides() -> Vec<(IdeInfo, String)> {
    let mut found = Vec::new();
    for def in KNOWN_IDES {
        for cmd in def.commands {
            if let Ok(output) = std::process::Command::new("which").arg(cmd).output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        found.push((
                            IdeInfo {
                                id: def.id.to_string(),
                                name: def.name.to_string(),
                            },
                            path,
                        ));
                        break; // first matching command wins for this IDE
                    }
                }
            }
        }
    }
    found
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /api/ide/detect` — return cached list of installed IDEs.
async fn get_detect(State(state): State<Arc<AppState>>) -> Json<IdeDetectResponse> {
    let ides: Vec<IdeInfo> = state
        .available_ides
        .iter()
        .map(|(info, _)| info.clone())
        .collect();
    Json(IdeDetectResponse { available: ides })
}

/// `POST /api/ide/open` — open a file in the requested IDE.
async fn post_open(
    State(state): State<Arc<AppState>>,
    Json(req): Json<OpenInIdeRequest>,
) -> ApiResult<Json<OpenInIdeResponse>> {
    // --- Validate IDE exists in detected list ---
    let (_, cmd_path) = state
        .available_ides
        .iter()
        .find(|(info, _)| info.id == req.ide)
        .ok_or_else(|| ApiError::BadRequest(format!("Unknown IDE: {}", req.ide)))?;

    // Look up the open style from the static registry.
    let open_style = KNOWN_IDES
        .iter()
        .find(|d| d.id == req.ide)
        .map(|d| d.open_style)
        .unwrap_or(OpenStyle::VsCodeGoto);

    // --- Validate project path ---
    let project = Path::new(&req.project_path);
    if !project.is_absolute() {
        return Err(ApiError::BadRequest(
            "projectPath must be absolute".to_string(),
        ));
    }
    if !project.is_dir() {
        return Err(ApiError::BadRequest(
            "projectPath does not exist or is not a directory".to_string(),
        ));
    }

    // --- Validate file path (if provided) ---
    let resolved_file: Option<PathBuf> = if let Some(ref rel) = req.file_path {
        let rel_path = Path::new(rel);
        if rel_path.is_absolute() {
            return Err(ApiError::BadRequest(
                "filePath must be relative".to_string(),
            ));
        }
        if rel_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(ApiError::BadRequest(
                "filePath must not contain '..'".to_string(),
            ));
        }
        let joined = project.join(rel_path);
        // Canonicalize to resolve any symlinks and verify the path stays within the project.
        let canon = joined
            .canonicalize()
            .map_err(|_| ApiError::BadRequest("filePath does not exist".to_string()))?;
        let canon_project = project
            .canonicalize()
            .map_err(|_| ApiError::BadRequest("projectPath cannot be resolved".to_string()))?;
        if !canon.starts_with(&canon_project) {
            return Err(ApiError::BadRequest(
                "filePath escapes project directory".to_string(),
            ));
        }
        Some(canon)
    } else {
        None
    };

    // --- Build command ---
    let mut command = tokio::process::Command::new(cmd_path);

    match open_style {
        OpenStyle::VsCodeGoto => {
            // `code --goto <file> <project>`
            if let Some(ref file) = resolved_file {
                command.arg("--goto").arg(file.display().to_string());
            }
            command.arg(&req.project_path);
        }
        OpenStyle::Positional => {
            // `zed <project> <file>`
            command.arg(&req.project_path);
            if let Some(ref file) = resolved_file {
                command.arg(file.display().to_string());
            }
        }
        OpenStyle::JetBrains => {
            // `idea <project> <file>`
            command.arg(&req.project_path);
            if let Some(ref file) = resolved_file {
                command.arg(file);
            }
        }
    }

    // Fire-and-forget: spawn detached, tokio auto-reaps.
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| ApiError::Internal(format!("Failed to launch IDE: {e}")))?;

    Ok(Json(OpenInIdeResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ide/detect", routing::get(get_detect))
        .route("/ide/open", routing::post(post_open))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use claude_view_db::Database;
    use tower::ServiceExt;

    /// Build a test app with a seeded fake IDE in `available_ides`.
    async fn test_app() -> Router {
        let db = Database::new_in_memory().await.expect("in-memory DB");
        let mut state = crate::state::AppState::new(db);
        // Seed a fake IDE for testing (Arc::get_mut works before cloning).
        let inner = Arc::get_mut(&mut state).expect("unique Arc");
        inner.available_ides.push((
            IdeInfo {
                id: "testvscode".to_string(),
                name: "Test VS Code".to_string(),
            },
            "/usr/bin/true".to_string(), // harmless binary
        ));
        crate::routes::api_routes(state)
    }

    /// Helper: GET request.
    async fn get(app: Router, uri: &str) -> (StatusCode, String) {
        let resp = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    /// Helper: POST request with JSON body.
    async fn post_json(app: Router, uri: &str, body: &impl Serialize) -> (StatusCode, String) {
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn test_ide_detect_returns_seeded_list() {
        let app = test_app().await;
        let (status, body) = get(app, "/api/ide/detect").await;
        assert_eq!(status, StatusCode::OK);
        let resp: IdeDetectResponse = serde_json::from_str(&body).unwrap();
        assert_eq!(resp.available.len(), 1);
        assert_eq!(resp.available[0].id, "testvscode");
    }

    #[tokio::test]
    async fn test_open_rejects_relative_path() {
        let app = test_app().await;
        let req = serde_json::json!({
            "ide": "testvscode",
            "projectPath": "relative/path",
        });
        let (status, body) = post_json(app, "/api/ide/open", &req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("absolute"), "body: {body}");
    }

    #[tokio::test]
    async fn test_open_rejects_unknown_ide() {
        let app = test_app().await;
        let req = serde_json::json!({
            "ide": "nonexistent",
            "projectPath": "/tmp",
        });
        let (status, body) = post_json(app, "/api/ide/open", &req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("Unknown IDE"), "body: {body}");
    }

    #[tokio::test]
    async fn test_open_rejects_path_traversal() {
        let app = test_app().await;
        // Use /tmp as the project dir (always exists on macOS/Linux).
        let req = serde_json::json!({
            "ide": "testvscode",
            "projectPath": "/tmp",
            "filePath": "../etc/passwd",
        });
        let (status, body) = post_json(app, "/api/ide/open", &req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body.contains("..") || body.contains("escapes"),
            "body: {body}"
        );
    }
}
