//! CQRS Phase 1 proc-macro stub for `#[derive(RollupTable)]`.
//!
//! PR 1.1: emits nothing. Existence is enough to let downstream crates attach the
//! derive for forward-compatibility. PR 1.4 validates `#[rollup]` attributes; Phase 4
//! expands to full codegen for 15 rollup table types + SQL migrations +
//! typed query functions. See design doc §2.2 and §6.

use proc_macro::TokenStream;

#[proc_macro_derive(RollupTable, attributes(rollup))]
pub fn rollup_table_derive(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
