use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use grep_matcher::Matcher;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkMatch};

use crate::grep_types::{GrepLineMatch, GrepResponse, GrepSessionHit};

/// Options for a grep search.
pub struct GrepOptions {
    pub pattern: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub limit: usize,
}

/// Metadata for one JSONL file to search.
pub struct JsonlFile {
    pub path: PathBuf,
    pub session_id: String,
    pub project: String,
    pub project_path: String,
    pub modified_at: i64,
}

/// Search raw JSONL files for a regex pattern using ripgrep core crates.
pub fn grep_files(files: &[JsonlFile], opts: &GrepOptions) -> Result<GrepResponse, GrepError> {
    if files.is_empty() {
        return Ok(GrepResponse {
            pattern: opts.pattern.clone(),
            total_matches: 0,
            total_sessions: 0,
            elapsed_ms: 0.0,
            truncated: false,
            results: vec![],
        });
    }

    let start = std::time::Instant::now();

    // Validate regex upfront — fail fast on invalid patterns
    RegexMatcherBuilder::new()
        .case_insensitive(!opts.case_sensitive)
        .word(opts.whole_word)
        .build(&opts.pattern)
        .map_err(|e| GrepError::InvalidPattern(e.to_string()))?;

    let total_matches = AtomicUsize::new(0);
    let limit_reached = AtomicBool::new(false);
    let session_hits: std::sync::Mutex<Vec<GrepSessionHit>> = std::sync::Mutex::new(Vec::new());

    let parallelism = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let pattern = opts.pattern.clone();
    let case_sensitive = opts.case_sensitive;
    let whole_word = opts.whole_word;

    std::thread::scope(|scope| {
        let chunk_size = files.len().div_ceil(parallelism);
        let chunks: Vec<&[JsonlFile]> = files.chunks(chunk_size.max(1)).collect();

        for chunk in chunks {
            let pattern = pattern.clone();
            let total_matches = &total_matches;
            let limit_reached = &limit_reached;
            let session_hits = &session_hits;
            let limit = opts.limit;

            scope.spawn(move || {
                let matcher = RegexMatcherBuilder::new()
                    .case_insensitive(!case_sensitive)
                    .word(whole_word)
                    .build(&pattern)
                    .expect("pattern already validated above");

                for file in chunk {
                    if limit_reached.load(Ordering::Relaxed) {
                        break;
                    }

                    let mut line_matches: Vec<GrepLineMatch> = Vec::new();

                    let _ = SearcherBuilder::new()
                        .line_number(true)
                        .build()
                        .search_path(
                            &matcher,
                            &file.path,
                            MatchCollector {
                                matches: &mut line_matches,
                                matcher: &matcher,
                                limit,
                                total_matches,
                                limit_reached,
                            },
                        );

                    if !line_matches.is_empty() {
                        let hit = GrepSessionHit {
                            session_id: file.session_id.clone(),
                            project: file.project.clone(),
                            project_path: file.project_path.clone(),
                            modified_at: file.modified_at,
                            matches: line_matches,
                        };
                        session_hits.lock().unwrap().push(hit);
                    }
                }
            });
        }
    });

    let total = total_matches.load(Ordering::Relaxed);
    let truncated = limit_reached.load(Ordering::Relaxed);
    let mut results = session_hits.into_inner().unwrap();
    results.sort_by_key(|r| std::cmp::Reverse(r.modified_at));

    Ok(GrepResponse {
        pattern: opts.pattern.clone(),
        total_matches: total,
        total_sessions: results.len(),
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        truncated,
        results,
    })
}

/// Sink implementation that collects grep matches.
struct MatchCollector<'a, M: Matcher> {
    matches: &'a mut Vec<GrepLineMatch>,
    matcher: &'a M,
    limit: usize,
    total_matches: &'a AtomicUsize,
    limit_reached: &'a AtomicBool,
}

impl<'a, M: Matcher> Sink for MatchCollector<'a, M> {
    type Error = std::io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        if self.limit_reached.load(Ordering::Relaxed) {
            return Ok(false);
        }

        let line_content = String::from_utf8_lossy(mat.bytes());
        // UTF-8 safe truncation
        let content = if line_content.len() > 500 {
            let end = line_content
                .char_indices()
                .nth(500)
                .map(|(i, _)| i)
                .unwrap_or(line_content.len());
            format!("{}...", &line_content[..end])
        } else {
            line_content.trim_end().to_string()
        };

        // Find match positions within the line — convert byte offsets to char
        // offsets so the frontend (JS String.slice on UTF-16 code units) highlights
        // correctly for non-ASCII content.
        let mut match_start = 0;
        let mut match_end = 0;
        if let Ok(Some(m)) = self.matcher.find(mat.bytes()) {
            let byte_start = m.start().min(content.len());
            let byte_end = m.end().min(content.len());
            match_start = content[..byte_start].chars().count();
            match_end = content[..byte_end].chars().count();
        }

        self.matches.push(GrepLineMatch {
            line_number: mat.line_number().unwrap_or(0) as usize,
            content,
            match_start,
            match_end,
        });

        let prev = self.total_matches.fetch_add(1, Ordering::Relaxed);
        if prev + 1 >= self.limit {
            self.limit_reached.store(true, Ordering::Relaxed);
            return Ok(false);
        }

        Ok(true)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GrepError {
    #[error("Invalid regex pattern: {0}")]
    InvalidPattern(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_jsonl(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_grep_finds_pattern_in_jsonl() {
        let tmp = TempDir::new().unwrap();
        let file = create_test_jsonl(
            tmp.path(),
            "test.jsonl",
            "{\"type\":\"user\",\"message\":\"deploy to production\"}\n\
             {\"type\":\"assistant\",\"message\":\"running deploy script\"}\n\
             {\"type\":\"user\",\"message\":\"check the logs\"}\n",
        );

        let opts = GrepOptions {
            pattern: "deploy".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 100,
        };

        let files = vec![JsonlFile {
            path: file,
            session_id: "test-session".to_string(),
            project: "alpha".to_string(),
            project_path: tmp.path().to_string_lossy().to_string(),
            modified_at: 1000,
        }];

        let result = grep_files(&files, &opts).unwrap();
        assert_eq!(result.total_matches, 2);
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.results[0].matches.len(), 2);
    }

    #[test]
    fn test_grep_case_sensitive() {
        let tmp = TempDir::new().unwrap();
        let file = create_test_jsonl(
            tmp.path(),
            "test.jsonl",
            "{\"message\":\"Deploy\"}\n{\"message\":\"deploy\"}\n",
        );

        let opts = GrepOptions {
            pattern: "Deploy".to_string(),
            case_sensitive: true,
            whole_word: false,
            limit: 100,
        };

        let files = vec![JsonlFile {
            path: file,
            session_id: "s1".to_string(),
            project: "alpha".to_string(),
            project_path: tmp.path().to_string_lossy().to_string(),
            modified_at: 1000,
        }];

        let result = grep_files(&files, &opts).unwrap();
        assert_eq!(
            result.total_matches, 1,
            "case sensitive should match only 'Deploy'"
        );
    }

    #[test]
    fn test_grep_empty_files_list() {
        let opts = GrepOptions {
            pattern: "test".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 100,
        };

        let result = grep_files(&[], &opts).unwrap();
        assert_eq!(result.total_matches, 0);
        assert_eq!(result.total_sessions, 0);
        assert!(!result.truncated);
    }

    #[test]
    fn test_grep_invalid_pattern() {
        let opts = GrepOptions {
            pattern: "[invalid".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 100,
        };

        let tmp = TempDir::new().unwrap();
        let file = create_test_jsonl(tmp.path(), "test.jsonl", "some content\n");

        let files = vec![JsonlFile {
            path: file,
            session_id: "s1".to_string(),
            project: "alpha".to_string(),
            project_path: tmp.path().to_string_lossy().to_string(),
            modified_at: 1000,
        }];

        let result = grep_files(&files, &opts);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, GrepError::InvalidPattern(_)));
    }

    #[test]
    fn test_grep_respects_limit() {
        let tmp = TempDir::new().unwrap();
        let mut content = String::new();
        for i in 0..50 {
            content.push_str(&format!("{{\"line\":{},\"msg\":\"match here\"}}\n", i));
        }
        let file = create_test_jsonl(tmp.path(), "test.jsonl", &content);

        let opts = GrepOptions {
            pattern: "match".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 5,
        };

        let files = vec![JsonlFile {
            path: file,
            session_id: "s1".to_string(),
            project: "alpha".to_string(),
            project_path: tmp.path().to_string_lossy().to_string(),
            modified_at: 1000,
        }];

        let result = grep_files(&files, &opts).unwrap();
        assert!(result.truncated);
        assert!(result.total_matches <= 6); // May slightly exceed due to concurrent check
    }

    #[test]
    fn test_grep_multiple_sessions_sorted_by_modified() {
        let tmp = TempDir::new().unwrap();
        let file1 = create_test_jsonl(tmp.path(), "old.jsonl", "{\"msg\":\"deploy old\"}\n");
        let file2 = create_test_jsonl(tmp.path(), "new.jsonl", "{\"msg\":\"deploy new\"}\n");

        let opts = GrepOptions {
            pattern: "deploy".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 100,
        };

        let files = vec![
            JsonlFile {
                path: file1,
                session_id: "old-session".to_string(),
                project: "alpha".to_string(),
                project_path: tmp.path().to_string_lossy().to_string(),
                modified_at: 1000,
            },
            JsonlFile {
                path: file2,
                session_id: "new-session".to_string(),
                project: "alpha".to_string(),
                project_path: tmp.path().to_string_lossy().to_string(),
                modified_at: 2000,
            },
        ];

        let result = grep_files(&files, &opts).unwrap();
        assert_eq!(result.total_sessions, 2);
        // Should be sorted newest first
        assert_eq!(result.results[0].session_id, "new-session");
        assert_eq!(result.results[1].session_id, "old-session");
    }

    #[test]
    fn test_grep_no_matches() {
        let tmp = TempDir::new().unwrap();
        let file = create_test_jsonl(tmp.path(), "test.jsonl", "{\"msg\":\"hello world\"}\n");

        let opts = GrepOptions {
            pattern: "nonexistent_pattern_xyz".to_string(),
            case_sensitive: false,
            whole_word: false,
            limit: 100,
        };

        let files = vec![JsonlFile {
            path: file,
            session_id: "s1".to_string(),
            project: "alpha".to_string(),
            project_path: tmp.path().to_string_lossy().to_string(),
            modified_at: 1000,
        }];

        let result = grep_files(&files, &opts).unwrap();
        assert_eq!(result.total_matches, 0);
        assert_eq!(result.total_sessions, 0);
        assert!(result.results.is_empty());
    }
}
