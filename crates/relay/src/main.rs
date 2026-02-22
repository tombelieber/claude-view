use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,claude_view_relay=info".into()),
        )
        .init();

    let state = claude_view_relay::state::RelayState::new();
    let app = claude_view_relay::app(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 47893));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind relay port");
    info!("Relay server listening on {addr}");
    axum::serve(listener, app).await.expect("relay server");
}
