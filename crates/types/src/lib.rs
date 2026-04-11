//! Pure type definitions shared across all claude-view crates.
//!
//! This crate contains ONLY types, enums, and constants.
//! No logic, no IO, no side effects.

pub mod block_types;
pub mod category;
pub mod error;
pub mod interact;
pub mod ownership;
pub mod types;

pub use block_types::*;
pub use category::*;
pub use error::*;
pub use interact::*;
pub use ownership::*;
pub use types::*;
