//! Input validation and mutation locking for plugin operations.

use crate::error::ApiError;

pub(crate) const VALID_ACTIONS: &[&str] = &["install", "update", "uninstall", "enable", "disable"];

/// Reject CLI flag injection -- only [a-zA-Z0-9._@-] allowed, must not start with `-`.
pub(crate) fn validate_plugin_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty()
        || name.len() > 128
        || name.starts_with('-')
        || name
            .chars()
            .any(|c| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.' && c != '@')
    {
        return Err(ApiError::BadRequest(format!(
            "Invalid plugin name: {name}. Must start with alphanumeric and contain only alphanumeric, hyphens, underscores, dots, and @."
        )));
    }
    Ok(())
}

pub(crate) fn validate_scope(scope: &Option<String>) -> Result<(), ApiError> {
    if let Some(s) = scope {
        if s != "user" && s != "project" {
            return Err(ApiError::BadRequest(format!(
                "Invalid scope: {s}. Must be 'user' or 'project'."
            )));
        }
    }
    Ok(())
}

/// Validate marketplace source -- must be "owner/repo" short form.
pub(crate) fn validate_marketplace_source(source: &str) -> Result<String, ApiError> {
    let short = source
        .trim_start_matches("https://github.com/")
        .trim_start_matches("http://github.com/")
        .trim_end_matches('/')
        .trim_end_matches(".git");

    let parts: Vec<&str> = short.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(ApiError::BadRequest(format!(
            "Invalid marketplace source: {source}. Use 'owner/repo' format."
        )));
    }

    for part in &parts {
        if part
            .chars()
            .any(|c| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.')
        {
            return Err(ApiError::BadRequest(format!(
                "Invalid characters in marketplace source: {source}."
            )));
        }
    }

    if short.len() > 256 {
        return Err(ApiError::BadRequest("Marketplace source too long.".into()));
    }

    Ok(short.to_string())
}

// Marketplace-only mutation lock (plugin mutations go through the op queue).
static MARKETPLACE_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

pub(crate) fn get_marketplace_lock() -> &'static tokio::sync::Mutex<()> {
    MARKETPLACE_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}
