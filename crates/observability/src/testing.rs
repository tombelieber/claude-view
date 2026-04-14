pub fn tempdir_logs() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().to_path_buf();
    (dir, path)
}

pub fn read_all_jsonl(dir: &std::path::Path) -> Vec<String> {
    let mut lines = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl") {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    lines.extend(content.lines().map(|l| l.to_string()));
                }
            }
        }
    }
    lines
}
