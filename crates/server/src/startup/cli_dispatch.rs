//! Early-exit dispatch for the `cleanup` subcommand.
//!
//! Extracted from `main.rs` in CQRS Phase 7.f. Runs before any async / DB
//! work so `claude-view cleanup` is a fast synchronous operation.

use crate::cli;

/// If the CLI command is `Cleanup`, run it and exit the process. Otherwise
/// return so `main` can continue startup.
pub fn handle_cleanup_if_requested(cli_parsed: &cli::Cli) {
    let Some(cli::Cmd::Cleanup) = &cli_parsed.command else {
        return;
    };

    eprintln!("\n\u{1f9f9} claude-view cleanup\n");
    let mut actions = Vec::new();

    // 1. Remove hooks from ~/.claude/settings.json (also removes .tmp)
    actions.extend(crate::live::hook_registrar::cleanup(0));
    crate::live::statusline_injector::cleanup();

    // 2. Remove cache directory (DB + Tantivy index)
    actions.extend(claude_view_core::paths::remove_cache_data());

    // 3. Remove lock files from /tmp
    actions.extend(claude_view_core::paths::remove_lock_files());

    // 4. Remove legacy debug/ directory (replaced by structured tracing)
    let legacy_debug = claude_view_core::paths::data_dir().join("debug");
    if legacy_debug.exists() {
        let _ = std::fs::remove_dir_all(&legacy_debug);
        actions.push("Removed legacy debug/ directory".to_string());
    }

    if actions.is_empty() {
        eprintln!("  Nothing to clean up.");
    } else {
        for action in &actions {
            eprintln!("  \u{2713} {action}");
        }
    }
    eprintln!();
    std::process::exit(0);
}
