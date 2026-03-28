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
#[serde(tag = "type")]
pub enum ComponentDetails {
    #[serde(rename = "sidecar", rename_all = "camelCase")]
    Sidecar { session_count: Option<u32> },
    #[serde(rename = "omlx", rename_all = "camelCase")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_kind_serializes_pascal_case() {
        assert_eq!(
            serde_json::to_string(&ComponentKind::ExternalService).unwrap(),
            "\"ExternalService\""
        );
        assert_eq!(
            serde_json::to_string(&ComponentKind::ChildProcess).unwrap(),
            "\"ChildProcess\""
        );
    }

    #[test]
    fn component_details_tagged_union() {
        let details = ComponentDetails::Sidecar {
            session_count: Some(3),
        };
        let json = serde_json::to_value(&details).unwrap();
        assert_eq!(json["type"], "sidecar");
        assert_eq!(json["sessionCount"], 3);
    }

    #[test]
    fn component_status_camel_case_fields() {
        let status = ComponentStatus {
            name: "test".into(),
            kind: ComponentKind::ChildProcess,
            enabled: true,
            running: true,
            pid: Some(1234),
            cpu_percent: 5.0,
            memory_bytes: 1024,
            details: ComponentDetails::Sidecar {
                session_count: None,
            },
        };
        let json = serde_json::to_value(&status).unwrap();
        // Verify camelCase field names
        assert!(json.get("cpuPercent").is_some());
        assert!(json.get("memoryBytes").is_some());
        // Verify PascalCase kind
        assert_eq!(json["kind"], "ChildProcess");
    }
}
