fn main() {
    let log_dir = std::path::PathBuf::from(
        std::env::var("CLAUDE_VIEW_PANIC_HARNESS_LOG_DIR").expect("LOG_DIR env var"),
    );
    std::fs::create_dir_all(&log_dir).ok();
    claude_view_observability::panic_hook::install(log_dir, "panic-harness");
    panic!("intentional test panic");
}
