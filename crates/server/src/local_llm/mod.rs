pub mod client;
mod config;
mod download;
mod inventory;
mod lifecycle;
mod model_manager;
pub mod registry;
mod routes;
mod service;
mod status;

pub use config::LocalLlmConfig;
pub use download::DownloadProgress;
pub use lifecycle::{run_lifecycle, ProcessMode};
pub use model_manager::ModelManager;
pub use registry::{ModelEntry, REGISTRY};
pub use routes::local_llm_routes;
pub use service::{LocalLlmService, ServiceStatus};
pub use status::{LlmStatus, ServerState, StatusSnapshot};
