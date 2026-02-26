pub mod auth;
pub mod pairing;
pub mod state;
pub mod ws;

use axum::{
    routing::{get, post},
    Router,
};
use state::RelayState;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tracing::debug;

pub fn app(state: RelayState) -> Router {
    // Spawn background cleanup for expired pairing offers
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            cleanup_state
                .pairing_offers
                .retain(|_, offer| offer.created_at.elapsed().as_secs() < 300);
            debug!("cleaned expired pairing offers");
        }
    });

    // Phone at :5173 POSTs to relay at :47893 — cross-origin
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ws", get(ws::ws_handler))
        .route("/pair", post(pairing::create_pair))
        .route("/pair/claim", post(pairing::claim_pair))
        .layer(cors)
        .with_state(state)
}
