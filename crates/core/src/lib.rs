// crates/core/src/lib.rs
pub mod error;
pub mod types;
pub mod parser;
pub mod discovery;
pub mod session_index;
pub mod registry;
pub mod invocation;
pub mod metrics;
pub mod llm;
pub mod cli;
pub mod classification;
pub mod patterns;
pub mod insights;

pub use error::*;
pub use types::*;
pub use parser::*;
pub use discovery::*;
pub use session_index::*;
pub use registry::*;
pub use invocation::*;
pub use metrics::*;
pub use cli::ClaudeCliStatus;
