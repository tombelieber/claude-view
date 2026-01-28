// crates/server/src/error.rs
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;
use ts_rs::TS;
use vibe_recall_core::{DiscoveryError, ParseError};
use vibe_recall_db::DbError;

/// Structured JSON error response for API errors
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct ErrorResponse {
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ErrorResponse {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            details: None,
        }
    }

    pub fn with_details(error: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            details: Some(details.into()),
        }
    }
}

/// API error types that map to HTTP status codes
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Project not found: {0}")]
    ProjectNotFound(String),

    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("Discovery error: {0}")]
    Discovery(#[from] DiscoveryError),

    #[error("Database error: {0}")]
    Database(#[from] DbError),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_response) = match &self {
            ApiError::SessionNotFound(id) => {
                tracing::error!(session_id = %id, "Session not found");
                (
                    StatusCode::NOT_FOUND,
                    ErrorResponse::with_details("Session not found", format!("Session ID: {}", id)),
                )
            }
            ApiError::ProjectNotFound(id) => {
                tracing::error!(project_id = %id, "Project not found");
                (
                    StatusCode::NOT_FOUND,
                    ErrorResponse::with_details("Project not found", format!("Project ID: {}", id)),
                )
            }
            ApiError::Parse(parse_err) => {
                let (status, error_msg) = match parse_err {
                    ParseError::NotFound { path } => {
                        tracing::error!(path = %path.display(), "File not found");
                        (StatusCode::NOT_FOUND, "File not found")
                    }
                    ParseError::PermissionDenied { path } => {
                        tracing::error!(path = %path.display(), "Permission denied");
                        (StatusCode::FORBIDDEN, "Permission denied")
                    }
                    ParseError::Io { path, source } => {
                        tracing::error!(path = %path.display(), error = %source, "IO error");
                        (StatusCode::INTERNAL_SERVER_ERROR, "IO error reading file")
                    }
                    ParseError::InvalidUtf8 { path, line } => {
                        tracing::error!(path = %path.display(), line = %line, "Invalid UTF-8");
                        (StatusCode::INTERNAL_SERVER_ERROR, "Invalid file encoding")
                    }
                    ParseError::MalformedJson { path, line, message } => {
                        tracing::error!(path = %path.display(), line = %line, message = %message, "Malformed JSON");
                        (StatusCode::INTERNAL_SERVER_ERROR, "Malformed session data")
                    }
                    ParseError::EmptyFile { path } => {
                        tracing::error!(path = %path.display(), "Empty file");
                        (StatusCode::INTERNAL_SERVER_ERROR, "Empty session file")
                    }
                };
                (
                    status,
                    ErrorResponse::with_details(error_msg, parse_err.to_string()),
                )
            }
            ApiError::Discovery(discovery_err) => {
                let error_msg = match discovery_err {
                    DiscoveryError::ProjectsDirNotFound { path } => {
                        tracing::error!(path = %path.display(), "Projects directory not found");
                        "Claude projects directory not found"
                    }
                    DiscoveryError::PermissionDenied { path } => {
                        tracing::error!(path = %path.display(), "Permission denied accessing projects");
                        "Cannot access Claude projects directory"
                    }
                    DiscoveryError::Io { path, source } => {
                        tracing::error!(path = %path.display(), error = %source, "IO error during discovery");
                        "IO error accessing projects"
                    }
                    DiscoveryError::HomeDirNotFound => {
                        tracing::error!("Home directory not found");
                        "Home directory not found"
                    }
                };
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorResponse::with_details(error_msg, discovery_err.to_string()),
                )
            }
            ApiError::Database(db_err) => {
                tracing::error!(error = %db_err, "Database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorResponse::with_details("Database error", db_err.to_string()),
                )
            }
            ApiError::Internal(msg) => {
                tracing::error!(message = %msg, "Internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorResponse::new("Internal server error"),
                )
            }
            ApiError::BadRequest(msg) => {
                tracing::warn!(message = %msg, "Bad request");
                (
                    StatusCode::BAD_REQUEST,
                    ErrorResponse::with_details("Bad request", msg.clone()),
                )
            }
            ApiError::Conflict(msg) => {
                tracing::warn!(message = %msg, "Conflict");
                (
                    StatusCode::CONFLICT,
                    ErrorResponse::with_details("Conflict", msg.clone()),
                )
            }
        };

        (status, Json(error_response)).into_response()
    }
}

/// Result type alias for API handlers
pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use std::path::PathBuf;

    /// Helper to extract status code and body from a response
    async fn extract_response(response: Response) -> (StatusCode, ErrorResponse) {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let error_response: ErrorResponse = serde_json::from_slice(&body).unwrap();
        (status, error_response)
    }

    #[tokio::test]
    async fn test_session_not_found_returns_404() {
        let error = ApiError::SessionNotFound("abc123".to_string());
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.error, "Session not found");
        assert!(body.details.unwrap().contains("abc123"));
    }

    #[tokio::test]
    async fn test_project_not_found_returns_404() {
        let error = ApiError::ProjectNotFound("my-project".to_string());
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.error, "Project not found");
        assert!(body.details.unwrap().contains("my-project"));
    }

    #[tokio::test]
    async fn test_parse_not_found_returns_404() {
        let error = ApiError::Parse(ParseError::NotFound {
            path: PathBuf::from("/path/to/session.jsonl"),
        });
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.error, "File not found");
        assert!(body.details.unwrap().contains("/path/to/session.jsonl"));
    }

    #[tokio::test]
    async fn test_parse_permission_denied_returns_403() {
        let error = ApiError::Parse(ParseError::PermissionDenied {
            path: PathBuf::from("/secret/file.jsonl"),
        });
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body.error, "Permission denied");
        assert!(body.details.unwrap().contains("/secret/file.jsonl"));
    }

    #[tokio::test]
    async fn test_parse_io_error_returns_500() {
        let error = ApiError::Parse(ParseError::Io {
            path: PathBuf::from("/path/file.jsonl"),
            source: std::io::Error::new(std::io::ErrorKind::Other, "disk error"),
        });
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "IO error reading file");
    }

    #[tokio::test]
    async fn test_parse_invalid_utf8_returns_500() {
        let error = ApiError::Parse(ParseError::InvalidUtf8 {
            path: PathBuf::from("/path/file.jsonl"),
            line: 42,
        });
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "Invalid file encoding");
    }

    #[tokio::test]
    async fn test_parse_malformed_json_returns_500() {
        let error = ApiError::Parse(ParseError::MalformedJson {
            path: PathBuf::from("/path/file.jsonl"),
            line: 10,
            message: "unexpected token".to_string(),
        });
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "Malformed session data");
    }

    #[tokio::test]
    async fn test_parse_empty_file_returns_500() {
        let error = ApiError::Parse(ParseError::EmptyFile {
            path: PathBuf::from("/path/empty.jsonl"),
        });
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "Empty session file");
    }

    #[tokio::test]
    async fn test_discovery_home_dir_not_found_returns_500() {
        let error = ApiError::Discovery(DiscoveryError::HomeDirNotFound);
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "Home directory not found");
    }

    #[tokio::test]
    async fn test_discovery_projects_dir_not_found_returns_500() {
        let error = ApiError::Discovery(DiscoveryError::ProjectsDirNotFound {
            path: PathBuf::from("/home/user/.claude/projects"),
        });
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "Claude projects directory not found");
    }

    #[tokio::test]
    async fn test_discovery_permission_denied_returns_500() {
        let error = ApiError::Discovery(DiscoveryError::PermissionDenied {
            path: PathBuf::from("/home/user/.claude/projects"),
        });
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "Cannot access Claude projects directory");
    }

    #[tokio::test]
    async fn test_discovery_io_error_returns_500() {
        let error = ApiError::Discovery(DiscoveryError::Io {
            path: PathBuf::from("/home/user/.claude/projects"),
            source: std::io::Error::new(std::io::ErrorKind::Other, "disk error"),
        });
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "IO error accessing projects");
    }

    #[tokio::test]
    async fn test_internal_error_returns_500() {
        let error = ApiError::Internal("Something went wrong".to_string());
        let response = error.into_response();
        let (status, body) = extract_response(response).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body.error, "Internal server error");
        // Internal errors should NOT expose details to clients
        assert!(body.details.is_none());
    }

    #[test]
    fn test_error_response_serialization() {
        let response = ErrorResponse::new("Test error");
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"error\":\"Test error\""));
        assert!(!json.contains("details")); // None should be skipped

        let response = ErrorResponse::with_details("Test error", "More info");
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"error\":\"Test error\""));
        assert!(json.contains("\"details\":\"More info\""));
    }

    #[test]
    fn test_api_error_from_parse_error() {
        let parse_err = ParseError::NotFound {
            path: PathBuf::from("/test"),
        };
        let api_err: ApiError = parse_err.into();
        assert!(matches!(api_err, ApiError::Parse(_)));
    }

    #[test]
    fn test_api_error_from_discovery_error() {
        let discovery_err = DiscoveryError::HomeDirNotFound;
        let api_err: ApiError = discovery_err.into();
        assert!(matches!(api_err, ApiError::Discovery(_)));
    }

    #[test]
    fn test_api_error_display() {
        let err = ApiError::SessionNotFound("test-id".to_string());
        assert_eq!(err.to_string(), "Session not found: test-id");

        let err = ApiError::ProjectNotFound("my-proj".to_string());
        assert_eq!(err.to_string(), "Project not found: my-proj");

        let err = ApiError::Internal("oops".to_string());
        assert_eq!(err.to_string(), "Internal server error: oops");
    }
}
