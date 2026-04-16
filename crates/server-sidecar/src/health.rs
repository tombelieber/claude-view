//! Health check and session recovery operations.

use std::time::Duration;

use super::error::SidecarError;
use super::manager::SidecarManager;

impl SidecarManager {
    /// HTTP health check over TCP using reqwest.
    pub(crate) async fn health_check(&self) -> Result<(), SidecarError> {
        let url = format!("{}/health", self.base_url);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| SidecarError::RequestError(format!("Build HTTP client: {e}")))?;

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| SidecarError::RequestError(format!("Health check request: {e}")))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(SidecarError::RequestError(format!(
                "Health check returned {}",
                response.status()
            )))
        }
    }

    /// Re-resume a batch of previously controlled sessions after a restart.
    ///
    /// Called ONCE at server boot from `promote_from_snapshot`. NEVER called
    /// from the reconciliation loop — autonomous recovery creates empty SDK
    /// sessions when the sidecar repeatedly crashes (see #54). Runtime
    /// recovery is lazy, per-session, on user interaction via
    /// `LiveSessionManager::ensure_session_control_alive`.
    pub async fn recover_controlled_sessions(
        &self,
        session_ids: &[(String, String)], // (session_id, old_control_id)
    ) -> Vec<(String, String)> {
        let mut recovered = Vec::new();
        for (session_id, _old_control_id) in session_ids {
            match self.resume_session(session_id).await {
                Ok(new_control_id) => {
                    tracing::info!(
                        session_id = %session_id,
                        new_control_id = %new_control_id,
                        "Recovered controlled session after sidecar restart"
                    );
                    recovered.push((session_id.clone(), new_control_id));
                }
                Err(e) => {
                    tracing::warn!(
                        session_id = %session_id,
                        error = %e,
                        "Failed to recover controlled session"
                    );
                }
            }
        }
        recovered
    }

    /// Call sidecar POST /api/sidecar/sessions/:id/resume for a single session.
    ///
    /// Returns the new `control_id`. Public so the app-layer lazy-recovery
    /// helper (`LiveSessionManager::ensure_session_control_alive`) can call it
    /// for a single session on user demand rather than bulk-recovering all.
    pub async fn resume_session(&self, session_id: &str) -> Result<String, SidecarError> {
        let url = format!(
            "{}/api/sidecar/sessions/{}/resume",
            self.base_url, session_id
        );
        let body = serde_json::json!({
            "model": "claude-sonnet-4-20250514",
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| SidecarError::RequestError(format!("Resume request: {e}")))?;

        if !resp.status().is_success() {
            return Err(SidecarError::RequestError(format!(
                "Resume returned {}",
                resp.status()
            )));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SidecarError::RequestError(format!("Parse JSON: {e}")))?;

        data["controlId"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SidecarError::RequestError("No controlId in response".into()))
    }
}
