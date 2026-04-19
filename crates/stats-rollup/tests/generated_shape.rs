//! Structural assertions on the output of `#[derive(RollupTable)]`.
//!
//! These pin the shape the Stage C consumer + migrations + endpoint
//! cutovers rely on. A change here means a breaking change to the
//! rollup surface; update deliberately.

use claude_view_stats_rollup::stats_core::{self, StatsCore};

#[test]
fn table_count_is_fifteen() {
    // 3 buckets × 5 dimensions = 15 tables. If you change this, the
    // migrations in `crates/db/src/migrations/rollups.rs` must also
    // change by exactly the same amount (add or remove the matching
    // CREATE TABLE entries).
    assert_eq!(stats_core::TABLE_COUNT, 15);
}

#[test]
fn migrations_count_matches_tables() {
    assert_eq!(
        stats_core::migrations::STATEMENTS.len(),
        stats_core::TABLE_COUNT
    );
}

#[test]
fn migrations_cover_all_buckets_and_dimensions() {
    // Canonical sequence — bucket outer, dimension inner. If this ever
    // changes, migration numbering in `rollups.rs` breaks silently.
    let expected: &[&str] = &[
        "daily_global_stats",
        "daily_project_stats",
        "daily_branch_stats",
        "daily_model_stats",
        "daily_category_stats",
        "weekly_global_stats",
        "weekly_project_stats",
        "weekly_branch_stats",
        "weekly_model_stats",
        "weekly_category_stats",
        "monthly_global_stats",
        "monthly_project_stats",
        "monthly_branch_stats",
        "monthly_model_stats",
        "monthly_category_stats",
    ];
    for (i, expected_table) in expected.iter().enumerate() {
        let sql = stats_core::migrations::STATEMENTS[i];
        assert!(
            sql.contains(&format!("CREATE TABLE IF NOT EXISTS {expected_table}")),
            "migration[{i}] missing expected table `{expected_table}`; got:\n{sql}"
        );
    }
}

#[test]
fn every_sql_has_strict_mode_and_primary_key() {
    for (i, sql) in stats_core::migrations::STATEMENTS.iter().enumerate() {
        assert!(
            sql.contains("STRICT;"),
            "migration[{i}] missing STRICT mode: {sql}"
        );
        assert!(
            sql.contains("PRIMARY KEY"),
            "migration[{i}] missing PRIMARY KEY: {sql}"
        );
        assert!(
            sql.contains("period_start INTEGER NOT NULL"),
            "migration[{i}] missing period_start column: {sql}"
        );
    }
}

#[test]
fn dim_key_columns_present_per_dimension() {
    // For each generated table, the dim-specific key columns MUST be in
    // the SQL. Hand-list to catch silent renames.
    let expectations = [
        ("daily_project_stats", vec!["project_id TEXT NOT NULL"]),
        (
            "daily_branch_stats",
            vec!["project_id TEXT NOT NULL", "branch TEXT NOT NULL"],
        ),
        ("daily_model_stats", vec!["model_id TEXT NOT NULL"]),
        ("daily_category_stats", vec!["category_l1 TEXT NOT NULL"]),
        (
            "weekly_branch_stats",
            vec!["project_id TEXT NOT NULL", "branch TEXT NOT NULL"],
        ),
        ("monthly_category_stats", vec!["category_l1 TEXT NOT NULL"]),
    ];
    for (table, cols) in &expectations {
        let sql = stats_core::migrations::STATEMENTS
            .iter()
            .find(|s| s.contains(&format!("CREATE TABLE IF NOT EXISTS {table}")))
            .unwrap_or_else(|| panic!("no migration found for `{table}`"));
        for col in cols {
            assert!(
                sql.contains(col),
                "table `{table}` missing expected column `{col}`; SQL:\n{sql}"
            );
        }
    }
}

#[test]
fn stats_core_field_count_matches_declared_constant() {
    // Hand-maintained FIELD_COUNT has to track the struct; this test is
    // the guard that keeps them in sync. std::mem::size_of would also
    // work but is brittle across layout changes.
    let _marker = StatsCore::__ROLLUP_TABLE_STUB;
    // Count = 14 (10 u64 stat fields + 2 u64 averaging counts + 1 u64
    // lines_* + 2 extra duration pair + reedit pair) — really just
    // "number of numeric fields in StatsCore". See `stats_core.rs`.
    //
    // If this assertion fails, also update the associativity proptest.
    assert_eq!(StatsCore::FIELD_COUNT, 14);
}

#[test]
fn generated_struct_constructs() {
    // Smoke — the most commonly consumed generated type can be
    // constructed with default-ish values. Compile-time assertion that
    // field names + types haven't drifted.
    let row = stats_core::DailyGlobalStats {
        period_start: 1_700_000_000,
        session_count: 10,
        total_tokens: 1_000_000,
        total_cost_cents: 4200,
        prompt_count: 15,
        file_count: 3,
        lines_added: 120,
        lines_removed: 45,
        commit_count: 2,
        commit_insertions: 80,
        commit_deletions: 20,
        duration_sum_ms: 600_000,
        duration_count: 10,
        reedit_rate_sum: 0.15,
        reedit_rate_count: 10,
    };
    assert_eq!(row.session_count, 10);
    assert_eq!(row.reedit_rate_sum, 0.15);
}

#[test]
fn branch_dim_struct_has_both_keys() {
    let row = stats_core::DailyBranchStats {
        period_start: 1_700_000_000,
        project_id: "/Users/me/proj".into(),
        branch: "main".into(),
        session_count: 1,
        total_tokens: 0,
        total_cost_cents: 0,
        prompt_count: 0,
        file_count: 0,
        lines_added: 0,
        lines_removed: 0,
        commit_count: 0,
        commit_insertions: 0,
        commit_deletions: 0,
        duration_sum_ms: 0,
        duration_count: 0,
        reedit_rate_sum: 0.0,
        reedit_rate_count: 0,
    };
    assert_eq!(row.project_id, "/Users/me/proj");
    assert_eq!(row.branch, "main");
}
