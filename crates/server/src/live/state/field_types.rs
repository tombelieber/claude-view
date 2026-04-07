//! Small field types used across multiple state structs.

use serde::Serialize;
use ts_rs::TS;

/// A tool integration (MCP server or skill) detected from actual usage in a session.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsed {
    /// Display name: "playwright", "chrome-devtools" for MCP; "commit", "review-pr" for skills.
    pub name: String,
    /// Category: "mcp" or "skill".
    pub kind: String,
}

/// A verified file reference detected from user messages.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedFile {
    /// Absolute path (verified to exist via stat()).
    pub path: String,
    /// How this file was detected.
    pub kind: FileSourceKind,
    /// Project-relative path for UI display.
    pub display_name: String,
}

/// How a file reference was detected in user messages.
#[derive(Debug, Clone, Serialize, TS)]
#[cfg_attr(
    feature = "codegen",
    ts(
        export,
        export_to = "../../../../../packages/shared/src/types/generated/"
    )
)]
#[serde(rename_all = "snake_case")]
pub enum FileSourceKind {
    /// @file mention in user message.
    Mention,
    /// <ide_opened_file> tag from IDE.
    Ide,
    /// Bare absolute path pasted in message.
    Pasted,
}

/// Helper for `#[serde(skip_serializing_if)]` on `u32` fields.
pub(crate) fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_file_serializes_to_camel_case() {
        let file = VerifiedFile {
            path: "/Users/dev/project/src/auth.rs".into(),
            kind: FileSourceKind::Mention,
            display_name: "src/auth.rs".into(),
        };
        let json = serde_json::to_value(&file).unwrap();
        assert_eq!(json["path"], "/Users/dev/project/src/auth.rs");
        assert_eq!(json["kind"], "mention");
        assert_eq!(json["displayName"], "src/auth.rs");
    }

    #[test]
    fn file_source_kind_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_value(FileSourceKind::Mention).unwrap(),
            "mention"
        );
        assert_eq!(serde_json::to_value(FileSourceKind::Ide).unwrap(), "ide");
        assert_eq!(
            serde_json::to_value(FileSourceKind::Pasted).unwrap(),
            "pasted"
        );
    }
}
