//! CQRS Phase 1 proc-macro scaffold for `#[derive(RollupTable)]`.
//!
//! PR 1.4 behavior: parses the `DeriveInput` so bogus applications (e.g., applied
//! to an `enum` or `union`) fail with a clear compiler error, and emits a hidden
//! `__ROLLUP_TABLE_STUB` const on the target type. This const lets Phase 4 code
//! — and tests — check "was `RollupTable` derived here?" without inventing a
//! separate trait or runtime registry.
//!
//! Phase 4 replaces the emitted body with full codegen: 15 typed rollup table
//! structs + SQL migration strings + typed query functions. See design doc §6.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput};

#[proc_macro_derive(RollupTable, attributes(rollup))]
pub fn rollup_table_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Validate target shape early so Phase 4's codegen has a documented
    // contract to extend. `RollupTable` is only meaningful for structs;
    // enums/unions get a clear error now rather than a confusing codegen
    // error later.
    if !matches!(input.data, Data::Struct(_)) {
        return syn::Error::new_spanned(
            &input.ident,
            "#[derive(RollupTable)] is only supported on structs",
        )
        .to_compile_error()
        .into();
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #[doc(hidden)]
            pub const __ROLLUP_TABLE_STUB: () = ();
        }
    };

    expanded.into()
}
