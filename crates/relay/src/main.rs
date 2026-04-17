use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tracing::info;

use claude_view_relay::auth::SupabaseAuth;
use claude_view_relay::device_cache::DeviceCache;
use claude_view_relay::rate_limit::RateLimiter;
use claude_view_relay::state::RelayState;
use claude_view_relay::supabase::{HttpSupabaseClient, SupabaseClient};

#[tokio::main]
async fn main() {
    // Install TLS crypto provider before ANY network I/O. Cargo feature unification
    // compiles both ring and aws-lc-rs into the binary; rustls 0.23+ panics at runtime
    // if it can't auto-detect a single provider. Explicit selection = no ambiguity.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // Initialize unified observability (structured JSON file + dev stderr + optional Sentry).
    let mut obs_cfg = claude_view_observability::ServiceConfig::new(
        "claude-view-relay",
        env!("CARGO_PKG_VERSION"),
    );
    obs_cfg.sentry_dsn = std::env::var("SENTRY_DSN").ok().filter(|s| !s.is_empty());
    let _obs_handle = claude_view_observability::init(obs_cfg).expect("observability init");

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

    // Shared HTTP client — 10s timeout, reused for Supabase + OneSignal + PostHog
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("build http client");

    // Supabase REST client for device lookups (behind the cache).
    let supabase_client: Arc<dyn SupabaseClient> = Arc::new(
        HttpSupabaseClient::from_env(http_client.clone())
            .expect("SUPABASE_URL and SUPABASE_SECRET_KEY must be set for the relay"),
    );
    let device_cache = Arc::new(DeviceCache::new(supabase_client, Duration::from_secs(60)));

    // Rate limiters: WS per-message (60/min = 1/s) and /push-tokens (10/min)
    let ws_rl = Arc::new(RateLimiter::new(1.0, 60.0));
    let push_rl = Arc::new(RateLimiter::new(10.0 / 60.0, 10.0));

    let onesignal_app_id = std::env::var("ONESIGNAL_APP_ID").ok();
    let onesignal_rest_api_key = std::env::var("ONESIGNAL_REST_API_KEY").ok();
    let onesignal_http = if onesignal_app_id.is_some() && onesignal_rest_api_key.is_some() {
        Some(http_client.clone())
    } else {
        None
    };

    let posthog_api_key = std::env::var("POSTHOG_API_KEY")
        .ok()
        .filter(|s| !s.is_empty());
    let posthog_http = posthog_api_key.as_ref().map(|_| http_client.clone());

    let state = RelayState {
        connections: Arc::new(DashMap::new()),
        supabase_auth,
        http: http_client.clone(),
        device_cache,
        onesignal_app_id,
        onesignal_rest_api_key,
        onesignal_http,
        posthog_api_key,
        posthog_http,
        ws_rate_limiter: ws_rl.clone(),
        push_rate_limiter: push_rl.clone(),
    };

    let app = claude_view_relay::app(state);

    // Spawn periodic rate-limiter bucket eviction (every 5 min, stale after 10 min)
    let ws_rl_evict = ws_rl.clone();
    let push_rl_evict = push_rl.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            ws_rl_evict.evict_stale(Duration::from_secs(600)).await;
            push_rl_evict.evict_stale(Duration::from_secs(600)).await;
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
