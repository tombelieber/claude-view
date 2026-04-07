//! Live-reloading teams store.
//!
//! Re-scans `~/.claude/teams/` on every public method call. The directory
//! typically has <10 entries, each config.json <5KB -- total I/O is
//! microseconds, far cheaper than any staleness/cache-invalidation bug.

use super::jsonl_index::build_team_jsonl_index;
use super::jsonl_reconstruct::{
    reconstruct_inbox_from_jsonl, reconstruct_team_and_inbox_from_jsonl,
    reconstruct_team_from_jsonl,
};
use super::parser::parse_team;
use super::types::{InboxMessage, TeamDetail, TeamJSONLIndex, TeamSummary};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Live-reloading teams store.
pub struct TeamsStore {
    /// Path to `~/.claude` (or equivalent). `None` for empty/test stores.
    claude_dir: Option<PathBuf>,
    /// Path to `~/.claude-view` (backup dir). `None` for empty/test stores.
    /// Teams are snapshotted here before TeamDelete cleanup.
    claude_view_dir: Option<PathBuf>,
    /// Eagerly loaded snapshot (used for the initial log message at startup).
    pub teams: HashMap<String, TeamDetail>,
    pub inboxes: HashMap<String, Vec<InboxMessage>>,
    /// JSONL fallback index: team_name -> JSONL file refs.
    /// Populated at startup by scanning all session JSONL files.
    /// Used when a team's filesystem directory no longer exists.
    jsonl_index: TeamJSONLIndex,
}

/// Scan backup dir: `{cv_dir}/{session_id}/teams/{team_name}/`.
/// Merges into existing maps (primary wins via `.entry().or_insert()`).
fn scan_backup_teams(
    cv_dir: &Path,
    teams: &mut HashMap<String, TeamDetail>,
    inboxes: &mut HashMap<String, Vec<InboxMessage>>,
) {
    let Ok(session_dirs) = std::fs::read_dir(cv_dir) else {
        return;
    };
    for session_entry in session_dirs.flatten() {
        let session_path = session_entry.path();
        if !session_path.is_dir() {
            continue;
        }
        let teams_subdir = session_path.join("teams");
        if !teams_subdir.is_dir() {
            continue;
        }
        let Ok(team_dirs) = std::fs::read_dir(&teams_subdir) else {
            continue;
        };
        for team_entry in team_dirs.flatten() {
            let team_path = team_entry.path();
            if !team_path.is_dir() {
                continue;
            }
            if let Some((detail, inbox)) = parse_team(&team_path) {
                let name = detail.name.clone();
                teams.entry(name.clone()).or_insert(detail);
                inboxes.entry(name).or_insert(inbox);
            }
        }
    }
}

/// Internal: scan the teams directory and return (teams, inboxes).
fn scan_teams_dir(
    claude_dir: &Path,
) -> (
    HashMap<String, TeamDetail>,
    HashMap<String, Vec<InboxMessage>>,
) {
    let teams_dir = claude_dir.join("teams");
    let mut teams = HashMap::new();
    let mut inboxes = HashMap::new();

    let Ok(entries) = std::fs::read_dir(&teams_dir) else {
        return (teams, inboxes);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some((detail, inbox)) = parse_team(&path) {
            let name = detail.name.clone();
            teams.insert(name.clone(), detail);
            inboxes.insert(name, inbox);
        }
    }

    (teams, inboxes)
}

impl TeamsStore {
    /// Create an empty store (used by test constructors that don't need real teams).
    pub fn empty() -> Self {
        Self {
            claude_dir: None,
            claude_view_dir: None,
            teams: HashMap::new(),
            inboxes: HashMap::new(),
            jsonl_index: HashMap::new(),
        }
    }

    /// Scan ~/.claude/teams/ and build the JSONL fallback index.
    pub fn load(claude_dir: &Path) -> Self {
        Self::load_with_index(claude_dir, None)
    }

    /// Scan ~/.claude/teams/ + optional backup dir, and build the JSONL fallback index.
    pub fn load_with_backup(claude_dir: &Path, claude_view_dir: &Path) -> Self {
        Self::load_with_index(claude_dir, Some(claude_view_dir))
    }

    /// Scan ~/.claude/teams/ for filesystem teams, optionally merge from backup,
    /// and build the JSONL fallback index.
    fn load_with_index(claude_dir: &Path, claude_view_dir: Option<&Path>) -> Self {
        let (mut teams, mut inboxes) = scan_teams_dir(claude_dir);

        // Merge backup: {claude_view_dir}/{session_id}/teams/{team_name}/
        if let Some(vdir) = claude_view_dir {
            scan_backup_teams(vdir, &mut teams, &mut inboxes);
        }

        let jsonl_index = build_team_jsonl_index(claude_dir);

        for (name, detail) in &teams {
            let msg_count = inboxes.get(name).map_or(0, |i| i.len());
            tracing::info!(
                "Loaded team '{}': {} members, {} inbox messages",
                name,
                detail.members.len(),
                msg_count,
            );
        }

        let jsonl_only: Vec<_> = jsonl_index
            .keys()
            .filter(|name| !teams.contains_key(*name))
            .collect();
        if !jsonl_only.is_empty() {
            tracing::info!(
                "Found {} teams in JSONL only (filesystem deleted): {:?}",
                jsonl_only.len(),
                jsonl_only,
            );
        }

        tracing::info!(
            "Loaded {} teams from disk + {} in JSONL index",
            teams.len(),
            jsonl_index.len(),
        );

        Self {
            claude_dir: Some(claude_dir.to_path_buf()),
            claude_view_dir: claude_view_dir.map(Path::to_path_buf),
            teams,
            inboxes,
            jsonl_index,
        }
    }

    /// Re-scan disk and update the in-memory snapshot.
    /// Reads primary (`~/.claude/`) first, merges backup (`~/.claude-view/`) for
    /// teams not found in primary.
    fn refresh(
        &self,
    ) -> (
        HashMap<String, TeamDetail>,
        HashMap<String, Vec<InboxMessage>>,
    ) {
        let (mut teams, mut inboxes) = match &self.claude_dir {
            Some(dir) => scan_teams_dir(dir),
            None => (HashMap::new(), HashMap::new()),
        };
        // Merge backup: {claude_view_dir}/{session_id}/teams/{team_name}/
        if let Some(ref vdir) = self.claude_view_dir {
            scan_backup_teams(vdir, &mut teams, &mut inboxes);
        }
        (teams, inboxes)
    }

    /// Build summary list for the /api/teams index endpoint.
    /// Re-scans the teams directory to pick up teams created after server start.
    pub fn summaries(&self) -> Vec<TeamSummary> {
        let (teams, inboxes) = self.refresh();
        let mut summaries: Vec<_> = teams
            .values()
            .map(|t| {
                let inbox = inboxes.get(&t.name);
                let msg_count = inbox.map_or(0, |i| i.len() as u32);
                let mut models: Vec<_> = t.members.iter().map(|m| m.model.clone()).collect();
                models.sort();
                models.dedup();

                // Estimate duration from first to last message timestamp.
                let duration = inbox.filter(|i| !i.is_empty()).and_then(|i| {
                    let first = chrono::DateTime::parse_from_rfc3339(&i[0].timestamp).ok()?;
                    let last =
                        chrono::DateTime::parse_from_rfc3339(&i[i.len() - 1].timestamp).ok()?;
                    Some((last - first).num_seconds().max(0) as u32)
                });

                TeamSummary {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    created_at: t.created_at,
                    lead_session_id: t.lead_session_id.clone(),
                    member_count: t.members.len() as u32,
                    message_count: msg_count,
                    duration_estimate_secs: duration,
                    models,
                }
            })
            .collect();
        // Add JSONL-only teams (not on filesystem).
        // Uses combined single-pass reconstruction to avoid reading each JSONL file twice.
        for (team_name, refs) in &self.jsonl_index {
            if teams.contains_key(team_name) {
                continue; // Already have this team from filesystem
            }
            if let Some((detail, inbox)) = reconstruct_team_and_inbox_from_jsonl(team_name, refs) {
                let msg_count = inbox.len() as u32;
                let mut models: Vec<_> = detail.members.iter().map(|m| m.model.clone()).collect();
                models.sort();
                models.dedup();

                let duration = if inbox.len() >= 2 {
                    let first = chrono::DateTime::parse_from_rfc3339(&inbox[0].timestamp).ok();
                    let last =
                        chrono::DateTime::parse_from_rfc3339(&inbox[inbox.len() - 1].timestamp)
                            .ok();
                    first
                        .zip(last)
                        .map(|(f, l)| (l - f).num_seconds().max(0) as u32)
                } else {
                    None
                };

                summaries.push(TeamSummary {
                    name: detail.name,
                    description: detail.description,
                    created_at: detail.created_at,
                    lead_session_id: detail.lead_session_id,
                    member_count: detail.members.len() as u32,
                    message_count: msg_count,
                    duration_estimate_secs: duration,
                    models,
                });
            }
        }

        summaries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        summaries
    }

    /// Look up a team by name (re-scans disk, falls back to JSONL index).
    pub fn get(&self, name: &str) -> Option<TeamDetail> {
        let (teams, _) = self.refresh();
        // Filesystem first
        if let Some(detail) = teams.get(name) {
            return Some(detail.clone());
        }
        // JSONL fallback -- reconstruct from session logs
        if let Some(refs) = self.jsonl_index.get(name) {
            return reconstruct_team_from_jsonl(name, refs);
        }
        None
    }

    /// Look up inbox messages for a team (re-scans disk, falls back to JSONL index).
    pub fn inbox(&self, name: &str) -> Option<Vec<InboxMessage>> {
        let (_, inboxes) = self.refresh();
        // Filesystem first
        if let Some(msgs) = inboxes.get(name) {
            return Some(msgs.clone());
        }
        // JSONL fallback -- return reconstructed inbox (may be empty vec)
        if self.jsonl_index.contains_key(name) {
            let refs = &self.jsonl_index[name];
            return Some(reconstruct_inbox_from_jsonl(name, refs));
        }
        None
    }
}
