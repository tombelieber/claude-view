//! Types for session index parsing and classification.

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionKind {
    Conversation, // has user + assistant lines
    MetadataOnly, // file-history-snapshot, summary, etc.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartType {
    User,
    FileHistorySnapshot,
    QueueOperation,
    Progress,
    Summary,
    Assistant,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct SessionClassification {
    pub kind: SessionKind,
    pub start_type: StartType,
    pub cwd: Option<String>,
    pub parent_id: Option<String>,
}

/// Wrapper for the `sessions-index.json` file format.
/// The file is `{"version": N, "entries": [...]}`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SessionIndexFile {
    #[allow(dead_code)]
    pub version: Option<u32>,
    pub entries: Vec<SessionIndexEntry>,
}

/// A single entry from a `sessions-index.json` file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIndexEntry {
    pub session_id: String,
    #[serde(default)]
    pub full_path: Option<String>,
    #[serde(default)]
    pub file_mtime: Option<u64>,
    #[serde(default)]
    pub first_prompt: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub message_count: Option<usize>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub modified: Option<String>,
    #[serde(default)]
    pub git_branch: Option<String>,
    #[serde(default)]
    pub project_path: Option<String>,
    #[serde(default)]
    pub is_sidechain: Option<bool>,
    #[serde(default)]
    pub session_cwd: Option<String>,
    #[serde(default)]
    pub parent_session_id: Option<String>,
}
