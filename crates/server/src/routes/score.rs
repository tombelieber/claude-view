// crates/server/src/routes/score.rs
//! Fluency score API endpoint.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};

use crate::state::AppState;

/// GET /api/score - Get the current AI Fluency Score.
///
/// Returns a composite score (0-100) plus sub-metric breakdown
/// computed from session facets.
pub async fn get_fluency_score(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    match state.db.compute_current_fluency_score().await {
        Ok(score) => Json(serde_json::to_value(score).unwrap()),
        Err(e) => Json(serde_json::json!({"error": e.to_string(), "score": null})),
    }
}

/// Create the score routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/score", get(get_fluency_score))
}
