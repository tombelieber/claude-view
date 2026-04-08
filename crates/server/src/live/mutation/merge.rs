//! Merge-strategy wrappers — re-exported from state::merge_wrappers.
//!
//! The canonical definitions now live in `crate::live::state::merge_wrappers`
//! so the state crate is self-contained. This module re-exports for backward
//! compatibility with existing `use crate::live::mutation::merge::*` paths.

pub use crate::live::state::merge_wrappers::{Latest, Monotonic, Transient};
