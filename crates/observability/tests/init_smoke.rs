use std::time::Duration;

#[test]
fn init_writes_jsonl_to_log_dir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log_dir = dir.path().to_path_buf();
    let cfg = claude_view_observability::ServiceConfig {
        service_name: "test-init",
        service_version: "0.0.1",
        build_sha: "test",
        log_dir: log_dir.clone(),
        default_filter: "info".to_string(),
        sink_mode: claude_view_observability::SinkMode::ProdOnly,
        deployment_mode: claude_view_observability::DeploymentMode::Dev,
        otel_endpoint: None,
        sentry_dsn: None,
    };

    let handle = claude_view_observability::init(cfg).expect("init succeeds");
    tracing::info!(operation = "smoke_test", "test.event.emitted");

    drop(handle);
    std::thread::sleep(Duration::from_millis(300));

    let entries: Vec<_> = std::fs::read_dir(&log_dir)
        .expect("read log dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
        .collect();
    assert!(
        !entries.is_empty(),
        "expected at least one .jsonl file in {log_dir:?}"
    );

    let content = std::fs::read_to_string(entries[0].path()).expect("read jsonl");
    let lines: Vec<&str> = content.lines().collect();
    assert!(
        lines.len() >= 2,
        "expected >= 2 lines (init + test), got {}",
        lines.len()
    );
    assert!(lines[0].contains("observability.init.complete"));
    assert!(lines[0].contains("\"test-init\""));
    assert!(lines[1].contains("test.event.emitted"));
    assert!(lines[1].contains("\"smoke_test\""));

    for line in &lines {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|e| panic!("line is not valid JSON: {e}\nline: {line}"));
    }

    let fname = entries[0]
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(
        fname.starts_with("test-init-"),
        "filename should start with service prefix, got: {fname}"
    );
}
