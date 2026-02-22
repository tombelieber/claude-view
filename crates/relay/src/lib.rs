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

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ws", get(ws::ws_handler))
        .route("/pair", post(pairing::create_pair))
        .route("/pair/claim", post(pairing::claim_pair))
        .with_state(state)
}
