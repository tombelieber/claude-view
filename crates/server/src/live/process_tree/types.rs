use serde::Serialize;
use ts_rs::TS;

/// Which top-level section a process belongs to.
/// Serializes as PascalCase to match TypeScript enum convention for discriminated unions.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
pub enum ProcessCategory {
    /// Claude itself + claude-view — the "brain" processes.
    ClaudeEcosystem,
    /// Spawned by a Claude ecosystem process — build tools, dev servers, file watchers, etc.
    ChildProcess,
}

/// Fine-grained tag within `ClaudeEcosystem`.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub enum EcosystemTag {
    /// `claude` binary launched from a terminal/shell.
    Cli,
    /// VS Code extension instance (anthropic.claude-code).
    Ide,
    /// Claude.app main process or Electron helper.
    Desktop,
    /// claude-view server (ourselves).
    #[serde(rename = "self")]
    Self_,
}

/// Staleness heuristic for unparented processes.
/// Serializes as PascalCase intentionally (matches discriminated union convention).
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
pub enum Staleness {
    /// Recent CPU activity (>0.1%) or recently started (<60s).
    Active,
    /// <0.1% CPU but has a parent, or PPID=1 but young (<5min).
    Idle,
    /// PPID=1 AND <0.1% CPU AND uptime >=5min. Conservative heuristic --
    /// NOT a guarantee the process is dead; user decides.
    LikelyStale,
}

/// A single OS process, classified and enriched.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ClassifiedProcess {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    /// Full command line. Truncated to 512 chars AFTER classification so path
    /// matching still works. Empty string means sysinfo could not read it.
    pub command: String,
    pub category: ProcessCategory,
    pub ecosystem_tag: Option<EcosystemTag>,
    pub cpu_percent: f32,
    #[ts(type = "number")]
    pub memory_bytes: u64,
    /// Approximate uptime: `now.saturating_sub(start_time)`.
    #[ts(type = "number")]
    pub uptime_secs: u64,
    /// Unix timestamp when process started. i64 for timestamp consistency.
    #[ts(type = "number")]
    pub start_time: i64,
    pub is_unparented: bool,
    pub staleness: Staleness,
    pub descendant_count: u32,
    /// Total CPU% across all descendants. Use `total_cmp` for any float sort.
    pub descendant_cpu: f32,
    #[ts(type = "number")]
    pub descendant_memory: u64,
    pub descendants: Vec<ClassifiedProcess>,
    pub is_self: bool,
}

/// The full classified process tree snapshot.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ProcessTreeSnapshot {
    #[ts(type = "number")]
    pub timestamp: i64,
    pub ecosystem: Vec<ClassifiedProcess>,
    pub children: Vec<ClassifiedProcess>,
    pub totals: ProcessTreeTotals,
}

/// Aggregate statistics across the classified tree.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ProcessTreeTotals {
    pub ecosystem_cpu: f32,
    #[ts(type = "number")]
    pub ecosystem_memory: u64,
    pub ecosystem_count: u32,
    pub child_cpu: f32,
    #[ts(type = "number")]
    pub child_memory: u64,
    pub child_count: u32,
    pub unparented_count: u32,
    #[ts(type = "number")]
    pub unparented_memory: u64,
}

/// Raw process data collected from sysinfo before classification.
#[derive(Debug)]
pub(super) struct RawProcessInfo {
    pub(super) pid: u32,
    pub(super) ppid: u32,
    pub(super) name: String,
    pub(super) command: String,
    pub(super) cpu_percent: f32,
    pub(super) memory_bytes: u64,
    pub(super) start_time: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_category_serializes_as_pascal_case() {
        let json = serde_json::to_value(ProcessCategory::ClaudeEcosystem).unwrap();
        assert_eq!(json, "ClaudeEcosystem");
        let json = serde_json::to_value(ProcessCategory::ChildProcess).unwrap();
        assert_eq!(json, "ChildProcess");
    }

    #[test]
    fn ecosystem_tag_serializes_as_camel_case() {
        let json = serde_json::to_value(EcosystemTag::Cli).unwrap();
        assert_eq!(json, "cli");
        let json = serde_json::to_value(EcosystemTag::Ide).unwrap();
        assert_eq!(json, "ide");
        let json = serde_json::to_value(EcosystemTag::Desktop).unwrap();
        assert_eq!(json, "desktop");
        let json = serde_json::to_value(EcosystemTag::Self_).unwrap();
        assert_eq!(json, "self");
    }

    #[test]
    fn staleness_serializes_as_pascal_case() {
        let json = serde_json::to_value(Staleness::Active).unwrap();
        assert_eq!(json, "Active");
        let json = serde_json::to_value(Staleness::LikelyStale).unwrap();
        assert_eq!(json, "LikelyStale");
    }

    #[test]
    fn classified_process_serializes_to_camel_case() {
        let proc = ClassifiedProcess {
            pid: 1234,
            ppid: 0,
            name: "claude".to_string(),
            command: "/usr/bin/claude".to_string(),
            category: ProcessCategory::ClaudeEcosystem,
            ecosystem_tag: Some(EcosystemTag::Cli),
            cpu_percent: 0.0,
            memory_bytes: 0,
            uptime_secs: 0,
            start_time: 0,
            is_unparented: false,
            staleness: Staleness::Active,
            descendant_count: 0,
            descendant_cpu: 0.0,
            descendant_memory: 0,
            descendants: vec![],
            is_self: false,
        };
        let json = serde_json::to_value(&proc).unwrap();
        assert_eq!(json["pid"], 1234);
        assert_eq!(json["ecosystemTag"], "cli");
        assert!(json.get("ecosystem_tag").is_none(), "must be camelCase");
        assert_eq!(json["memoryBytes"], 0u64);
        assert!(json.get("memory_bytes").is_none(), "must be camelCase");
    }
}
