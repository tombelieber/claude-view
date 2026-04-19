//! Codegen — turns parsed `RollupConfig` + `StatsCore` fields into the
//! 15 typed structs, SQL migrations, and I/O functions that Stage C and
//! the read-side handlers consume.
//!
//! Each (dimension × bucket) pair produces:
//! - one `struct` definition (e.g. `DailyGlobalStats`)
//! - one `CREATE TABLE` string in `migrations::STATEMENTS`
//! - three `async fn`s: `insert_*`, `upsert_*`, `select_range_*`
//!
//! The emitted struct mirrors the user's `StatsCore` fields verbatim —
//! the proc-macro inspects `data: syn::Data` from `DeriveInput` to
//! extract them. Only `u64` / `f64` / `i64` numeric fields are supported
//! today; any other type fails the derive with a span-anchored error.
//!
//! SQLite STRICT type mapping:
//!   `u64`, `i64`   → INTEGER (values ≥ 0 for u64 by convention)
//!   `f64`          → REAL
//!   (TEXT handled separately via dim keys)
//!
//! See CQRS Phase 1-7 design §6.2 for the spec.

use crate::attrs::{Bucket, DimSpec, RollupConfig};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{spanned::Spanned, Data, DeriveInput, Field, Type};

/// One StatsCore field as the macro understands it.
pub struct StatsField {
    pub ident: Ident,
    /// Snake_case column / field name (just `ident.to_string()`; held
    /// separately for ergonomics in SQL emission).
    pub name: String,
    pub kind: FieldKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// u64 — emitted as INTEGER column, bound via `as i64`, re-read via
    /// `i64 as u64`.
    U64,
    /// f64 — emitted as REAL column, bound / read as `f64` directly.
    F64,
}

impl FieldKind {
    pub fn sql_type(&self) -> &'static str {
        match self {
            FieldKind::U64 => "INTEGER",
            FieldKind::F64 => "REAL",
        }
    }

    pub fn rust_type_tokens(&self) -> TokenStream {
        match self {
            FieldKind::U64 => quote!(u64),
            FieldKind::F64 => quote!(f64),
        }
    }
}

impl StatsField {
    fn from_syn(f: &Field) -> syn::Result<Self> {
        let ident = f.ident.clone().ok_or_else(|| {
            syn::Error::new(
                f.span(),
                "tuple structs are not supported by #[derive(RollupTable)]",
            )
        })?;
        let kind = match &f.ty {
            Type::Path(tp) if tp.path.is_ident("u64") => FieldKind::U64,
            Type::Path(tp) if tp.path.is_ident("f64") => FieldKind::F64,
            other => {
                return Err(syn::Error::new(
                    other.span(),
                    "only `u64` and `f64` field types are supported in \
                     #[derive(RollupTable)] structs today",
                ));
            }
        };
        let name = ident.to_string();
        Ok(Self { ident, name, kind })
    }

    pub fn all_from(input: &DeriveInput) -> syn::Result<Vec<Self>> {
        let data = match &input.data {
            Data::Struct(s) => s,
            _ => {
                return Err(syn::Error::new(
                    input.ident.span(),
                    "#[derive(RollupTable)] is only supported on structs",
                ))
            }
        };
        data.fields.iter().map(StatsField::from_syn).collect()
    }
}

/// Top-level entry — turns the parsed input into the macro's emitted
/// `TokenStream`.
pub fn emit(input: &DeriveInput, cfg: &RollupConfig, fields: &[StatsField]) -> TokenStream {
    // 1. Per-(dim × bucket) artifacts.
    let mut struct_defs = Vec::new();
    let mut sql_consts = Vec::new();
    let mut sql_const_idents = Vec::new();
    let mut fn_defs = Vec::new();

    for bucket in &cfg.buckets {
        for dim in &cfg.dimensions {
            struct_defs.push(emit_struct(bucket, dim, fields));
            let (sql_const_ident, sql_const) = emit_sql_const(bucket, dim, fields);
            sql_const_idents.push(sql_const_ident);
            sql_consts.push(sql_const);
            fn_defs.push(emit_fns(bucket, dim, fields));
        }
    }

    let table_count = cfg.table_count();
    let input_name = &input.ident;

    // 2. Migrations module — a single `STATEMENTS` slice containing every
    //    CREATE TABLE string in a stable order (bucket × dim).
    let migrations_mod = quote! {
        /// CQRS Phase 4 rollup table migrations — emitted by
        /// `#[derive(RollupTable)]` on `StatsCore`. See design §6.2.
        ///
        /// `STATEMENTS.len() == TABLE_COUNT`. Order is
        /// `(outer: bucket, inner: dimension)` and is the canonical apply
        /// order when spliced into `crates/db/src/migrations/rollups.rs`.
        pub mod migrations {
            #(#sql_consts)*

            /// All CREATE TABLE statements in canonical apply order.
            pub const STATEMENTS: &[&str] = &[ #(#sql_const_idents),* ];
        }
    };

    // 3. Input-struct stub preserved for backwards compat with PR 1.4 —
    //    lets tests and downstream code check "was RollupTable derived?"
    //    without inventing a separate trait.
    let stub_impl = quote! {
        impl #input_name {
            #[doc(hidden)]
            pub const __ROLLUP_TABLE_STUB: () = ();
        }
    };

    quote! {
        #stub_impl

        /// Number of tables generated by `#[derive(RollupTable)]` =
        /// `buckets.len() * dimensions.len()`.
        pub const TABLE_COUNT: usize = #table_count;

        #(#struct_defs)*

        #migrations_mod

        #(#fn_defs)*
    }
}

fn emit_struct(bucket: &Bucket, dim: &DimSpec, fields: &[StatsField]) -> TokenStream {
    let struct_ident = struct_ident(bucket, dim);
    let doc = format!(
        "Rollup row type for the `{}_{}_stats` table.\n\n\
         Generated by `#[derive(RollupTable)]`. Primary key: `(period_start{})`.",
        bucket.snake(),
        dim.name_snake,
        dim_key_pk_doc(dim),
    );

    let dim_key_fields: Vec<TokenStream> = dim
        .keys
        .iter()
        .map(|k| {
            let ident = format_ident!("{}", k.column, span = k.span);
            let ty = format_ident!("{}", k.sql_type.rust_ident(), span = k.span);
            let col_doc = format!(
                "Composite-key column `{}` ({})",
                k.column,
                k.sql_type.sql_keyword()
            );
            quote! {
                #[doc = #col_doc]
                pub #ident: #ty,
            }
        })
        .collect();

    let stat_fields: Vec<TokenStream> = fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let ty = f.kind.rust_type_tokens();
            quote! { pub #ident: #ty, }
        })
        .collect();

    quote! {
        #[doc = #doc]
        #[derive(Debug, Clone, PartialEq)]
        pub struct #struct_ident {
            /// Unix seconds at the bucket-boundary start (midnight UTC
            /// for daily, Monday 00:00 UTC for weekly, 1st 00:00 UTC for
            /// monthly). Stored as INTEGER; callers align before insert.
            pub period_start: i64,
            #(#dim_key_fields)*
            #(#stat_fields)*
        }
    }
}

fn emit_sql_const(bucket: &Bucket, dim: &DimSpec, fields: &[StatsField]) -> (Ident, TokenStream) {
    let const_ident = format_ident!(
        "CREATE_{}_{}_STATS",
        bucket.snake().to_uppercase(),
        dim.name_snake.to_uppercase()
    );

    let table = table_name(bucket, dim);

    // Column definitions.
    let mut cols: Vec<String> = Vec::with_capacity(1 + dim.keys.len() + fields.len());
    cols.push("    period_start INTEGER NOT NULL".to_string());
    for k in &dim.keys {
        cols.push(format!(
            "    {} {} NOT NULL",
            k.column,
            k.sql_type.sql_keyword()
        ));
    }
    for f in fields {
        let default = match f.kind {
            FieldKind::U64 => "0",
            FieldKind::F64 => "0.0",
        };
        cols.push(format!(
            "    {} {} NOT NULL DEFAULT {}",
            f.name,
            f.kind.sql_type(),
            default
        ));
    }

    // Primary key = (period_start, dim_keys...)
    let mut pk_cols = vec!["period_start".to_string()];
    for k in &dim.keys {
        pk_cols.push(k.column.clone());
    }
    cols.push(format!("    PRIMARY KEY ({})", pk_cols.join(", ")));

    let sql = format!(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n) STRICT;",
        table,
        cols.join(",\n"),
    );

    let doc = format!("CREATE TABLE statement for the `{}` rollup table.", table);

    let ts = quote! {
        #[doc = #doc]
        pub const #const_ident: &str = #sql;
    };

    (const_ident, ts)
}

fn emit_fns(bucket: &Bucket, dim: &DimSpec, fields: &[StatsField]) -> TokenStream {
    let struct_ident = struct_ident(bucket, dim);
    let table = table_name(bucket, dim);

    // Column lists used by multiple SQL statements.
    let mut insert_cols: Vec<String> = vec!["period_start".to_string()];
    insert_cols.extend(dim.keys.iter().map(|k| k.column.clone()));
    insert_cols.extend(fields.iter().map(|f| f.name.clone()));

    let placeholders: Vec<String> = (1..=insert_cols.len()).map(|i| format!("?{i}")).collect();

    // On-conflict target = PK columns. On-conflict update = sum stat
    // fields with `excluded.*`. Dim keys + period_start are PK components
    // and NEVER updated.
    let conflict_target: Vec<&str> = std::iter::once("period_start")
        .chain(dim.keys.iter().map(|k| k.column.as_str()))
        .collect();
    let update_clauses: Vec<String> = fields
        .iter()
        .map(|f| format!("{col} = {col} + excluded.{col}", col = f.name))
        .collect();

    let upsert_sql = format!(
        "INSERT INTO {table} ({cols}) VALUES ({placeholders}) \
         ON CONFLICT({pk}) DO UPDATE SET {updates}",
        table = table,
        cols = insert_cols.join(", "),
        placeholders = placeholders.join(", "),
        pk = conflict_target.join(", "),
        updates = update_clauses.join(", "),
    );

    let insert_sql = format!(
        "INSERT INTO {table} ({cols}) VALUES ({placeholders})",
        table = table,
        cols = insert_cols.join(", "),
        placeholders = placeholders.join(", "),
    );

    let select_cols = insert_cols.join(", ");
    let select_range_sql = format!(
        "SELECT {cols} FROM {table} WHERE period_start >= ?1 AND period_start < ?2 ORDER BY period_start ASC",
        cols = select_cols,
        table = table,
    );

    // Binding expressions for INSERT / UPSERT — period_start, dim keys,
    // then stat fields in the same order as `insert_cols`.
    let mut bind_exprs: Vec<TokenStream> = Vec::new();
    bind_exprs.push(quote! { .bind(row.period_start) });
    for k in &dim.keys {
        let ident = format_ident!("{}", k.column, span = k.span);
        bind_exprs.push(quote! { .bind(&row.#ident) });
    }
    for f in fields {
        let ident = &f.ident;
        match f.kind {
            FieldKind::U64 => bind_exprs.push(quote! { .bind(row.#ident as i64) }),
            FieldKind::F64 => bind_exprs.push(quote! { .bind(row.#ident) }),
        }
    }

    // Row-reading expressions for SELECT. Column index = insert_cols
    // position. Dim keys first (after period_start), then stats. sqlx
    // `try_get::<T, _>(idx)` is positional.
    let mut row_reads: Vec<TokenStream> = Vec::new();
    row_reads.push(quote! { period_start: row.try_get::<i64, _>(0usize)? });
    let mut idx: usize = 1;
    for k in &dim.keys {
        let ident = format_ident!("{}", k.column, span = k.span);
        let i = idx;
        row_reads.push(quote! { #ident: row.try_get::<String, _>(#i)? });
        idx += 1;
    }
    for f in fields {
        let ident = &f.ident;
        let i = idx;
        match f.kind {
            FieldKind::U64 => row_reads.push(quote! { #ident: row.try_get::<i64, _>(#i)? as u64 }),
            FieldKind::F64 => row_reads.push(quote! { #ident: row.try_get::<f64, _>(#i)? }),
        }
        idx += 1;
    }

    let insert_fn = format_ident!("insert_{}_{}_stats", bucket.snake(), dim.name_snake);
    let upsert_fn = format_ident!("upsert_{}_{}_stats", bucket.snake(), dim.name_snake);
    let select_range_fn = format_ident!("select_range_{}_{}_stats", bucket.snake(), dim.name_snake);

    let insert_doc = format!(
        "INSERT a new row into `{}`. Fails on primary-key conflict — use \
         `{}` for Stage-C-style merge behavior.",
        table, upsert_fn
    );
    let upsert_doc = format!(
        "UPSERT a row into `{}` with pointwise-sum semantics on stat \
         fields. The Stage C rollup writer's hot path.",
        table
    );
    let select_range_doc = format!(
        "SELECT all rows from `{}` with `period_start` in `[start, end)`. \
         Returns in ascending `period_start` order.",
        table
    );

    quote! {
        #[doc = #insert_doc]
        pub async fn #insert_fn(
            pool: &::sqlx::SqlitePool,
            row: &#struct_ident,
        ) -> ::std::result::Result<(), ::sqlx::Error> {
            ::sqlx::query(#insert_sql)
                #(#bind_exprs)*
                .execute(pool)
                .await?;
            Ok(())
        }

        #[doc = #upsert_doc]
        pub async fn #upsert_fn(
            pool: &::sqlx::SqlitePool,
            row: &#struct_ident,
        ) -> ::std::result::Result<(), ::sqlx::Error> {
            ::sqlx::query(#upsert_sql)
                #(#bind_exprs)*
                .execute(pool)
                .await?;
            Ok(())
        }

        #[doc = #select_range_doc]
        pub async fn #select_range_fn(
            pool: &::sqlx::SqlitePool,
            start: i64,
            end: i64,
        ) -> ::std::result::Result<::std::vec::Vec<#struct_ident>, ::sqlx::Error> {
            use ::sqlx::Row;
            let rows = ::sqlx::query(#select_range_sql)
                .bind(start)
                .bind(end)
                .fetch_all(pool)
                .await?;
            let mut out = ::std::vec::Vec::with_capacity(rows.len());
            for row in &rows {
                out.push(#struct_ident {
                    #(#row_reads,)*
                });
            }
            Ok(out)
        }
    }
}

fn struct_ident(bucket: &Bucket, dim: &DimSpec) -> Ident {
    Ident::new(
        &format!("{}{}Stats", bucket.camel(), dim.name_camel),
        Span::call_site(),
    )
}

fn table_name(bucket: &Bucket, dim: &DimSpec) -> String {
    format!("{}_{}_stats", bucket.snake(), dim.name_snake)
}

fn dim_key_pk_doc(dim: &DimSpec) -> String {
    if dim.keys.is_empty() {
        String::new()
    } else {
        let keys: Vec<&str> = dim.keys.iter().map(|k| k.column.as_str()).collect();
        format!(", {}", keys.join(", "))
    }
}
