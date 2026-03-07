use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

use claude_view_relay::auth::SupabaseAuth;
use claude_view_relay::rate_limit::RateLimiter;

#[tokio::main]
async fn main() {
    // Sentry init — REPLACES the standalone tracing_subscriber::fmt().init()
    let _sentry = sentry::init((
        std::env::var("SENTRY_DSN").unwrap_or_default(),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(
                std::env::var("ENVIRONMENT")
                    .unwrap_or_else(|_| "development".to_string())
                    .into(),
            ),
            traces_sample_rate: 0.1,
            ..Default::default()
        },
    ));

    use tracing_subscriber::prelude::*;
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,claude_view_relay=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(sentry_tracing::layer())
        .init();

    // Load Supabase JWT validator (optional — disabled if SUPABASE_URL not set)
    let supabase_auth = match std::env::var("SUPABASE_URL") {
        Ok(url) => match SupabaseAuth::from_supabase_url(&url).await {
            Ok(auth) => {
                info!("Supabase JWT validation enabled");
                Some(Arc::new(auth))
            }
            Err(e) => {
                tracing::warn!("Supabase JWKS load failed: {e}");
                None
            }
        },
        Err(_) => {
            info!("SUPABASE_URL not set — JWT auth disabled");
            None
        }
    };

    // Rate limiters: 5 req/min burst 5 for /pair, 10 req/min burst 10 for /pair/claim,
    // 10 req/min burst 10 for /push-tokens
    let pair_rl = Arc::new(RateLimiter::new(5.0 / 60.0, 5.0));
    let claim_rl = Arc::new(RateLimiter::new(10.0 / 60.0, 10.0));
    let push_rl = Arc::new(RateLimiter::new(10.0 / 60.0, 10.0));

    let state = claude_view_relay::state::RelayState::new(
        supabase_auth,
        pair_rl.clone(),
        claim_rl.clone(),
        push_rl.clone(),
    );
    let app = claude_view_relay::app(state);

    // Spawn periodic rate-limiter bucket eviction (every 5 min, stale after 10 min)
    let pair_rl_clone = pair_rl.clone();
    let claim_rl_clone = claim_rl.clone();
    let push_rl_clone = push_rl.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            pair_rl_clone.evict_stale(Duration::from_secs(600)).await;
            claim_rl_clone.evict_stale(Duration::from_secs(600)).await;
            push_rl_clone.evict_stale(Duration::from_secs(600)).await;
        }
    });

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(47893);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind relay port");
    info!("Relay server listening on {addr}");
    axum::serve(listener, app).await.expect("relay server");
}
