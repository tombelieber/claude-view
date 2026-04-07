// crates/server/src/codegen_format.rs
//! Auto-format generated TypeScript types with Biome after ts-rs export.
//!
//! ts-rs outputs types with `;` separators, but our Biome config uses `,`
//! with trailing commas. Without this step, `git status` shows 140+ files
//! of pure formatting noise after every codegen run.
//!
//! Named with `zzz_export_bindings_` prefix so it:
//! 1. Matches the `export_bindings` test filter used by generate-types.sh
//! 2. Sorts alphabetically AFTER all real export_bindings tests

#[test]
fn zzz_export_bindings_format_generated() {
    let dirs = [
        "apps/web/src/types/generated/",
        "packages/shared/src/types/generated/",
    ];
    let status = std::process::Command::new("bunx")
        .args(["biome", "check", "--write", "--no-errors-on-unmatched"])
        .args(dirs)
        .status()
        .expect("failed to run bunx biome — is bun installed?");
    assert!(
        status.success(),
        "biome formatting of generated types failed"
    );
}
