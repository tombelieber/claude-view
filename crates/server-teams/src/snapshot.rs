//! Team snapshot (backup before TeamDelete cleanup).

use std::path::Path;

/// Recursively copy src/ into dst/, creating dirs as needed. Overwrites existing files.
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Snapshot `~/.claude/teams/{name}/` -> `~/.claude-view/{session_id}/teams/{name}/`.
///
/// Keyed by session_id to avoid collisions when different sessions reuse the
/// same team name. Layout: `{claude_view_dir}/{session_id}/teams/{team_name}/`.
///
/// Called when `TeamDelete` tool_use is detected in a JSONL line (PreToolUse timing --
/// files still exist). The backup survives Claude Code's cleanup hook and is used
/// as a fallback in `TeamsStore::get()` / `TeamsStore::inbox()`.
pub fn snapshot_team(
    team_name: &str,
    session_id: &str,
    claude_dir: &Path,
    claude_view_dir: &Path,
) -> std::io::Result<()> {
    let src = claude_dir.join("teams").join(team_name);
    if !src.exists() {
        return Ok(());
    }
    let dst = claude_view_dir
        .join(session_id)
        .join("teams")
        .join(team_name);
    copy_dir_all(&src, &dst)
}
