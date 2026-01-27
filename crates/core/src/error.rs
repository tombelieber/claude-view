// crates/core/src/error.rs
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur when parsing JSONL sessions
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Session file not found: {path}")]
    NotFound { path: PathBuf },

    #[error("Permission denied reading file: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("IO error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Invalid UTF-8 in file {path} at line {line}")]
    InvalidUtf8 { path: PathBuf, line: usize },

    #[error("Malformed JSON at line {line} in {path}: {message}")]
    MalformedJson {
        path: PathBuf,
        line: usize,
        message: String,
    },

    #[error("Empty session file: {path}")]
    EmptyFile { path: PathBuf },
}

/// Errors that can occur during project discovery
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("Claude projects directory not found: {path}")]
    ProjectsDirNotFound { path: PathBuf },

    #[error("Cannot access Claude projects directory: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("IO error accessing {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Home directory not found")]
    HomeDirNotFound,
}

impl ParseError {
    pub fn not_found(path: impl Into<PathBuf>) -> Self {
        Self::NotFound { path: path.into() }
    }

    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        let path = path.into();
        match source.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound { path },
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied { path },
            _ => Self::Io { path, source },
        }
    }
}

/// Errors that can occur when parsing session index files
#[derive(Debug, Error)]
pub enum SessionIndexError {
    #[error("Session index file not found: {path}")]
    NotFound { path: PathBuf },

    #[error("Permission denied reading session index: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("IO error reading session index {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Malformed JSON in session index {path}: {message}")]
    MalformedJson { path: PathBuf, message: String },

    #[error("Projects directory not found: {path}")]
    ProjectsDirNotFound { path: PathBuf },
}

impl SessionIndexError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        let path = path.into();
        match source.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound { path },
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied { path },
            _ => Self::Io { path, source },
        }
    }
}

impl DiscoveryError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        let path = path.into();
        match source.kind() {
            std::io::ErrorKind::NotFound => Self::ProjectsDirNotFound { path },
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied { path },
            _ => Self::Io { path, source },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_display() {
        let err = ParseError::not_found("/path/to/file.jsonl");
        assert!(err.to_string().contains("/path/to/file.jsonl"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_parse_error_io_classification() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = ParseError::io("/test/path", io_err);
        assert!(matches!(err, ParseError::NotFound { .. }));

        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = ParseError::io("/test/path", io_err);
        assert!(matches!(err, ParseError::PermissionDenied { .. }));
    }

    #[test]
    fn test_parse_error_io_other() {
        let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout");
        let err = ParseError::io("/test/path", io_err);
        assert!(matches!(err, ParseError::Io { .. }));
    }

    #[test]
    fn test_discovery_error_display() {
        let err = DiscoveryError::HomeDirNotFound;
        assert!(err.to_string().contains("Home directory"));
    }

    #[test]
    fn test_discovery_error_io_classification() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let err = DiscoveryError::io("/test/path", io_err);
        assert!(matches!(err, DiscoveryError::ProjectsDirNotFound { .. }));

        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err = DiscoveryError::io("/test/path", io_err);
        assert!(matches!(err, DiscoveryError::PermissionDenied { .. }));
    }
}
