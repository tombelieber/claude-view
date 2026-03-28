use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
pub enum ComponentKind {
    ChildProcess,
    ExternalService,
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ComponentDetails {
    #[serde(rename = "sidecar")]
    Sidecar { session_count: Option<u32> },
    #[serde(rename = "omlx")]
    Omlx {
        model_id: String,
        port: u16,
        healthy: bool,
    },
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ComponentStatus {
    pub name: String,
    pub kind: ComponentKind,
    pub enabled: bool,
    pub running: bool,
    pub pid: Option<u32>,
    pub cpu_percent: f32,
    #[ts(type = "number")]
    pub memory_bytes: u64,
    pub details: ComponentDetails,
}

#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(export, export_to = "../../../../../apps/web/src/types/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ComponentSnapshot {
    pub components: Vec<ComponentStatus>,
    pub build_mode: String,
}
