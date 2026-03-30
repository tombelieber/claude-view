pub mod client;
mod config;
mod lifecycle;
mod model_manager;
mod routes;
mod service;
mod status;

pub use config::LocalLlmConfig;
pub use lifecycle::{run_lifecycle, ProcessMode, EXPECTED_MODEL_SUBSTRING};
pub use model_manager::{DownloadProgress, ModelManager};
pub use routes::local_llm_routes;
pub use service::{LocalLlmService, ServiceStatus};
pub use status::{LlmStatus, ServerState, StatusSnapshot};
