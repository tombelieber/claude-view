//! CQRS Phase 4 proc-macro for `#[derive(RollupTable)]`.
//!
//! Expands the derive into the 15 typed rollup structs, migrations, and
//! I/O functions that Stage C + the read-side cutover endpoints consume.
//! Spec: `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §6.2`.
//!
//! Input shape (enforced by [`attrs::RollupConfig`]):
//! ```ignore
//! #[derive(RollupTable)]
//! #[rollup(buckets = [daily, weekly, monthly])]
//! #[rollup(dimensions = [
//!     global,
//!     project(project_id: TEXT),
//!     branch(project_id: TEXT, branch: TEXT),
//!     model(model_id: TEXT),
//!     category(category_l1: TEXT),
//! ])]
//! pub struct StatsCore {
//!     pub session_count: u64,
//!     // ... numeric stat fields only ...
//!     pub reedit_rate_sum: f64,
//! }
//! ```
//!
//! Output: `TABLE_COUNT` const = `buckets.len() * dimensions.len()`,
//! one `struct` per `(bucket, dim)` pair, one `CREATE TABLE` string in
//! `migrations::STATEMENTS`, and three `async fn`s per struct
//! (`insert_*`, `upsert_*`, `select_range_*`).
//!
//! See `crates/stats-rollup-derive/tests/ui/` for the happy-path and
//! compile-fail matrix.

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod attrs;
mod codegen;

#[proc_macro_derive(RollupTable, attributes(rollup))]
pub fn rollup_table_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let cfg = match attrs::RollupConfig::from_attrs(&input.attrs) {
        Ok(c) => c,
        Err(e) => return e.to_compile_error().into(),
    };

    let fields = match codegen::StatsField::all_from(&input) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error().into(),
    };

    if fields.is_empty() {
        return syn::Error::new(
            input.ident.span(),
            "#[derive(RollupTable)] requires at least one numeric field \
             (u64 or f64) on the target struct",
        )
        .to_compile_error()
        .into();
    }

    codegen::emit(&input, &cfg, &fields).into()
}
