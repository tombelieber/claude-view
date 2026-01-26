# vibe-recall Phase 1: Foundation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the Express/Node.js backend with a Rust binary that serves the existing React frontend.

**Architecture:** 4-crate Rust workspace (core, db, search, server) with React frontend. Phase 1 establishes the foundation: types, JSONL parsing, SQLite, and basic API endpoints.

**Tech Stack:** Rust 2024, Axum 0.8.6, SQLx 0.8.6, Tantivy 0.25, React 19, Vite

---

## Prerequisites

- Rust toolchain installed (`rustup`)
- SQLx CLI: `cargo install sqlx-cli --features sqlite`

---

## Task 1: Scaffold Rust Workspace

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/core/Cargo.toml`
- Create: `crates/core/src/lib.rs`
- Create: `crates/db/Cargo.toml`
- Create: `crates/db/src/lib.rs`
- Create: `crates/search/Cargo.toml`
- Create: `crates/search/src/lib.rs`
- Create: `crates/server/Cargo.toml`
- Create: `crates/server/src/lib.rs`
- Create: `crates/server/src/main.rs`

**Step 1: Create workspace Cargo.toml**

```toml
# Cargo.toml (root)
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/user/vibe-recall"

[workspace.dependencies]
# Async runtime
tokio = { version = "1.49", features = ["full"] }

# Web framework
axum = { version = "0.8", features = ["macros"] }
tower-http = { version = "0.6", features = ["cors", "fs", "trace"] }

# Database
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }

# Search (Phase 2)
tantivy = "0.25"

# File watcher (Phase 2)
notify = "8.2"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Error handling
thiserror = "2"
anyhow = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Internal crates
vibe-recall-core = { path = "crates/core" }
vibe-recall-db = { path = "crates/db" }
vibe-recall-search = { path = "crates/search" }

[profile.release]
lto = true
strip = true
```

**Step 2: Create crates/core/Cargo.toml**

```toml
# crates/core/Cargo.toml
[package]
name = "vibe-recall-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["fs", "io-util"] }
```

**Step 3: Create crates/core/src/lib.rs**

```rust
// crates/core/src/lib.rs
pub mod types;
pub mod parser;

pub use types::*;
pub use parser::*;
```

**Step 4: Create crates/db/Cargo.toml**

```toml
# crates/db/Cargo.toml
[package]
name = "vibe-recall-db"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
vibe-recall-core = { workspace = true }
sqlx = { workspace = true }
tokio = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
```

**Step 5: Create crates/db/src/lib.rs**

```rust
// crates/db/src/lib.rs
pub mod models;
mod pool;

pub use pool::*;
```

**Step 6: Create crates/search/Cargo.toml**

```toml
# crates/search/Cargo.toml
[package]
name = "vibe-recall-search"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
vibe-recall-core = { workspace = true }
tantivy = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
```

**Step 7: Create crates/search/src/lib.rs**

```rust
// crates/search/src/lib.rs
// Phase 2: Search functionality
```

**Step 8: Create crates/server/Cargo.toml**

```toml
# crates/server/Cargo.toml
[package]
name = "vibe-recall-server"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "vibe-recall"
path = "src/main.rs"

[dependencies]
vibe-recall-core = { workspace = true }
vibe-recall-db = { workspace = true }
vibe-recall-search = { workspace = true }
axum = { workspace = true }
tower-http = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

**Step 9: Create crates/server/src/lib.rs**

```rust
// crates/server/src/lib.rs
pub mod routes;
pub mod state;
```

**Step 10: Create crates/server/src/main.rs**

```rust
// crates/server/src/main.rs
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("vibe-recall server starting...");
    Ok(())
}
```

**Step 11: Verify workspace compiles**

Run: `cargo build`
Expected: Compiles successfully with warnings about unused code

**Step 12: Commit**

```bash
git add Cargo.toml crates/
git commit -m "feat: scaffold Rust workspace with 4 crates

- core: shared types and JSONL parser
- db: SQLite via sqlx
- search: Tantivy full-text (Phase 2)
- server: Axum HTTP server

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 2: Define Core Types

**Files:**
- Create: `crates/core/src/types.rs`

**Step 1: Create types.rs with all domain types**

```rust
// crates/core/src/types.rs
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A Claude Code project (directory containing sessions)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    /// URL-safe encoded directory name
    pub id: String,
    /// Human-readable project name (e.g., "claude-view")
    pub name: String,
    /// Full filesystem path to the project
    pub path: String,
    /// Encoded directory name in ~/.claude/projects/
    pub dir_name: String,
    /// Number of sessions in this project
    pub session_count: usize,
    /// Count of sessions active in last 5 minutes
    pub active_count: usize,
}

/// A Claude Code session (conversation)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// Session ID (JSONL filename without extension)
    pub id: String,
    /// Encoded project directory name
    pub project: String,
    /// Full filesystem path to project
    pub project_path: String,
    /// Full path to the JSONL file
    pub file_path: String,
    /// Last modified timestamp (Unix ms)
    pub modified_at: i64,
    /// File size in bytes
    pub size_bytes: u64,
    /// Preview text (first user message, truncated)
    pub preview: String,
    /// Last user message (truncated)
    pub last_message: String,
    /// Files touched in this session (max 5)
    pub files_touched: Vec<String>,
    /// Skills/commands used (max 5)
    pub skills_used: Vec<String>,
    /// Tool usage counts
    pub tool_counts: ToolCounts,
    /// Total message count
    pub message_count: usize,
    /// Number of human→assistant turns
    pub turn_count: usize,
}

/// Tool usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolCounts {
    pub edit: usize,
    pub read: usize,
    pub bash: usize,
    pub write: usize,
}

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// A tool call made by the assistant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub count: usize,
}

/// Parsed session with messages and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedSession {
    pub messages: Vec<Message>,
    pub metadata: SessionMetadata,
}

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub total_messages: usize,
    pub tool_call_count: usize,
}

/// Project info with sessions (API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub name: String,
    pub display_name: String,
    pub path: String,
    pub sessions: Vec<Session>,
    pub active_count: usize,
}

/// JSONL entry types for parsing
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JsonlEntry {
    User {
        message: Option<JsonlMessage>,
        timestamp: Option<String>,
        #[serde(rename = "isMeta")]
        is_meta: Option<bool>,
    },
    Assistant {
        message: Option<JsonlMessage>,
        timestamp: Option<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonlMessage {
    pub role: Option<String>,
    pub content: JsonlContent,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum JsonlContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        name: String,
        input: Option<serde_json::Value>,
    },
    ToolResult {
        #[serde(default)]
        content: Option<serde_json::Value>,
    },
    #[serde(other)]
    Other,
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p vibe-recall-core`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add crates/core/src/types.rs
git commit -m "feat(core): add domain types for projects, sessions, messages

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 3: JSONL Parser

**Files:**
- Create: `crates/core/src/parser.rs`

**Step 1: Create parser.rs**

```rust
// crates/core/src/parser.rs
use crate::types::*;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Session file not found: {0}")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Parse a Claude Code JSONL session file into structured messages
pub async fn parse_session(file_path: &Path) -> Result<ParsedSession, ParseError> {
    if !file_path.exists() {
        return Err(ParseError::NotFound(file_path.display().to_string()));
    }

    let file = File::open(file_path).await?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut messages: Vec<Message> = Vec::new();
    let mut tool_call_counts: HashMap<String, usize> = HashMap::new();
    let mut pending_tool_calls: Vec<String> = Vec::new();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        // Parse JSONL entry, skip malformed lines
        let entry: JsonlEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        match entry {
            JsonlEntry::User { message, timestamp, is_meta } => {
                if is_meta.unwrap_or(false) {
                    continue;
                }

                if let Some(msg) = message {
                    let content = extract_user_content(&msg.content);

                    if let Some(text) = content {
                        // Attach pending tool calls to previous assistant message
                        if !pending_tool_calls.is_empty() {
                            if let Some(last) = messages.last_mut() {
                                if last.role == Role::Assistant {
                                    last.tool_calls = Some(aggregate_tool_calls(&pending_tool_calls));
                                }
                            }
                            pending_tool_calls.clear();
                        }

                        messages.push(Message {
                            role: Role::User,
                            content: text,
                            timestamp,
                            tool_calls: None,
                        });
                    }
                }
            }

            JsonlEntry::Assistant { message, timestamp } => {
                if let Some(msg) = message {
                    let (text, tools) = extract_assistant_content(&msg.content);

                    // Collect tool calls
                    for tool in &tools {
                        pending_tool_calls.push(tool.clone());
                        *tool_call_counts.entry(tool.clone()).or_insert(0) += 1;
                    }

                    // Add text message if present
                    if let Some(text) = text {
                        let mut message = Message {
                            role: Role::Assistant,
                            content: text,
                            timestamp,
                            tool_calls: None,
                        };

                        if !pending_tool_calls.is_empty() {
                            message.tool_calls = Some(aggregate_tool_calls(&pending_tool_calls));
                            pending_tool_calls.clear();
                        }

                        messages.push(message);
                    }
                }
            }

            JsonlEntry::Other => {}
        }
    }

    // Handle remaining pending tool calls
    if !pending_tool_calls.is_empty() {
        if let Some(last) = messages.last_mut() {
            if last.role == Role::Assistant && last.tool_calls.is_none() {
                last.tool_calls = Some(aggregate_tool_calls(&pending_tool_calls));
            }
        }
    }

    let total_tool_calls: usize = tool_call_counts.values().sum();

    Ok(ParsedSession {
        messages,
        metadata: SessionMetadata {
            total_messages: messages.len(),
            tool_call_count: total_tool_calls,
        },
    })
}

/// Extract text content from a user message
fn extract_user_content(content: &JsonlContent) -> Option<String> {
    match content {
        JsonlContent::Text(text) => {
            // Clean up command tags
            let cleaned = clean_command_tags(text);
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned)
            }
        }
        JsonlContent::Blocks(blocks) => {
            let text_parts: Vec<&str> = blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect();

            if text_parts.is_empty() {
                None
            } else {
                let joined = text_parts.join("\n").trim().to_string();
                if joined.is_empty() {
                    None
                } else {
                    Some(joined)
                }
            }
        }
    }
}

/// Extract text and tool names from an assistant message
fn extract_assistant_content(content: &JsonlContent) -> (Option<String>, Vec<String>) {
    match content {
        JsonlContent::Text(text) => (Some(text.clone()), Vec::new()),
        JsonlContent::Blocks(blocks) => {
            let mut text_parts: Vec<String> = Vec::new();
            let mut tools: Vec<String> = Vec::new();

            for block in blocks {
                match block {
                    ContentBlock::Text { text } => {
                        text_parts.push(text.clone());
                    }
                    ContentBlock::ToolUse { name, .. } => {
                        tools.push(name.clone());
                    }
                    _ => {}
                }
            }

            let text = if text_parts.is_empty() {
                None
            } else {
                let joined = text_parts.join("\n").trim().to_string();
                if joined.is_empty() {
                    None
                } else {
                    Some(joined)
                }
            };

            (text, tools)
        }
    }
}

/// Clean command tags from text
fn clean_command_tags(text: &str) -> String {
    // Remove <command-*>...</command-*> tags
    let re = regex_lite::Regex::new(r"<command-[^>]*>[^<]*</command-[^>]*>").unwrap();
    re.replace_all(text, "").trim().to_string()
}

/// Aggregate tool calls by name
fn aggregate_tool_calls(tools: &[String]) -> Vec<ToolCall> {
    let mut counts: HashMap<String, usize> = HashMap::new();

    for tool in tools {
        *counts.entry(tool.clone()).or_insert(0) += 1;
    }

    counts
        .into_iter()
        .map(|(name, count)| ToolCall { name, count })
        .collect()
}
```

**Step 2: Add regex-lite dependency to core**

Update `crates/core/Cargo.toml`:

```toml
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["fs", "io-util"] }
regex-lite = "0.1"
```

**Step 3: Verify it compiles**

Run: `cargo build -p vibe-recall-core`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add crates/core/
git commit -m "feat(core): add JSONL parser for Claude Code sessions

- Async streaming parser using tokio
- Extracts user/assistant messages
- Tracks tool calls per message
- Cleans command tags from content

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 4: Project Discovery

**Files:**
- Create: `crates/core/src/discovery.rs`
- Modify: `crates/core/src/lib.rs`

**Step 1: Create discovery.rs**

```rust
// crates/core/src/discovery.rs
use crate::types::*;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Get the Claude projects directory
pub fn claude_projects_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".claude")
        .join("projects")
}

/// Discover all Claude Code projects and their sessions
pub async fn get_projects() -> Result<Vec<ProjectInfo>, std::io::Error> {
    let projects_dir = claude_projects_dir();

    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let mut projects: Vec<ProjectInfo> = Vec::new();
    let mut entries = fs::read_dir(&projects_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        // Skip non-directories and hidden directories
        if !path.is_dir() {
            continue;
        }

        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) if !name.starts_with('.') => name.to_string(),
            _ => continue,
        };

        // Resolve the encoded directory name to actual path
        let resolved = resolve_project_path(&dir_name);

        // Get sessions in this project
        let sessions = get_project_sessions(&path, &dir_name, &resolved).await?;

        if sessions.is_empty() {
            continue;
        }

        // Count sessions active in last 5 minutes
        let five_minutes_ago = chrono::Utc::now().timestamp_millis() - 5 * 60 * 1000;
        let active_count = sessions
            .iter()
            .filter(|s| s.modified_at > five_minutes_ago)
            .count();

        projects.push(ProjectInfo {
            name: resolved.full_path.clone(),
            display_name: resolved.display_name.clone(),
            path: path.display().to_string(),
            sessions,
            active_count,
        });
    }

    // Sort by most recent session
    projects.sort_by(|a, b| {
        let a_latest = a.sessions.first().map(|s| s.modified_at).unwrap_or(0);
        let b_latest = b.sessions.first().map(|s| s.modified_at).unwrap_or(0);
        b_latest.cmp(&a_latest)
    });

    Ok(projects)
}

/// Get all sessions in a project directory
async fn get_project_sessions(
    project_path: &Path,
    dir_name: &str,
    resolved: &ResolvedProject,
) -> Result<Vec<Session>, std::io::Error> {
    let mut sessions: Vec<Session> = Vec::new();
    let mut entries = fs::read_dir(project_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        // Only process .jsonl files
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) if name.ends_with(".jsonl") => name,
            _ => continue,
        };

        let metadata = match fs::metadata(&path).await {
            Ok(m) if m.is_file() => m,
            _ => continue,
        };

        let session_id = file_name.trim_end_matches(".jsonl").to_string();
        let modified_at = metadata
            .modified()
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        // Extract session metadata
        let session_meta = get_session_metadata(&path).await;

        sessions.push(Session {
            id: session_id,
            project: dir_name.to_string(),
            project_path: resolved.full_path.clone(),
            file_path: path.display().to_string(),
            modified_at,
            size_bytes: metadata.len(),
            preview: session_meta.preview,
            last_message: session_meta.last_message,
            files_touched: session_meta.files_touched,
            skills_used: session_meta.skills_used,
            tool_counts: session_meta.tool_counts,
            message_count: session_meta.message_count,
            turn_count: session_meta.turn_count,
        });
    }

    // Sort by modified date (newest first)
    sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

    Ok(sessions)
}

struct SessionMetadataResult {
    preview: String,
    last_message: String,
    files_touched: Vec<String>,
    skills_used: Vec<String>,
    tool_counts: ToolCounts,
    message_count: usize,
    turn_count: usize,
}

/// Extract metadata from a session file
async fn get_session_metadata(file_path: &Path) -> SessionMetadataResult {
    let mut result = SessionMetadataResult {
        preview: "(no user message found)".to_string(),
        last_message: String::new(),
        files_touched: Vec::new(),
        skills_used: Vec::new(),
        tool_counts: ToolCounts::default(),
        message_count: 0,
        turn_count: 0,
    };

    let content = match fs::read_to_string(file_path).await {
        Ok(c) => c,
        Err(_) => return result,
    };

    let mut files_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut skills_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut first_user_message = String::new();
    let mut last_user_message = String::new();
    let mut user_count = 0;
    let mut assistant_count = 0;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let entry: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let entry_type = entry.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if entry_type == "user" {
            if let Some(text) = extract_user_text(&entry) {
                if text.len() > 10 && !text.starts_with('<') {
                    user_count += 1;

                    let truncated = if text.len() > 200 {
                        format!("{}…", &text[..200])
                    } else {
                        text.clone()
                    };

                    if first_user_message.is_empty() {
                        first_user_message = truncated.clone();
                    }
                    last_user_message = truncated;

                    // Detect skills (slash commands)
                    for cap in regex_lite::Regex::new(r"/[\w:-]+")
                        .unwrap()
                        .find_iter(&text)
                    {
                        skills_set.insert(cap.as_str().to_string());
                    }
                }
            }
        }

        if entry_type == "assistant" {
            assistant_count += 1;

            if let Some(content) = entry.get("message").and_then(|m| m.get("content")) {
                if let Some(blocks) = content.as_array() {
                    for block in blocks {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            let tool_name = block
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("")
                                .to_lowercase();

                            match tool_name.as_str() {
                                "edit" => {
                                    result.tool_counts.edit += 1;
                                    if let Some(path) =
                                        block.get("input").and_then(|i| i.get("file_path")).and_then(|p| p.as_str())
                                    {
                                        files_set.insert(path.to_string());
                                    }
                                }
                                "write" => {
                                    result.tool_counts.write += 1;
                                    if let Some(path) =
                                        block.get("input").and_then(|i| i.get("file_path")).and_then(|p| p.as_str())
                                    {
                                        files_set.insert(path.to_string());
                                    }
                                }
                                "read" => result.tool_counts.read += 1,
                                "bash" => result.tool_counts.bash += 1,
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    result.preview = if first_user_message.is_empty() {
        "(no user message found)".to_string()
    } else {
        first_user_message
    };
    result.last_message = last_user_message;
    result.files_touched = files_set
        .into_iter()
        .take(5)
        .map(|f| f.split('/').last().unwrap_or(&f).to_string())
        .collect();
    result.skills_used = skills_set.into_iter().take(5).collect();
    result.message_count = user_count + assistant_count;
    result.turn_count = user_count.min(assistant_count);

    result
}

fn extract_user_text(entry: &serde_json::Value) -> Option<String> {
    let content = entry.get("message")?.get("content")?;

    if let Some(text) = content.as_str() {
        let cleaned = clean_command_tags(text);
        return if cleaned.is_empty() { None } else { Some(cleaned) };
    }

    if let Some(blocks) = content.as_array() {
        let text_parts: Vec<&str> = blocks
            .iter()
            .filter_map(|b| {
                if b.get("type")?.as_str()? == "text" {
                    b.get("text")?.as_str()
                } else {
                    None
                }
            })
            .collect();

        if text_parts.is_empty() {
            return None;
        }

        let joined = text_parts.join("\n");
        let cleaned = clean_command_tags(&joined);
        return if cleaned.is_empty() { None } else { Some(cleaned) };
    }

    None
}

fn clean_command_tags(text: &str) -> String {
    regex_lite::Regex::new(r"<command-[^>]*>[^<]*</command-[^>]*>")
        .unwrap()
        .replace_all(text, "")
        .trim()
        .to_string()
}

struct ResolvedProject {
    full_path: String,
    display_name: String,
}

/// Resolve an encoded directory name to its actual filesystem path
fn resolve_project_path(encoded_name: &str) -> ResolvedProject {
    // Basic decode: -Users-foo-bar -> /Users/foo/bar
    let basic_decode = format!(
        "/{}",
        encoded_name
            .trim_start_matches('-')
            .replace("--", "/@")
            .replace('-', "/")
    );

    // Check if basic decode exists
    if Path::new(&basic_decode).exists() {
        return ResolvedProject {
            full_path: basic_decode[1..].to_string(), // Remove leading /
            display_name: Path::new(&basic_decode)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(encoded_name)
                .to_string(),
        };
    }

    // Try recursive resolution for paths with hyphens in directory names
    let segments: Vec<&str> = basic_decode[1..].split('/').collect();
    if let Some(resolved) = resolve_segments(&segments, "") {
        return ResolvedProject {
            full_path: resolved[1..].to_string(),
            display_name: Path::new(&resolved)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(encoded_name)
                .to_string(),
        };
    }

    // Fallback
    ResolvedProject {
        full_path: basic_decode[1..].to_string(),
        display_name: Path::new(&basic_decode)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(encoded_name)
            .to_string(),
    }
}

fn resolve_segments(segments: &[&str], current_path: &str) -> Option<String> {
    if segments.is_empty() {
        return if current_path.is_empty() {
            None
        } else {
            Some(current_path.to_string())
        };
    }

    // Try joining progressively more segments
    for join_count in (1..=segments.len()).rev() {
        let to_join = &segments[..join_count];

        for joined in get_join_variants(to_join) {
            let test_path = if current_path.is_empty() {
                format!("/{}", joined)
            } else {
                format!("{}/{}", current_path, joined)
            };

            if Path::new(&test_path).exists() {
                let remaining = &segments[join_count..];
                if remaining.is_empty() {
                    return Some(test_path);
                }
                if let Some(result) = resolve_segments(remaining, &test_path) {
                    return Some(result);
                }
            }
        }
    }

    None
}

fn get_join_variants(segments: &[&str]) -> Vec<String> {
    if segments.len() == 1 {
        return vec![segments[0].to_string()];
    }

    let mut variants = Vec::new();

    // All hyphens (e.g., "claude-view")
    variants.push(segments.join("-"));

    // Dot before last segment (e.g., "famatch.io")
    if segments.len() == 2 {
        variants.push(segments.join("."));
    } else if segments.len() > 2 {
        let before_last = segments[..segments.len() - 1].join("-");
        variants.push(format!("{}.{}", before_last, segments.last().unwrap()));
    }

    variants
}
```

**Step 2: Add dependencies to core**

Update `crates/core/Cargo.toml`:

```toml
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["fs", "io-util"] }
regex-lite = "0.1"
dirs = "5"
chrono = { version = "0.4", features = ["serde"] }
```

**Step 3: Update lib.rs**

```rust
// crates/core/src/lib.rs
pub mod types;
pub mod parser;
pub mod discovery;

pub use types::*;
pub use parser::*;
pub use discovery::*;
```

**Step 4: Verify it compiles**

Run: `cargo build -p vibe-recall-core`
Expected: Compiles successfully

**Step 5: Commit**

```bash
git add crates/core/
git commit -m "feat(core): add project discovery with path resolution

- Scans ~/.claude/projects/ for sessions
- Resolves encoded directory names to actual paths
- Extracts rich metadata from session files
- Handles directory names with hyphens

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 5: Basic Axum Server

**Files:**
- Create: `crates/server/src/state.rs`
- Create: `crates/server/src/routes/mod.rs`
- Create: `crates/server/src/routes/projects.rs`
- Create: `crates/server/src/routes/sessions.rs`
- Modify: `crates/server/src/lib.rs`
- Modify: `crates/server/src/main.rs`

**Step 1: Create state.rs**

```rust
// crates/server/src/state.rs
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    // Future: db pool, search index
}

impl AppState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }
}
```

**Step 2: Create routes/mod.rs**

```rust
// crates/server/src/routes/mod.rs
pub mod projects;
pub mod sessions;

use axum::Router;
use std::sync::Arc;
use crate::state::AppState;

pub fn api_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(projects::routes(state.clone()))
        .merge(sessions::routes(state))
}
```

**Step 3: Create routes/projects.rs**

```rust
// crates/server/src/routes/projects.rs
use axum::{extract::State, routing::get, Json, Router};
use std::sync::Arc;
use vibe_recall_core::{get_projects, ProjectInfo};
use crate::state::AppState;

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/projects", get(list_projects))
        .with_state(state)
}

async fn list_projects(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Vec<ProjectInfo>>, (axum::http::StatusCode, String)> {
    match get_projects().await {
        Ok(projects) => Ok(Json(projects)),
        Err(e) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get projects: {}", e),
        )),
    }
}
```

**Step 4: Create routes/sessions.rs**

```rust
// crates/server/src/routes/sessions.rs
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use vibe_recall_core::{parse_session, ParsedSession, claude_projects_dir};
use crate::state::AppState;

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/session/:project_dir/:session_id", get(get_session))
        .with_state(state)
}

async fn get_session(
    State(_state): State<Arc<AppState>>,
    Path((project_dir, session_id)): Path<(String, String)>,
) -> Result<Json<ParsedSession>, (axum::http::StatusCode, String)> {
    let file_path = claude_projects_dir()
        .join(&project_dir)
        .join(format!("{}.jsonl", session_id));

    match parse_session(&file_path).await {
        Ok(session) => Ok(Json(session)),
        Err(e) => {
            let status = if e.to_string().contains("not found") {
                axum::http::StatusCode::NOT_FOUND
            } else {
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            };
            Err((status, format!("Failed to parse session: {}", e)))
        }
    }
}
```

**Step 5: Update lib.rs**

```rust
// crates/server/src/lib.rs
pub mod routes;
pub mod state;

use axum::Router;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use routes::api_routes;
use state::AppState;

pub fn create_app() -> Router {
    let state = AppState::new();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .merge(api_routes(state))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}
```

**Step 6: Update main.rs**

```rust
// crates/server/src/main.rs
use anyhow::Result;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        ))
        .init();

    let app = vibe_recall_server::create_app();

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(47892);

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

    tracing::info!("vibe-recall server running at http://localhost:{}", port);

    axum::serve(listener, app).await?;

    Ok(())
}
```

**Step 7: Verify it compiles and runs**

Run: `cargo run -p vibe-recall-server`
Expected: Server starts on port 47892

Test API:
```bash
curl http://localhost:47892/api/projects | jq
```
Expected: JSON array of projects

**Step 8: Commit**

```bash
git add crates/server/
git commit -m "feat(server): add Axum server with projects and sessions API

- GET /api/projects returns all projects with sessions
- GET /api/session/:project/:id returns parsed session
- CORS enabled for development
- Tracing middleware for request logging

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 6: Move Frontend and Serve Static Files

**Files:**
- Move: `src/` → `frontend/src/`
- Move: `index.html` → `frontend/index.html`
- Move: `public/` → `frontend/public/`
- Create: `frontend/package.json`
- Create: `frontend/vite.config.ts`
- Modify: `crates/server/src/main.rs`
- Delete: Old `src/server/` directory

**Step 1: Create frontend directory structure**

```bash
mkdir -p frontend
mv src/components frontend/src/
mv src/hooks frontend/src/
mv src/lib frontend/src/
mv src/store frontend/src/
mv src/App.tsx frontend/src/
mv src/main.tsx frontend/src/
mv src/router.tsx frontend/src/
mv src/index.css frontend/src/
mv index.html frontend/
mv public frontend/
mv vite.config.ts frontend/
mv tsconfig.app.json frontend/
mv tailwind.config.js frontend/ 2>/dev/null || true
mv postcss.config.js frontend/ 2>/dev/null || true
```

**Step 2: Create frontend/package.json**

```json
{
  "name": "vibe-recall-frontend",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "@tanstack/react-query": "^5.90.18",
    "clsx": "^2.1.1",
    "lucide-react": "^0.562.0",
    "prism-react-renderer": "^2.3.1",
    "react": "^19.2.0",
    "react-dom": "^19.2.0",
    "react-markdown": "^9.0.1",
    "react-router-dom": "^7.13.0",
    "react-virtuoso": "^4.18.1",
    "remark-gfm": "^4.0.0",
    "tailwind-merge": "^3.4.0",
    "zustand": "^5.0.10"
  },
  "devDependencies": {
    "@types/react": "^19.1.6",
    "@types/react-dom": "^19.1.5",
    "@vitejs/plugin-react": "^4.3.4",
    "autoprefixer": "^10.4.20",
    "postcss": "^8.4.49",
    "tailwindcss": "^3.4.17",
    "typescript": "~5.8.3",
    "vite": "^6.0.5"
  }
}
```

**Step 3: Update frontend/vite.config.ts**

```typescript
// frontend/vite.config.ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:47892',
        changeOrigin: true
      }
    }
  },
  build: {
    outDir: '../dist/frontend',
    emptyOutDir: true
  }
})
```

**Step 4: Update Axum to serve static files in production**

```rust
// crates/server/src/main.rs
use anyhow::Result;
use axum::Router;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        ))
        .init();

    let app = vibe_recall_server::create_app();

    // Serve static frontend in production
    let app = if std::env::var("VIBE_RECALL_DEV").is_err() {
        let frontend_dir = get_frontend_dir();
        if frontend_dir.exists() {
            let serve_dir = ServeDir::new(&frontend_dir)
                .not_found_service(ServeFile::new(frontend_dir.join("index.html")));

            app.fallback_service(serve_dir)
        } else {
            tracing::warn!("Frontend directory not found: {:?}", frontend_dir);
            app
        }
    } else {
        app
    };

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(47892);

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

    tracing::info!("vibe-recall server running at http://localhost:{}", port);

    axum::serve(listener, app).await?;

    Ok(())
}

fn get_frontend_dir() -> PathBuf {
    // Check relative to binary first (production)
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    if let Some(dir) = exe_dir {
        let frontend = dir.join("frontend");
        if frontend.exists() {
            return frontend;
        }
    }

    // Fallback to dist/frontend (development build)
    PathBuf::from("dist/frontend")
}
```

**Step 5: Delete old server code**

```bash
rm -rf src/server
rm src/lib/export-html.ts 2>/dev/null || true
```

**Step 6: Update root package.json**

```json
{
  "name": "vibe-recall",
  "version": "0.1.0",
  "description": "Browse and search Claude Code conversation history",
  "scripts": {
    "dev": "cd frontend && npm run dev",
    "dev:rust": "VIBE_RECALL_DEV=1 cargo run -p vibe-recall-server",
    "build": "cd frontend && npm run build && cargo build --release -p vibe-recall-server",
    "start": "cargo run --release -p vibe-recall-server"
  }
}
```

**Step 7: Create pnpm-workspace.yaml**

```yaml
packages:
  - frontend
  - npx-cli
```

**Step 8: Test the setup**

Terminal 1:
```bash
VIBE_RECALL_DEV=1 cargo run -p vibe-recall-server
```

Terminal 2:
```bash
cd frontend && npm install && npm run dev
```

Open http://localhost:5173 - should show the UI with data from Rust backend

**Step 9: Commit**

```bash
git add -A
git commit -m "refactor: move frontend to dedicated directory, serve via Axum

- Frontend moved to frontend/
- Vite proxies /api to Rust backend in dev
- Axum serves static files in production
- Old Express server code removed

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 7: Clean Up and Verify

**Step 1: Remove old Node.js server files**

```bash
rm -rf src/server 2>/dev/null || true
rm tsconfig.server.json 2>/dev/null || true
```

**Step 2: Update .gitignore**

```
# Rust
/target/
Cargo.lock

# Node
node_modules/
dist/

# IDE
.idea/
.vscode/

# OS
.DS_Store

# Data
*.db
*.db-journal
```

**Step 3: Full build test**

```bash
# Build frontend
cd frontend && npm install && npm run build && cd ..

# Build Rust
cargo build --release -p vibe-recall-server

# Run production mode
./target/release/vibe-recall
```

Open http://localhost:47892 - should show working UI

**Step 4: Commit**

```bash
git add -A
git commit -m "chore: clean up old files, update gitignore

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Summary

After completing Phase 1, you have:

1. **Rust workspace** with 4 crates (core, db, search, server)
2. **Core crate** with types and JSONL parser
3. **Server crate** with Axum API endpoints
4. **Frontend** moved to dedicated directory
5. **Working MVP** that replaces Express with Rust

Next: **Phase 2** will add Tantivy search and the db crate with SQLite.

---

*Plan created: 2026-01-27*
