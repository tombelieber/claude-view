use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, info, warn};

use super::config::LocalLlmConfig;
use super::process::ManagedProcess;
use super::registry;
use super::status::{LlmStatus, ServerState};

const POLL_INTERVAL_STARTUP: Duration = Duration::from_secs(2);
const POLL_INTERVAL_RUNNING: Duration = Duration::from_secs(10);

/// How the LLM process is managed.
#[derive(Debug)]
pub enum ProcessMode {
    /// External process — user runs it. We discover via port.
    Discover { port: u16 },
    /// claude-view owns the oMLX child process.
    Managed {
        port: u16,
        process: Arc<Mutex<Option<ManagedProcess>>>,
    },
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

/// Background task: polls the LLM server health, updates shared status.
pub async fn run_lifecycle(status: Arc<LlmStatus>, config: Arc<LocalLlmConfig>, mode: ProcessMode) {
    match mode {
        ProcessMode::Discover { port: _ } => {
            run_discover_loop(status, config).await;
        }
        ProcessMode::Managed { port: _, process } => {
            run_managed_loop(status, config, process).await;
        }
    }
}

async fn run_discover_loop(status: Arc<LlmStatus>, config: Arc<LocalLlmConfig>) {
    let base_url = format!("http://localhost:{}", status.port);
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .expect("reqwest client");

    let mut state = ServerState::Unknown;
    info!(
        port = status.port,
        "LLM lifecycle started, checking {}", base_url
    );

    loop {
        // Respect feature toggle — exit if disabled
        if !config.enabled() {
            if state == ServerState::Running {
                info!("LLM config disabled, shutting down lifecycle");
                status.ready.store(false, Ordering::Release);
                status.set_pid(None);
                status.set_discovered_model_id(None);
                status.set_server_state(ServerState::Unknown);
            }
            tokio::time::sleep(POLL_INTERVAL_STARTUP).await;
            state = ServerState::Unknown;
            continue;
        }

        // Self-healing: if client cleared ready flag, re-probe aggressively
        let was_demoted = state == ServerState::Running && !status.ready.load(Ordering::Acquire);

        let interval = match state {
            ServerState::Running if !was_demoted => POLL_INTERVAL_RUNNING,
            _ => POLL_INTERVAL_STARTUP,
        };

        tokio::time::sleep(interval).await;

        // Resolve active model substring for health check
        let active = config
            .active_model()
            .and_then(|id| registry::find_model(&id))
            .unwrap_or_else(registry::default_model);

        // Probe /v1/models
        let model_id = match check_model(&client, &base_url, active.model_id_substring).await {
            Some(id) => id,
            None => {
                if state == ServerState::Running {
                    warn!("LLM model check failed, marking unavailable");
                }
                status.ready.store(false, Ordering::Release);
                status.set_pid(None);
                status.set_discovered_model_id(None);
                status.set_server_state(ServerState::Unavailable);
                state = ServerState::Unavailable;
                continue;
            }
        };

        // Store the runtime model ID discovered from oMLX — this is the
        // single source of truth for what model name to use in requests.
        status.set_discovered_model_id(Some(model_id.clone()));

        // Verify inference on first transition to Running (or re-probe after demotion)
        if state != ServerState::Running || was_demoted {
            if !verify_inference(&client, &base_url, &model_id).await {
                debug!("LLM inference verify failed (model loading?)");
                status.ready.store(false, Ordering::Release);
                status.set_discovered_model_id(None);
                status.set_server_state(ServerState::Unavailable);
                state = ServerState::Unavailable;
                continue;
            }

            let pid = find_llm_pid(status.port);
            status.set_pid(pid);
            info!(model = %model_id, ?pid, "LLM ready");
        }

        status.ready.store(true, Ordering::Release);
        status.set_server_state(ServerState::Running);
        state = ServerState::Running;
    }
}

const MAX_RESTART_ATTEMPTS: u32 = 3;

async fn run_managed_loop(
    status: Arc<LlmStatus>,
    config: Arc<LocalLlmConfig>,
    process: Arc<Mutex<Option<ManagedProcess>>>,
) {
    let base_url = format!("http://localhost:{}", status.port);
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .expect("reqwest client");

    let mut state = ServerState::Unknown;
    let mut restart_count = 0u32;

    info!(port = status.port, "LLM managed lifecycle started");

    loop {
        if !config.enabled() {
            if state == ServerState::Running {
                info!("LLM config disabled");
                status.ready.store(false, Ordering::Release);
                status.set_pid(None);
                status.set_discovered_model_id(None);
                status.set_server_state(ServerState::Unknown);
            }
            // Shutdown managed process if running
            if let Some(mut proc) = process.lock().unwrap().take() {
                tokio::spawn(async move { proc.shutdown().await });
            }
            tokio::time::sleep(POLL_INTERVAL_STARTUP).await;
            state = ServerState::Unknown;
            restart_count = 0;
            continue;
        }

        // Check if child process is still alive
        let process_died = {
            let mut guard = process.lock().unwrap();
            if let Some(ref mut proc) = *guard {
                if !proc.is_alive() {
                    warn!("oMLX process exited unexpectedly");
                    *guard = None;
                    status.ready.store(false, Ordering::Release);
                    status.set_pid(None);
                    status.set_discovered_model_id(None);
                    status.set_server_state(ServerState::Unavailable);
                    state = ServerState::Unavailable;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }; // guard dropped here

        if process_died {
            // Auto-restart up to MAX_RESTART_ATTEMPTS
            if restart_count < MAX_RESTART_ATTEMPTS {
                restart_count += 1;
                warn!(attempt = restart_count, "will attempt restart");
            } else {
                warn!("max restart attempts reached, giving up");
                tokio::time::sleep(POLL_INTERVAL_RUNNING).await;
                continue;
            }
        }

        // Standard health check (same as discover loop)
        let was_demoted = state == ServerState::Running && !status.ready.load(Ordering::Acquire);
        let interval = match state {
            ServerState::Running if !was_demoted => POLL_INTERVAL_RUNNING,
            _ => POLL_INTERVAL_STARTUP,
        };
        tokio::time::sleep(interval).await;

        let active = config
            .active_model()
            .and_then(|id| registry::find_model(&id))
            .unwrap_or_else(registry::default_model);

        let model_id = match check_model(&client, &base_url, active.model_id_substring).await {
            Some(id) => id,
            None => {
                if state == ServerState::Running {
                    warn!("LLM model check failed, marking unavailable");
                }
                status.ready.store(false, Ordering::Release);
                status.set_pid(None);
                status.set_discovered_model_id(None);
                status.set_server_state(ServerState::Unavailable);
                state = ServerState::Unavailable;
                continue;
            }
        };

        status.set_discovered_model_id(Some(model_id.clone()));

        if state != ServerState::Running || was_demoted {
            if !verify_inference(&client, &base_url, &model_id).await {
                debug!("LLM inference verify failed (model loading?)");
                status.ready.store(false, Ordering::Release);
                status.set_discovered_model_id(None);
                status.set_server_state(ServerState::Unavailable);
                state = ServerState::Unavailable;
                continue;
            }

            let pid = find_llm_pid(status.port);
            status.set_pid(pid);
            info!(model = %model_id, ?pid, "LLM ready (managed)");
            restart_count = 0; // Reset on successful startup
        }

        status.ready.store(true, Ordering::Release);
        status.set_server_state(ServerState::Running);
        state = ServerState::Running;
    }
}

async fn check_model(client: &Client, base_url: &str, model_substring: &str) -> Option<String> {
    let resp = client
        .get(format!("{base_url}/v1/models"))
        .send()
        .await
        .ok()?;
    let models: ModelsResponse = resp.json().await.ok()?;
    models
        .data
        .iter()
        .find(|m| m.id.contains(model_substring))
        .map(|m| m.id.clone())
}

async fn verify_inference(client: &Client, base_url: &str, model: &str) -> bool {
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1,
        "temperature": 0.0,
        "chat_template_kwargs": {"enable_thinking": false}
    });
    client
        .post(format!("{base_url}/v1/chat/completions"))
        .json(&body)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

fn find_llm_pid(port: u16) -> Option<u32> {
    let output = std::process::Command::new("lsof")
        .args(["-ti", &format!(":{port}"), "-sTCP:LISTEN"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.split_whitespace().next()?.parse::<u32>().ok()
}
