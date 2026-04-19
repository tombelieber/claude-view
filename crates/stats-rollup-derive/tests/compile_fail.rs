//! trybuild ui-test harness for `#[derive(RollupTable)]`.
//!
//! Phase 4 surface:
//!   - happy path — canonical StatsCore expands; TABLE_COUNT == 15.
//!   - compile_fail matrix — every foreseeable user error fails with a
//!     span-anchored compile error.
//!
//! To refresh the `.stderr` expectations after a legitimate error
//! message change: `TRYBUILD=overwrite cargo test -p
//! claude-view-stats-rollup-derive --test compile_fail` — verify the
//! regenerated `.stderr` files by hand before committing.

#[test]
fn trybuild_happy_path() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/happy_path.rs");
}

#[test]
fn trybuild_compile_fails() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/bad_bucket.rs");
    t.compile_fail("tests/ui/missing_buckets.rs");
    t.compile_fail("tests/ui/missing_dimensions.rs");
    t.compile_fail("tests/ui/unsupported_field_type.rs");
    t.compile_fail("tests/ui/non_struct.rs");
}
