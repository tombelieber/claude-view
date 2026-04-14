use std::process::Command;

#[test]
fn panic_creates_crash_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let output = Command::new(env!("CARGO_BIN_EXE_panic_harness"))
        .env("CLAUDE_VIEW_PANIC_HARNESS_LOG_DIR", dir.path())
        .env("RUST_BACKTRACE", "1")
        .output()
        .expect("spawn harness");

    assert!(
        !output.status.success(),
        "harness should exit non-zero on panic"
    );

    let crashes: Vec<_> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("crash-"))
        .collect();
    assert_eq!(
        crashes.len(),
        1,
        "expected exactly 1 crash file, got: {crashes:?}"
    );

    let content = std::fs::read_to_string(crashes[0].path()).expect("read crash");
    assert!(
        content.starts_with("PANIC"),
        "crash file should start with PANIC"
    );
    assert!(content.contains("intentional test panic"));
    assert!(content.contains("panic-harness"));
}
