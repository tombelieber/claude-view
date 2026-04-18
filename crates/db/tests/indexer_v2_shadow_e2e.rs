//! End-to-end test for the indexer_v2 shadow pipeline.
//!
//! Drives `spawn_shadow_indexer_with_root` against a tempdir to verify
//! the full flow: write a JSONL file → fsnotify watcher fires →
//! debouncer coalesces → `index_session` runs → `session_stats` row
//! appears with the right shape.
//!
//! These are real integration tests (not `#[ignore]`'d) because they
//! exercise the actual `notify` crate and tokio runtime. They use
//! tempdirs so they're hermetic; nothing under `~/.claude-view/` or
//! `~/.claude/` is touched.
//!
//! ## Why a separate file
//!
//! These tests use `spawn_shadow_indexer_with_root` which expects a
//! real on-disk projects tree. Mixing them with the in-memory
//! per-module unit tests would couple test runtime to fsnotify
//! latency. Keeping them in their own integration test file lets
//! `cargo test --lib` stay fast (<1 s) while these run on the integration
//! schedule.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use claude_view_db::indexer_v2::spawn_shadow_indexer_with_root;
use claude_view_db::Database;

/// Smallest valid JSONL the parser will accept — one user line with a
/// timestamp. Mirrors the orchestrator's unit-test fixture format.
const ONE_USER_LINE: &str = r#"{"type":"user","timestamp":"2026-04-18T10:30:00Z","message":{"role":"user","content":"hello"}}
"#;

const TWO_USER_LINES: &str = concat!(
    r#"{"type":"user","timestamp":"2026-04-18T10:30:00Z","message":{"role":"user","content":"hello"}}"#,
    "\n",
    r#"{"type":"user","timestamp":"2026-04-18T10:31:00Z","message":{"role":"user","content":"second"}}"#,
    "\n"
);

/// Poll until the predicate returns Some, with a hard timeout. Panics
/// on timeout so a test failure makes the wait visible.
async fn await_until<T, F, Fut>(timeout: Duration, mut probe: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(v) = probe().await {
            return v;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("await_until timed out after {timeout:?}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Compose a depth-2 jsonl path under `root` (mirrors Claude Code's layout).
fn session_path(root: &PathBuf, project: &str, sid: &str) -> PathBuf {
    let project_dir = root.join(project);
    std::fs::create_dir_all(&project_dir).expect("create project dir");
    project_dir.join(format!("{sid}.jsonl"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shadow_indexer_picks_up_new_jsonl_file() {
    let db = Arc::new(
        Database::new_in_memory()
            .await
            .expect("in-memory DB for test"),
    );

    let root_dir = tempfile::tempdir().expect("tempdir");
    let root = root_dir.path().to_path_buf();

    // Pre-create the project subdirectory BEFORE starting the watcher.
    // notify on macOS (FSEvents) sometimes misses Modify events on
    // files inside subdirectories that were created after the watch
    // started — pre-creating the dir lets the kernel subscribe to its
    // inode along with the root.
    std::fs::create_dir_all(root.join("demo-project")).expect("project dir");

    let _handle = spawn_shadow_indexer_with_root(db.clone(), root.clone());

    // Generous warmup so the recursive subscription is hot before we
    // write. 500 ms is well above the FSEvents activation latency
    // observed on macOS dev machines.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let sid = "11111111-2222-3333-4444-555555555555";
    let path = session_path(&root, "demo-project", sid);
    std::fs::write(&path, ONE_USER_LINE).expect("write fixture jsonl");

    let header = await_until(Duration::from_secs(5), || {
        let db = db.clone();
        async move { db.get_stats_header(sid).await.expect("get_stats_header") }
    })
    .await;

    assert_eq!(header.session_id, sid);
    assert_eq!(header.source_size, ONE_USER_LINE.len() as i64);
    assert_eq!(
        header.source_content_hash.len(),
        32,
        "blake3_head_tail must be 32 bytes"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shadow_indexer_re_indexes_on_append() {
    let db = Arc::new(
        Database::new_in_memory()
            .await
            .expect("in-memory DB for test"),
    );

    let root_dir = tempfile::tempdir().expect("tempdir");
    let root = root_dir.path().to_path_buf();

    std::fs::create_dir_all(root.join("demo-project-2")).expect("project dir");
    let _handle = spawn_shadow_indexer_with_root(db.clone(), root.clone());
    tokio::time::sleep(Duration::from_millis(500)).await;

    let sid = "22222222-3333-4444-5555-666666666666";
    let path = session_path(&root, "demo-project-2", sid);
    std::fs::write(&path, ONE_USER_LINE).expect("write fixture jsonl");

    // Wait for the first index to land — line_count = 1.
    await_until(Duration::from_secs(5), || {
        let db = db.clone();
        let sid = sid.to_string();
        async move {
            let row: Option<(i64,)> =
                sqlx::query_as("SELECT line_count FROM session_stats WHERE session_id = ?")
                    .bind(&sid)
                    .fetch_optional(db.pool())
                    .await
                    .expect("query");
            match row {
                Some((1,)) => Some(()),
                _ => None,
            }
        }
    })
    .await;

    // Append a second line. The watcher should fire, the debouncer
    // should coalesce, and the next index_session must reflect the
    // larger file.
    std::fs::write(&path, TWO_USER_LINES).expect("append second line");

    await_until(Duration::from_secs(5), || {
        let db = db.clone();
        let sid = sid.to_string();
        async move {
            let row: Option<(i64, i64)> = sqlx::query_as(
                "SELECT line_count, source_size FROM session_stats WHERE session_id = ?",
            )
            .bind(&sid)
            .fetch_optional(db.pool())
            .await
            .expect("query");
            match row {
                Some((2, sz)) if sz == TWO_USER_LINES.len() as i64 => Some(()),
                _ => None,
            }
        }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shadow_indexer_ignores_subagent_files() {
    let db = Arc::new(
        Database::new_in_memory()
            .await
            .expect("in-memory DB for test"),
    );

    let root_dir = tempfile::tempdir().expect("tempdir");
    let root = root_dir.path().to_path_buf();

    // Pre-create the full subagent tree so the watcher's recursive
    // subscription is hot before we write — same reason as the
    // pick-up test.
    let project_dir = root.join("demo-project-3");
    let sid = "33333333-4444-5555-6666-777777777777";
    let subagent_dir = project_dir.join(sid).join("subagents");
    std::fs::create_dir_all(&subagent_dir).expect("create subagent dir");

    let _handle = spawn_shadow_indexer_with_root(db.clone(), root.clone());
    tokio::time::sleep(Duration::from_millis(500)).await;

    let subagent_path = subagent_dir.join("agent-foo.jsonl");
    std::fs::write(&subagent_path, ONE_USER_LINE).expect("write subagent jsonl");

    // Wait long enough that any indexing would have happened, then
    // assert no row was created. Use the debounce window + a grace
    // window so the assertion is not racy.
    tokio::time::sleep(Duration::from_millis(800)).await;
    let agent_id = subagent_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap()
        .to_string();
    let header = db
        .get_stats_header(&agent_id)
        .await
        .expect("get_stats_header");
    assert!(
        header.is_none(),
        "subagent file at depth 4 must NOT be indexed by the shadow indexer"
    );
}
