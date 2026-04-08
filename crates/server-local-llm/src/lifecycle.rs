use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use tracing::info;

use super::config::LocalLlmConfig;
use super::omlx_binary;
use super::provider::{self, Provider, PROBE_ORDER};
use super::status::{LlmStatus, ServerState};

const POLL_CONNECTED: Duration = Duration::from_secs(10);
const POLL_DISCONNECTED: Duration = Duration::from_secs(5);

/// Background task: probes for local LLM servers, updates shared status.
pub async fn run_lifecycle(status: Arc<LlmStatus>, config: Arc<LocalLlmConfig>) {
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .expect("reqwest client");

    info!("LLM lifecycle started (provider-agnostic)");

    loop {
        if !config.enabled() {
            if status.server_state() != ServerState::Unknown {
                info!("local LLM disabled");
                status.clear_connection();
                status.set_server_state(ServerState::Unknown);
            }
            tokio::time::sleep(POLL_DISCONNECTED).await;
            continue;
        }

        status.set_server_state(ServerState::Scanning);

        // Probe: custom URL or auto-detect by priority
        let result = if let Some(url) = config.url() {
            provider::probe_url(&client, &url)
                .await
                .map(|models| (Provider::Custom, url, models))
        } else {
            provider::probe_providers(&client, PROBE_ORDER).await
        };

        match result {
            Some((prov, url, models)) => {
                let preferred = config.preferred_model();
                let active = preferred
                    .filter(|m| models.contains(m))
                    .or_else(|| models.first().cloned());

                let was_connected = status.server_state() == ServerState::Connected;
                status.set_connection(prov, url.clone(), models, active.clone());

                if !was_connected {
                    info!(
                        provider = ?prov,
                        url = %url,
                        model = ?active,
                        "LLM connected"
                    );
                }
            }
            None => {
                if status.server_state() == ServerState::Connected {
                    info!("LLM disconnected");
                }
                status.clear_connection();
                status.set_server_state(ServerState::Disconnected);
            }
        }

        // oMLX UX guidance flags
        status.set_omlx_installed(omlx_binary::is_installed());
        status.set_omlx_running(omlx_binary::is_port_in_use(10710));

        let interval = match status.server_state() {
            ServerState::Connected => POLL_CONNECTED,
            _ => POLL_DISCONNECTED,
        };
        tokio::time::sleep(interval).await;
    }
}
