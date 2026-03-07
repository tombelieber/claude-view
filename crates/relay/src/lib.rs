pub mod auth;
pub mod pairing;
pub mod posthog;
pub mod push;
pub mod rate_limit;
pub mod state;
pub mod ws;

use axum::{
    http::{header, HeaderValue, Method, StatusCode},
    routing::{get, post},
    Router,
};
use state::RelayState;
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
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

    // CORS: allowlist production + dev origins
    let allowed_origins: Vec<HeaderValue> = vec![
        "https://claudeview.ai".parse().unwrap(),
        "https://claudeview.com".parse().unwrap(),
        "http://localhost:5173".parse().unwrap(),
        "http://localhost:8081".parse().unwrap(),
    ];
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    // HTTP routes get a 30s timeout via HandleErrorLayer (NOT applied to WS — connections are long-lived)
    let http_routes = Router::new()
        .route("/pair", post(pairing::create_pair))
        .route("/pair/claim", post(pairing::claim_pair))
        .route("/push-tokens", post(push::register_push_token))
        .route("/health", get(|| async { "ok" }))
        .layer(
            ServiceBuilder::new()
                .layer(axum::error_handling::HandleErrorLayer::new(
                    |_: tower::BoxError| async { StatusCode::REQUEST_TIMEOUT },
                ))
                .layer(tower::timeout::TimeoutLayer::new(Duration::from_secs(30))),
        );

    // WS route — no timeout
    let ws_routes = Router::new().route("/ws", get(ws::ws_handler));

    // Shared layers: body limit, CORS, tracing (order: outermost first)
    http_routes
        .merge(ws_routes)
        .layer(
            ServiceBuilder::new()
                .layer(RequestBodyLimitLayer::new(256 * 1024))
                .layer(cors),
        )
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state)
}
