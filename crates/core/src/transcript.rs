//! Parse `<teammate-message>` XML blocks from user-role string content.
//!
//! Claude Code wraps teammate messages in XML tags:
//! ```xml
//! <teammate-message teammate_id="NAME" color="COLOR" summary="SUMMARY">
//! CONTENT
//! </teammate-message>
//! ```
//! Multiple blocks can appear in a single string. Content is either
//! markdown (substantive) or JSON with a `type` field (protocol).

use regex_lite::Regex;
use std::sync::OnceLock;

/// Parsed content from a `<teammate-message>` block.
#[derive(Debug, Clone)]
pub struct TeammateMessageParsed {
    pub teammate_id: String,
    pub color: Option<String>,
    pub summary: Option<String>,
    pub content: TeammateContent,
}

/// Content classification.
#[derive(Debug, Clone)]
pub enum TeammateContent {
    /// Substantive markdown message (debate argument, response, etc.)
    Message(String),
    /// Protocol message (idle, shutdown, terminated)
    Protocol { msg_type: String, raw: String },
}

static TEAMMATE_MSG_RE: OnceLock<Regex> = OnceLock::new();
static ATTR_RE: OnceLock<Regex> = OnceLock::new();

fn teammate_msg_re() -> &'static Regex {
    TEAMMATE_MSG_RE.get_or_init(|| {
        Regex::new(r#"(?s)<teammate-message\s+([^>]*)>(.*?)</teammate-message>"#)
            .expect("teammate-message regex must compile")
    })
}

fn attr_re() -> &'static Regex {
    ATTR_RE.get_or_init(|| Regex::new(r#"(\w+)="([^"]*)""#).expect("attr regex must compile"))
}

const PROTOCOL_TYPES: &[&str] = &[
    "idle_notification",
    "shutdown_approved",
    "shutdown_request",
    "teammate_terminated",
];

/// Parse all `<teammate-message>` blocks from a user-role string.
pub fn parse_teammate_messages(input: &str) -> Vec<TeammateMessageParsed> {
    teammate_msg_re()
        .captures_iter(input)
        .map(|cap| {
            let attrs_str = &cap[1];
            let body = cap[2].trim().to_string();

            let mut teammate_id = String::new();
            let mut color = None;
            let mut summary = None;

            for attr in attr_re().captures_iter(attrs_str) {
                match &attr[1] {
                    "teammate_id" => teammate_id = attr[2].to_string(),
                    "color" => color = Some(attr[2].to_string()),
                    "summary" => summary = Some(attr[2].to_string()),
                    _ => {}
                }
            }

            let content = classify_content(&body);

            TeammateMessageParsed {
                teammate_id,
                color,
                summary,
                content,
            }
        })
        .collect()
}

/// Classify body as Protocol (JSON with known `type`) or Message (markdown).
fn classify_content(body: &str) -> TeammateContent {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg_type) = json.get("type").and_then(|t| t.as_str()) {
            if PROTOCOL_TYPES.contains(&msg_type) {
                return TeammateContent::Protocol {
                    msg_type: msg_type.to_string(),
                    raw: body.to_string(),
                };
            }
        }
    }
    TeammateContent::Message(body.to_string())
}

/// Words that should be rendered in ALL CAPS.
const UPPERCASE_WORDS: &[&str] = &["ai", "api", "ui", "ux", "ml", "llm", "id"];

/// Convert a teammate ID like "pro-ai" to a display name like "Pro AI".
pub fn make_display_name(id: &str) -> String {
    id.split('-')
        .map(|word| {
            if UPPERCASE_WORDS.contains(&word) {
                return word.to_uppercase();
            }
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_message() {
        let input = r#"<teammate-message teammate_id="pro-ai" color="blue" summary="Opening argument">
**Opening Statement**

AI makes devs better.
</teammate-message>"#;
        let msgs = parse_teammate_messages(input);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].teammate_id, "pro-ai");
        assert_eq!(msgs[0].color.as_deref(), Some("blue"));
        assert_eq!(msgs[0].summary.as_deref(), Some("Opening argument"));
        assert!(
            matches!(&msgs[0].content, TeammateContent::Message(t) if t.contains("AI makes devs better"))
        );
    }

    #[test]
    fn parse_multiple_messages_interleaved() {
        let input = r#"<teammate-message teammate_id="pro-ai" color="blue" summary="Opening">
Pro argument here.
</teammate-message>

<teammate-message teammate_id="ai-skeptic" color="green" summary="Opening">
Skeptic argument here.
</teammate-message>

<teammate-message teammate_id="pro-ai" color="blue">
{"type":"idle_notification","from":"pro-ai","timestamp":"2026-04-06T17:11:53.582Z","idleReason":"available"}
</teammate-message>"#;
        let msgs = parse_teammate_messages(input);
        assert_eq!(msgs.len(), 3);
        assert!(matches!(&msgs[0].content, TeammateContent::Message(_)));
        assert!(matches!(&msgs[1].content, TeammateContent::Message(_)));
        assert!(
            matches!(&msgs[2].content, TeammateContent::Protocol { msg_type, .. } if msg_type == "idle_notification")
        );
    }

    #[test]
    fn parse_protocol_types() {
        for (json_type, expected) in [
            ("idle_notification", "idle_notification"),
            ("shutdown_approved", "shutdown_approved"),
            ("shutdown_request", "shutdown_request"),
            ("teammate_terminated", "teammate_terminated"),
        ] {
            let input = format!(
                r#"<teammate-message teammate_id="agent" color="blue">
{{"type":"{json_type}","from":"agent"}}
</teammate-message>"#
            );
            let msgs = parse_teammate_messages(&input);
            assert_eq!(msgs.len(), 1);
            assert!(
                matches!(&msgs[0].content, TeammateContent::Protocol { msg_type, .. } if msg_type == expected),
                "Expected protocol type {expected}"
            );
        }
    }

    #[test]
    fn parse_no_teammate_messages() {
        let input = "Just a regular user message with no XML tags.";
        let msgs = parse_teammate_messages(input);
        assert!(msgs.is_empty());
    }

    #[test]
    fn parse_missing_optional_attrs() {
        let input = r#"<teammate-message teammate_id="system">
{"type":"teammate_terminated","message":"pro-ai has shut down."}
</teammate-message>"#;
        let msgs = parse_teammate_messages(input);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].teammate_id, "system");
        assert_eq!(msgs[0].color, None);
        assert_eq!(msgs[0].summary, None);
    }

    #[test]
    fn display_name_from_id() {
        assert_eq!(make_display_name("pro-ai"), "Pro AI");
        assert_eq!(make_display_name("ai-skeptic"), "AI Skeptic");
        assert_eq!(make_display_name("tabs-advocate"), "Tabs Advocate");
        assert_eq!(make_display_name("team-lead"), "Team Lead");
        assert_eq!(make_display_name("alice"), "Alice");
    }
}
