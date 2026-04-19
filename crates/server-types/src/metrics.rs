//! Application metrics for Prometheus monitoring.
//!
//! This module provides:
//! - Prometheus metrics recorder initialization
//! - Metric definitions (counters, histograms, gauges)
//! - Helper functions for recording metrics
//! - `/metrics` endpoint handler

use claude_view_core::EffectiveRangeSource;
use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;
use std::time::Instant;

/// Global Prometheus handle for rendering metrics.
static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Initialize the Prometheus metrics recorder.
///
/// This should be called once at application startup, before any metrics are recorded.
/// Returns `true` if initialization succeeded, `false` if already initialized.
pub fn init_metrics() -> bool {
    if PROMETHEUS_HANDLE.get().is_some() {
        return false;
    }

    let recorder = PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();

    // Install the recorder globally
    if metrics::set_global_recorder(recorder).is_err() {
        tracing::warn!("Failed to set global metrics recorder (already set)");
        return false;
    }

    // Store the handle for later rendering
    if PROMETHEUS_HANDLE.set(handle).is_err() {
        tracing::warn!("Failed to store Prometheus handle (already set)");
    }

    // Describe all metrics
    describe_metrics();

    tracing::info!("Prometheus metrics initialized");
    true
}

/// Describe all application metrics for Prometheus.
fn describe_metrics() {
    // Request metrics
    describe_counter!(
        "dashboard_requests_total",
        "Total number of API requests to dashboard endpoints"
    );
    describe_histogram!(
        "dashboard_request_duration_seconds",
        "Duration of API requests in seconds"
    );
    describe_counter!(
        "time_range_resolution_total",
        "Total number of successful time range resolutions"
    );
    describe_counter!(
        "time_range_resolution_error_total",
        "Total number of time range resolution errors"
    );

    // Sync metrics
    describe_histogram!(
        "sync_duration_seconds",
        "Duration of sync operations in seconds"
    );
    describe_gauge!(
        "sync_sessions_processed",
        "Number of sessions processed in the last sync"
    );

    // Storage metrics
    describe_gauge!("storage_bytes", "Storage usage in bytes by type");

    // CQRS Phase 7 — shadow observability.
    describe_gauge!(
        "shadow_flags_diff_total",
        "Per-field drift count from the session_flags parity sweep. \
         Phase D.3 makes this a no-op but the gauge is kept so dashboards \
         stay schema-stable across the transition."
    );
    describe_gauge!(
        "flag_fold_lag_seq",
        "Pending session_action_log rows: max(seq) - fold_state.applied_seq."
    );
    describe_gauge!(
        "stage_c_outbox_pending_total",
        "Unapplied FlagDelta rows in stage_c_outbox (applied_at IS NULL)."
    );
}

/// Record the current CQRS shadow observability sample.
///
/// Called by the Phase 7 background sampler task every 60 s. The
/// gauge values reflect the most recent snapshot; the sampler task
/// overwrites them in-place so `/metrics` always serves fresh data
/// without synchronously hitting the DB on every scrape.
pub fn record_cqrs_shadow_sample(
    per_field_counts: &std::collections::BTreeMap<&'static str, u64>,
    fold_lag: i64,
    outbox_pending: i64,
) {
    for (field, count) in per_field_counts {
        gauge!("shadow_flags_diff_total", "field" => (*field).to_string()).set(*count as f64);
    }
    gauge!("flag_fold_lag_seq").set(fold_lag as f64);
    gauge!("stage_c_outbox_pending_total").set(outbox_pending as f64);
}

/// Render current metrics in Prometheus text format.
///
/// Returns `None` if metrics are not initialized.
pub fn render_metrics() -> Option<String> {
    PROMETHEUS_HANDLE.get().map(|h| h.render())
}

/// Record a completed API request.
///
/// # Arguments
/// * `endpoint` - The API endpoint name (e.g., "dashboard_stats", "sessions")
/// * `status` - HTTP status code as string (e.g., "200", "404", "500")
/// * `duration` - Request duration from start instant
pub fn record_request(endpoint: &str, status: &str, duration: std::time::Duration) {
    counter!("dashboard_requests_total", "endpoint" => endpoint.to_string(), "status" => status.to_string())
        .increment(1);
    histogram!("dashboard_request_duration_seconds", "endpoint" => endpoint.to_string())
        .record(duration.as_secs_f64());
}

/// Record a successful time range resolution.
pub fn record_time_range_resolution(endpoint: &str, source: EffectiveRangeSource) {
    counter!(
        "time_range_resolution_total",
        "endpoint" => endpoint.to_string(),
        "source" => source.as_str().to_string()
    )
    .increment(1);
}

/// Record a failed time range resolution.
pub fn record_time_range_resolution_error(endpoint: &str, reason: &str) {
    counter!(
        "time_range_resolution_error_total",
        "endpoint" => endpoint.to_string(),
        "reason" => reason.to_string()
    )
    .increment(1);
}

/// Record a completed sync operation.
///
/// # Arguments
/// * `sync_type` - The type of sync ("deep" or "git")
/// * `duration` - Sync duration
/// * `sessions_processed` - Number of sessions processed (if applicable)
pub fn record_sync(
    sync_type: &str,
    duration: std::time::Duration,
    sessions_processed: Option<u64>,
) {
    histogram!("sync_duration_seconds", "type" => sync_type.to_string())
        .record(duration.as_secs_f64());

    if let Some(count) = sessions_processed {
        gauge!("sync_sessions_processed").set(count as f64);
    }

    tracing::info!(
        sync_type = sync_type,
        duration_secs = duration.as_secs_f64(),
        sessions_processed = sessions_processed,
        "Sync operation completed"
    );
}

/// Record storage usage.
///
/// # Arguments
/// * `storage_type` - Type of storage ("jsonl", "sqlite", "index")
/// * `bytes` - Size in bytes
pub fn record_storage(storage_type: &str, bytes: u64) {
    gauge!("storage_bytes", "type" => storage_type.to_string()).set(bytes as f64);
}

/// Helper for timing request handlers.
///
/// Usage:
/// ```ignore
/// let timer = RequestTimer::new("dashboard_stats");
/// // ... do work ...
/// timer.finish_ok(); // or timer.finish_err(status_code)
/// ```
pub struct RequestTimer {
    endpoint: String,
    start: Instant,
}

impl RequestTimer {
    /// Create a new request timer for the given endpoint.
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            start: Instant::now(),
        }
    }

    /// Finish timing with a successful status.
    pub fn finish_ok(self) {
        record_request(&self.endpoint, "200", self.start.elapsed());
    }

    /// Finish timing with an error status.
    pub fn finish_err(self, status: u16) {
        record_request(&self.endpoint, &status.to_string(), self.start.elapsed());
    }

    /// Finish timing with a custom status string.
    pub fn finish(self, status: &str) {
        record_request(&self.endpoint, status, self.start.elapsed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_timer() {
        // Just test that RequestTimer doesn't panic
        let timer = RequestTimer::new("test_endpoint");
        std::thread::sleep(std::time::Duration::from_millis(1));
        timer.finish_ok();
    }

    #[test]
    fn test_render_metrics_before_init() {
        // Before init, render_metrics returns None (unless another test initialized it)
        // This is a weak test since test order isn't guaranteed
        let _ = render_metrics();
    }
}
