// crates/providers/src/parsers/iflow/command.rs
//
// Command-envelope normalization and system-message detection, ported from
// agentsview's extractCommandText / isCommandEnvelope / isIflowSystemMessage
// (hand-rolled tag scanning replaces the Go regexes; inner tag text must be
// '<'-free, matching the `[^<]*` patterns).

const CMD_TAGS: [&str; 3] = ["name", "message", "args"];

const SYSTEM_PREFIXES: [&str; 5] = [
    "This session is being continued",
    "[Request interrupted",
    "<task-notification>",
    "<local-command-",
    "Stop hook feedback:",
];

fn trim_lead(s: &str) -> &str {
    s.trim_start_matches(|c: char| c == '\u{feff}' || c.is_whitespace())
}

/// True when content matches a known system-injected user message pattern.
pub(super) fn is_system_message(content: &str) -> bool {
    let trimmed = trim_lead(content);
    SYSTEM_PREFIXES.iter().any(|p| trimmed.starts_with(p))
}

/// Normalize a command/skill invocation envelope into "/name args".
/// Only pure envelopes match (prose that merely mentions the tags stays
/// untouched). Returns None when content is not a command message.
pub(super) fn extract_command_text(content: &str) -> Option<String> {
    let trimmed = trim_lead(content);
    if !trimmed.starts_with("<command-message>") && !trimmed.starts_with("<command-name>") {
        return None;
    }
    if !strip_command_tags(trimmed).trim().is_empty() {
        return None;
    }
    let args = tag_value(content, "args").map(str::trim).unwrap_or("");
    match tag_value(content, "name").filter(|n| !n.is_empty()) {
        Some(name) => {
            let name = if name.starts_with('/') {
                name.to_string()
            } else {
                format!("/{name}")
            };
            Some(if args.is_empty() {
                name
            } else {
                format!("{name} {args}")
            })
        }
        // Bare <command-message> without <command-name>: fall back to the
        // message value.
        None => tag_value(content, "message")
            .filter(|m| !m.is_empty())
            .map(|m| format!("/{m}")),
    }
}

/// True for a pure command XML envelope — used to drop messages that look
/// like envelopes but couldn't be normalized.
pub(super) fn is_command_envelope(content: &str) -> bool {
    let trimmed = trim_lead(content);
    (trimmed.starts_with("<command-message>") || trimmed.starts_with("<command-name>"))
        && strip_command_tags(trimmed).trim().is_empty()
}

/// First `<command-TAG>value</command-…>` value whose inner text is
/// '<'-free.
fn tag_value<'a>(content: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<command-{tag}>");
    let close = format!("</command-{tag}>");
    let mut rest = content;
    while let Some(pos) = rest.find(open.as_str()) {
        let after = &rest[pos + open.len()..];
        if let Some(lt) = after.find('<') {
            if after[lt..].starts_with(close.as_str()) {
                return Some(&after[..lt]);
            }
        }
        rest = after;
    }
    None
}

/// Remove every well-formed command tag pair; a pure envelope leaves only
/// whitespace behind (mirrors the Go xmlCmdStripRe sweep).
fn strip_command_tags(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    'scan: while let Some(pos) = rest.find("<command-") {
        let (head, tail) = rest.split_at(pos);
        for tag in CMD_TAGS {
            let Some(after_open) = tail.strip_prefix(&format!("<command-{tag}>")) else {
                continue;
            };
            let Some(lt) = after_open.find('<') else {
                continue;
            };
            for close_tag in CMD_TAGS {
                let close = format!("</command-{close_tag}>");
                if let Some(after_close) = after_open[lt..].strip_prefix(close.as_str()) {
                    out.push_str(head);
                    rest = after_close;
                    continue 'scan;
                }
            }
        }
        out.push_str(head);
        out.push_str("<command-");
        rest = &tail["<command-".len()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_envelope_helpers() {
        assert_eq!(
            extract_command_text("<command-name>clear</command-name>").as_deref(),
            Some("/clear")
        );
        assert_eq!(
            extract_command_text("<command-message>review</command-message>").as_deref(),
            Some("/review"),
            "bare command-message falls back to the message value"
        );
        assert_eq!(
            extract_command_text(
                "\u{feff} <command-name>goal</command-name><command-args> a b </command-args>"
            )
            .as_deref(),
            Some("/goal a b")
        );
        // Prose mentioning the tags is NOT an envelope.
        assert_eq!(
            extract_command_text("see <command-name>x</command-name> in docs"),
            None
        );
        assert!(!is_command_envelope(
            "see <command-name>x</command-name> in docs"
        ));
        // Empty name with no message: envelope, but un-normalizable.
        assert_eq!(extract_command_text("<command-name></command-name>"), None);
        assert!(is_command_envelope("<command-name></command-name>"));
        assert!(is_system_message("  [Request interrupted by user]"));
        assert!(!is_system_message("normal prompt"));
    }
}
