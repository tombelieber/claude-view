//! Query string parsing: tokenization, qualifier extraction, session ID detection.

/// A parsed qualifier extracted from the query string.
#[derive(Debug, Clone)]
pub(crate) struct Qualifier {
    pub key: String,
    pub value: String,
}

/// Check if the query string is a bare UUID (session ID).
/// Pattern: 8-4-4-4-12 hex digits, case-insensitive, whitespace-trimmed.
pub(crate) fn is_session_id(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.len() != 36 {
        return false;
    }
    let bytes = trimmed.as_bytes();
    // Check hyphens at positions 8, 13, 18, 23
    if bytes[8] != b'-' || bytes[13] != b'-' || bytes[18] != b'-' || bytes[23] != b'-' {
        return false;
    }
    // Check all other chars are hex digits
    trimmed
        .chars()
        .enumerate()
        .all(|(i, c)| i == 8 || i == 13 || i == 18 || i == 23 || c.is_ascii_hexdigit())
}

/// Parse a raw query string into text query + qualifiers.
///
/// Qualifiers are `key:value` pairs. Supported keys:
/// `project`, `branch`, `model`, `role`, `skill`.
///
/// Everything that is not a qualifier becomes the text query.
pub(crate) fn parse_query_string(raw: &str) -> (String, Vec<Qualifier>) {
    let mut qualifiers = Vec::new();
    let mut text_parts = Vec::new();

    let known_keys = [
        "project", "branch", "model", "role", "skill", "session", "after", "before",
    ];

    // Tokenize respecting quoted strings
    let tokens = tokenize_query(raw);

    for token in tokens {
        if let Some(colon_pos) = token.find(':') {
            let key = &token[..colon_pos];
            let value = &token[colon_pos + 1..];
            if known_keys.contains(&key) && !value.is_empty() {
                qualifiers.push(Qualifier {
                    key: key.to_string(),
                    value: value.to_string(),
                });
                continue;
            }
        }
        text_parts.push(token);
    }

    let text_query = text_parts.join(" ");
    (text_query, qualifiers)
}

/// Tokenize a query string, preserving quoted phrases as single tokens.
pub(crate) fn tokenize_query(raw: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = raw.chars().peekable();
    let mut current = String::new();

    while let Some(&ch) = chars.peek() {
        match ch {
            '"' => {
                // Start of a quoted phrase — consume until closing quote
                chars.next(); // consume opening quote
                let mut phrase = String::from("\"");
                loop {
                    match chars.next() {
                        Some('"') => {
                            phrase.push('"');
                            break;
                        }
                        Some(c) => phrase.push(c),
                        None => {
                            // Unterminated quote — treat as regular text
                            phrase.push('"');
                            break;
                        }
                    }
                }
                // Flush any accumulated text before the quote
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                tokens.push(phrase);
            }
            ' ' | '\t' => {
                chars.next();
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => {
                chars.next();
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Tokenize free-text query terms in a way that mirrors Tantivy's default tokenizer.
///
/// Splits on non-alphanumeric characters, lowercases terms, and ignores empties.
/// If the entire query is wrapped in quotes, strips the wrapper first.
pub(crate) fn tokenize_text_terms(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    let unquoted = if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 2 {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    unquoted
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_plain_text() {
        let (text, quals) = parse_query_string("JWT authentication");
        assert_eq!(text, "JWT authentication");
        assert!(quals.is_empty());
    }

    #[test]
    fn test_parse_query_with_qualifiers() {
        let (text, quals) = parse_query_string("project:claude-view auth token");
        assert_eq!(text, "auth token");
        assert_eq!(quals.len(), 1);
        assert_eq!(quals[0].key, "project");
        assert_eq!(quals[0].value, "claude-view");
    }

    #[test]
    fn test_parse_query_multiple_qualifiers() {
        let (text, quals) = parse_query_string("project:myapp role:user fix bug");
        assert_eq!(text, "fix bug");
        assert_eq!(quals.len(), 2);
        assert_eq!(quals[0].key, "project");
        assert_eq!(quals[0].value, "myapp");
        assert_eq!(quals[1].key, "role");
        assert_eq!(quals[1].value, "user");
    }

    #[test]
    fn test_parse_query_quoted_phrase() {
        let (text, quals) = parse_query_string("\"JWT authentication\" project:myapp");
        assert_eq!(text, "\"JWT authentication\"");
        assert_eq!(quals.len(), 1);
        assert_eq!(quals[0].key, "project");
        assert_eq!(quals[0].value, "myapp");
    }

    #[test]
    fn test_parse_query_unknown_qualifier_treated_as_text() {
        let (text, quals) = parse_query_string("unknown:value search text");
        assert_eq!(text, "unknown:value search text");
        assert!(quals.is_empty());
    }

    #[test]
    fn test_parse_query_qualifier_empty_value() {
        // `project:` with no value should be treated as text, not a qualifier
        let (text, quals) = parse_query_string("project: search text");
        assert_eq!(text, "project: search text");
        assert!(quals.is_empty());
    }

    #[test]
    fn test_tokenize_preserves_quoted_strings() {
        let tokens = tokenize_query("hello \"world foo\" bar");
        assert_eq!(tokens, vec!["hello", "\"world foo\"", "bar"]);
    }

    #[test]
    fn test_tokenize_unterminated_quote() {
        let tokens = tokenize_query("hello \"world foo");
        assert_eq!(tokens, vec!["hello", "\"world foo\""]);
    }

    #[test]
    fn test_tokenize_text_terms_splits_hyphen() {
        let tokens = tokenize_text_terms("pm-status");
        assert_eq!(tokens, vec!["pm", "status"]);
    }

    #[test]
    fn test_is_session_id_valid_uuid() {
        assert!(is_session_id("136ed96f-913d-4a1a-91a9-5e651469b2a0"));
    }

    #[test]
    fn test_is_session_id_uppercase() {
        assert!(is_session_id("136ED96F-913D-4A1A-91A9-5E651469B2A0"));
    }

    #[test]
    fn test_is_session_id_with_whitespace() {
        assert!(is_session_id("  136ed96f-913d-4a1a-91a9-5e651469b2a0  "));
    }

    #[test]
    fn test_is_session_id_plain_text() {
        assert!(!is_session_id("JWT authentication"));
    }

    #[test]
    fn test_is_session_id_with_qualifier() {
        assert!(!is_session_id("project:claude-view auth"));
    }

    #[test]
    fn test_is_session_id_partial_uuid() {
        assert!(!is_session_id("136ed96f-913d"));
    }

    #[test]
    fn test_is_session_id_empty() {
        assert!(!is_session_id(""));
    }

    #[test]
    fn test_parse_query_session_id_detected() {
        // Verify that is_session_id is true for a UUID query
        assert!(is_session_id("136ed96f-913d-4a1a-91a9-5e651469b2a0"));
        // And that normal text is not
        let (text, quals) = parse_query_string("136ed96f-913d-4a1a-91a9-5e651469b2a0");
        assert_eq!(text, "136ed96f-913d-4a1a-91a9-5e651469b2a0");
        assert!(quals.is_empty());
    }

    #[test]
    fn test_parse_scope_multiple_qualifiers() {
        // This is the exact format the frontend sends as the scope parameter
        let (text, quals) = parse_query_string("project:claude-view branch:main");
        assert!(text.is_empty());
        assert_eq!(quals.len(), 2);
        assert_eq!(quals[0].key, "project");
        assert_eq!(quals[0].value, "claude-view");
        assert_eq!(quals[1].key, "branch");
        assert_eq!(quals[1].value, "main");
    }

    #[test]
    fn test_parse_scope_single_qualifier() {
        let (text, quals) = parse_query_string("project:claude-view");
        assert!(text.is_empty());
        assert_eq!(quals.len(), 1);
        assert_eq!(quals[0].key, "project");
        assert_eq!(quals[0].value, "claude-view");
    }

    #[test]
    fn test_parse_scope_with_all_qualifier_types() {
        let (text, quals) =
            parse_query_string("project:myapp branch:dev model:claude-opus-4-6 role:assistant");
        assert!(text.is_empty());
        assert_eq!(quals.len(), 4);
        assert_eq!(quals[0].key, "project");
        assert_eq!(quals[1].key, "branch");
        assert_eq!(quals[2].key, "model");
        assert_eq!(quals[3].key, "role");
    }
}
