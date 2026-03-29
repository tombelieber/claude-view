//! oMLX lifecycle manager: health check, readiness gate.
//!
//! Runs as a tokio task. Polls the local oMLX server's `/v1/models` endpoint
//! to verify the correct model is loaded. Sets `omlx_ready` AtomicBool
//! for the classify scheduler.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

const POLL_INTERVAL_STARTUP: Duration = Duration::from_secs(2);
const POLL_INTERVAL_RUNNING: Duration = Duration::from_secs(10);
pub const EXPECTED_MODEL_SUBSTRING: &str = "Qwen3.5-4B";

/// Shared status of the oMLX service, readable by the component collector.
/// `ready` is `Arc<AtomicBool>` so the scheduler can share just the flag.
pub struct OmlxStatus {
    pub ready: Arc<AtomicBool>,
    pub port: u16,
    omlx_pid: Arc<AtomicU32>,
}

impl OmlxStatus {
    pub fn new(port: u16) -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
            port,
            omlx_pid: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Returns the cached oMLX PID, or `None` if not yet resolved / not running.
    pub fn pid(&self) -> Option<u32> {
        match self.omlx_pid.load(Ordering::Acquire) {
            0 => None,
            pid => Some(pid),
        }
    }

    /// Cache the oMLX PID. Pass `None` to clear (stores 0).
    pub fn set_pid(&self, pid: Option<u32>) {
        self.omlx_pid.store(pid.unwrap_or(0), Ordering::Release);
    }
}

#[derive(Debug, PartialEq)]
enum OmlxState {
    Unknown,
    Running,
    Unavailable,
}

/// Run the oMLX lifecycle as a long-running tokio task.
/// Sets `status.ready` to true when the correct model is detected.
pub async fn run_lifecycle(status: Arc<OmlxStatus>) {
    let base_url = format!("http://localhost:{}", status.port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .expect("reqwest client");

    let mut state = OmlxState::Unknown;
    info!(
        port = status.port,
        "oMLX lifecycle started, checking {}", base_url
    );

    loop {
        // If the client signalled errors (cleared omlx_ready), demote to fast poll
        if state == OmlxState::Running && !status.ready.load(Ordering::Acquire) {
            warn!("oMLX readiness cleared by client errors, re-probing");
            state = OmlxState::Unavailable;
            status.set_pid(None);
        }

        let interval = match state {
            OmlxState::Running => POLL_INTERVAL_RUNNING,
            _ => POLL_INTERVAL_STARTUP,
        };

        tokio::time::sleep(interval).await;

        match check_model(&client, &base_url).await {
            ModelCheck::CorrectModel(model_id) => {
                if state != OmlxState::Running {
                    info!(model_id, "oMLX ready with correct model (inference verified)");
                    state = OmlxState::Running;
                    status.ready.store(true, Ordering::Release);

                    // PID resolution (only on state transitions)
                    let pid = find_omlx_pid(status.port);
                    status.set_pid(pid);
                    if let Some(p) = pid {
                        info!("oMLX PID resolved: {p}");
                    }
                }
            }
            ModelCheck::WrongModel(model_id) => {
                if state == OmlxState::Running {
                    warn!(model_id, "oMLX model changed, no longer ready");
                }
                state = OmlxState::Unavailable;
                status.ready.store(false, Ordering::Release);
                status.set_pid(None);
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
                status.ready.store(false, Ordering::Release);
                status.set_pid(None);
            }
            ModelCheck::Unreachable(err) => {
                if state == OmlxState::Running {
                    warn!(%err, "oMLX became unreachable");
                }
                if state != OmlxState::Unknown {
                    state = OmlxState::Unavailable;
                }
                status.ready.store(false, Ordering::Release);
                status.set_pid(None);
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
    let matched = body.data.iter().find(|m| m.id.contains(EXPECTED_MODEL_SUBSTRING));
    let Some(model) = matched else {
        return ModelCheck::WrongModel(body.data[0].id.clone());
    };
    let model_id = model.id.clone();

    // Probe: send a tiny inference call to verify weights are loaded.
    // oMLX lists the model before weights finish loading → /v1/models
    // returns 200 but inference returns 500. This probe catches that.
    if !probe_inference(client, base_url, &model_id).await {
        return ModelCheck::Unreachable("model listed but inference 500 (weights loading)".into());
    }

    ModelCheck::CorrectModel(model_id)
}

/// Tiny inference probe — 1 token, verifies model weights are loaded.
async fn probe_inference(client: &reqwest::Client, base_url: &str, model: &str) -> bool {
    let url = format!("{}/v1/chat/completions", base_url);
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1,
        "temperature": 0.0,
    });
    match client.post(&url).json(&body).send().await {
        Ok(r) => r.status().is_success(),
        Err(_) => false,
    }
}

/// One-time PID resolution for oMLX. Only called at startup/rediscovery.
fn find_omlx_pid(port: u16) -> Option<u32> {
    let output = std::process::Command::new("lsof")
        .args(["-ti", &format!(":{port}"), "-sTCP:LISTEN"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.split_whitespace().next()?.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omlx_status_pid_default_zero() {
        let status = OmlxStatus::new(8080);
        assert_eq!(status.pid(), None, "no PID before lifecycle discovers it");
    }

    #[test]
    fn omlx_status_set_and_get_pid() {
        let status = OmlxStatus::new(8080);
        status.set_pid(Some(12345));
        assert_eq!(status.pid(), Some(12345));
    }

    #[test]
    fn omlx_status_clear_pid() {
        let status = OmlxStatus::new(8080);
        status.set_pid(Some(12345));
        status.set_pid(None);
        assert_eq!(status.pid(), None);
    }
}
