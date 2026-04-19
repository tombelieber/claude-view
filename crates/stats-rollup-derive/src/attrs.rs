//! Attribute parsing for `#[derive(RollupTable)]`.
//!
//! Parses two attribute forms attached to the target struct:
//!
//! ```ignore
//! #[rollup(buckets = [daily, weekly, monthly])]
//! #[rollup(dimensions = [
//!     global,
//!     project(project_id: TEXT),
//!     branch(project_id: TEXT, branch: TEXT),
//!     model(model_id: TEXT),
//!     category(category_l1: TEXT),
//! ])]
//! ```
//!
//! Each returns a typed `RollupConfig` that the codegen consumes. Parse
//! failures produce `syn::Error` with spans pointing at the offending
//! token so the compiler message lands at the user's call-site, not in
//! the macro expansion.
//!
//! See CQRS Phase 1-7 design §6.2 for the spec.

use proc_macro2::Span;
use syn::parse::{Parse, ParseStream};
use syn::{bracketed, parenthesized, punctuated::Punctuated, Attribute, Ident, Token};

/// One of the three allowed time buckets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    Daily,
    Weekly,
    Monthly,
}

impl Bucket {
    /// Singular form for struct-name composition (`Daily` + `Global` + `Stats`).
    pub fn camel(&self) -> &'static str {
        match self {
            Bucket::Daily => "Daily",
            Bucket::Weekly => "Weekly",
            Bucket::Monthly => "Monthly",
        }
    }

    /// snake_case form for SQL table names (`daily_global_stats`).
    pub fn snake(&self) -> &'static str {
        match self {
            Bucket::Daily => "daily",
            Bucket::Weekly => "weekly",
            Bucket::Monthly => "monthly",
        }
    }

    fn from_ident(i: &Ident) -> syn::Result<Self> {
        match i.to_string().as_str() {
            "daily" => Ok(Bucket::Daily),
            "weekly" => Ok(Bucket::Weekly),
            "monthly" => Ok(Bucket::Monthly),
            other => Err(syn::Error::new(
                i.span(),
                format!("unknown bucket `{other}`; expected one of: daily, weekly, monthly"),
            )),
        }
    }
}

/// One dimension's declaration, e.g. `branch(project_id: TEXT, branch: TEXT)`.
///
/// `global` parses as `DimSpec { name: "global", keys: [] }`.
#[derive(Debug, Clone)]
pub struct DimSpec {
    /// CamelCase form for struct-name composition (`Global`, `Project`).
    pub name_camel: String,
    /// snake_case form for SQL table names (`global`, `project`).
    pub name_snake: String,
    /// Ordered list of composite-key columns. Empty for `global`.
    pub keys: Vec<DimKey>,
    /// Span of the dimension identifier — kept for future error-message
    /// anchoring (e.g. "unknown dimension `X`" in later validation).
    #[allow(dead_code)]
    pub span: Span,
}

/// One key column inside a dimension, e.g. `project_id: TEXT`.
#[derive(Debug, Clone)]
pub struct DimKey {
    /// SQL column name (snake_case).
    pub column: String,
    /// Declared SQL type. Currently restricted to `TEXT` — widen if a
    /// non-TEXT key arrives (Phase 6+).
    pub sql_type: SqlType,
    /// Span of the key identifier for error messages.
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlType {
    Text,
}

impl SqlType {
    pub fn sql_keyword(&self) -> &'static str {
        match self {
            SqlType::Text => "TEXT",
        }
    }

    /// The Rust type used on the generated struct field. `TEXT` → `String`.
    pub fn rust_ident(&self) -> &'static str {
        match self {
            SqlType::Text => "String",
        }
    }

    fn from_ident(i: &Ident) -> syn::Result<Self> {
        match i.to_string().as_str() {
            "TEXT" => Ok(SqlType::Text),
            other => Err(syn::Error::new(
                i.span(),
                format!(
                    "unsupported SQL type `{other}`; only TEXT is allowed for dim keys \
                     today (widen in stats-rollup-derive::attrs if a non-TEXT key lands)"
                ),
            )),
        }
    }
}

/// Parsed `#[rollup(...)]` configuration collected across both attribute
/// forms on a single struct.
#[derive(Debug, Clone)]
pub struct RollupConfig {
    pub buckets: Vec<Bucket>,
    pub dimensions: Vec<DimSpec>,
}

impl RollupConfig {
    /// Parse all `#[rollup(...)]` attributes on the struct.
    ///
    /// Enforces exactly one `buckets = [...]` and one `dimensions = [...]`
    /// across the combined attribute set. Either order is accepted.
    pub fn from_attrs(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut buckets: Option<Vec<Bucket>> = None;
        let mut dimensions: Option<Vec<DimSpec>> = None;

        for attr in attrs {
            if !attr.path().is_ident("rollup") {
                continue;
            }
            // Each `#[rollup(...)]` body is a single `name = value` pair.
            let pair = attr.parse_args::<AttrPair>()?;
            match pair.key.to_string().as_str() {
                "buckets" => {
                    if buckets.is_some() {
                        return Err(syn::Error::new(
                            pair.key.span(),
                            "duplicate `buckets = [...]` attribute",
                        ));
                    }
                    buckets = Some(pair.parse_buckets()?);
                }
                "dimensions" => {
                    if dimensions.is_some() {
                        return Err(syn::Error::new(
                            pair.key.span(),
                            "duplicate `dimensions = [...]` attribute",
                        ));
                    }
                    dimensions = Some(pair.parse_dimensions()?);
                }
                other => {
                    return Err(syn::Error::new(
                        pair.key.span(),
                        format!(
                            "unknown rollup attribute key `{other}`; expected \
                             `buckets` or `dimensions`"
                        ),
                    ));
                }
            }
        }

        let buckets = buckets.ok_or_else(|| {
            syn::Error::new(
                Span::call_site(),
                "missing `#[rollup(buckets = [daily, weekly, monthly])]` attribute",
            )
        })?;
        let dimensions = dimensions.ok_or_else(|| {
            syn::Error::new(
                Span::call_site(),
                "missing `#[rollup(dimensions = [global, project(project_id: TEXT), ...])]` attribute",
            )
        })?;

        if buckets.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "`buckets = []` is empty; at least one bucket required",
            ));
        }
        if dimensions.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "`dimensions = []` is empty; at least one dimension required",
            ));
        }

        Ok(Self {
            buckets,
            dimensions,
        })
    }

    /// Total number of emitted tables = buckets × dimensions.
    pub fn table_count(&self) -> usize {
        self.buckets.len() * self.dimensions.len()
    }
}

/// Helper: parse a single `key = <bracketed-list>` pair.
struct AttrPair {
    key: Ident,
    bracketed_body: proc_macro2::TokenStream,
}

impl Parse for AttrPair {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        let _eq: Token![=] = input.parse()?;
        let content;
        bracketed!(content in input);
        let bracketed_body = content.parse()?;
        Ok(AttrPair {
            key,
            bracketed_body,
        })
    }
}

impl AttrPair {
    fn parse_buckets(self) -> syn::Result<Vec<Bucket>> {
        let list: Punctuated<Ident, Token![,]> =
            syn::parse::Parser::parse2(Punctuated::parse_terminated, self.bracketed_body)?;
        list.iter().map(Bucket::from_ident).collect()
    }

    fn parse_dimensions(self) -> syn::Result<Vec<DimSpec>> {
        let list: Punctuated<DimSpec, Token![,]> =
            syn::parse::Parser::parse2(Punctuated::parse_terminated, self.bracketed_body)?;
        Ok(list.into_iter().collect())
    }
}

impl Parse for DimSpec {
    /// Parses one of:
    /// - `global`
    /// - `project(project_id: TEXT)`
    /// - `branch(project_id: TEXT, branch: TEXT)`
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let span = name.span();
        let name_snake = name.to_string();
        let name_camel = snake_to_camel(&name_snake);

        let keys = if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            let list: Punctuated<DimKey, Token![,]> = Punctuated::parse_terminated(&content)?;
            list.into_iter().collect()
        } else {
            Vec::new()
        };

        Ok(DimSpec {
            name_camel,
            name_snake,
            keys,
            span,
        })
    }
}

impl Parse for DimKey {
    /// Parses `column: TYPE`, e.g. `project_id: TEXT`.
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let col_ident: Ident = input.parse()?;
        let span = col_ident.span();
        let _colon: Token![:] = input.parse()?;
        let ty_ident: Ident = input.parse()?;
        Ok(DimKey {
            column: col_ident.to_string(),
            sql_type: SqlType::from_ident(&ty_ident)?,
            span,
        })
    }
}

fn snake_to_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper_next = true;
    for ch in s.chars() {
        if ch == '_' {
            upper_next = true;
        } else if upper_next {
            for u in ch.to_uppercase() {
                out.push(u);
            }
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_camel() {
        assert_eq!(snake_to_camel("global"), "Global");
        assert_eq!(snake_to_camel("project_id"), "ProjectId");
        assert_eq!(snake_to_camel("category_l1"), "CategoryL1");
    }
}
