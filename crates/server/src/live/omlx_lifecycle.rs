//! oMLX lifecycle manager: health check, readiness gate.
//!
//! Runs as a tokio task. Polls the local oMLX server's `/v1/models` endpoint
//! to verify the correct model is loaded. Sets `omlx_ready` AtomicBool
//! for the classify scheduler.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

const POLL_INTERVAL_STARTUP: Duration = Duration::from_secs(2);
const POLL_INTERVAL_RUNNING: Duration = Duration::from_secs(10);
const EXPECTED_MODEL_SUBSTRING: &str = "Qwen3.5-4B";

#[derive(Debug, PartialEq)]
enum OmlxState {
    Unknown,
    Running,
    Unavailable,
}

/// Run the oMLX lifecycle as a long-running tokio task.
/// Sets `omlx_ready` to true when the correct model is detected.
pub async fn run_lifecycle(omlx_ready: Arc<AtomicBool>, port: u16) {
    let base_url = format!("http://localhost:{}", port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .expect("reqwest client");

    let mut state = OmlxState::Unknown;
    info!(port, "oMLX lifecycle started, checking {}", base_url);

    loop {
        let interval = match state {
            OmlxState::Running => POLL_INTERVAL_RUNNING,
            _ => POLL_INTERVAL_STARTUP,
        };

        tokio::time::sleep(interval).await;

        match check_model(&client, &base_url).await {
            ModelCheck::CorrectModel(model_id) => {
                if state != OmlxState::Running {
                    info!(model_id, "oMLX ready with correct model");
                    state = OmlxState::Running;
                    omlx_ready.store(true, Ordering::Release);
                }
            }
            ModelCheck::WrongModel(model_id) => {
                if state == OmlxState::Running {
                    warn!(model_id, "oMLX model changed, no longer ready");
                }
                state = OmlxState::Unavailable;
                omlx_ready.store(false, Ordering::Release);
                debug!(
                    model_id,
                    "oMLX has wrong model, expected substring '{}'", EXPECTED_MODEL_SUBSTRING
                );
            }
            ModelCheck::NoModels => {
                if state == OmlxState::Running {
                    warn!("oMLX lost model, marking unavailable");
                }
                state = OmlxState::Unavailable;
                omlx_ready.store(false, Ordering::Release);
            }
            ModelCheck::Unreachable(err) => {
                if state == OmlxState::Running {
                    warn!(%err, "oMLX became unreachable");
                }
                if state != OmlxState::Unknown {
                    state = OmlxState::Unavailable;
                }
                omlx_ready.store(false, Ordering::Release);
                debug!(%err, "oMLX not reachable at {}", base_url);
            }
        }
    }
}

enum ModelCheck {
    CorrectModel(String),
    WrongModel(String),
    NoModels,
    Unreachable(String),
}

#[derive(serde::Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(serde::Deserialize)]
struct ModelEntry {
    id: String,
}

async fn check_model(client: &reqwest::Client, base_url: &str) -> ModelCheck {
    let url = format!("{}/v1/models", base_url);
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return ModelCheck::Unreachable(e.to_string()),
    };

    if !resp.status().is_success() {
        return ModelCheck::Unreachable(format!("HTTP {}", resp.status()));
    }

    let body: ModelsResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => return ModelCheck::Unreachable(format!("JSON parse: {}", e)),
    };

    if body.data.is_empty() {
        return ModelCheck::NoModels;
    }

    // Check if any loaded model matches our expected substring
    for model in &body.data {
        if model.id.contains(EXPECTED_MODEL_SUBSTRING) {
            return ModelCheck::CorrectModel(model.id.clone());
        }
    }

    // Models loaded but none match
    ModelCheck::WrongModel(body.data[0].id.clone())
}
