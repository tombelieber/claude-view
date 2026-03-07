use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    last_access: Instant,
    rate: f64,
    capacity: f64,
}

impl TokenBucket {
    fn new(rate: f64, capacity: f64) -> Self {
        let now = Instant::now();
        Self {
            tokens: capacity,
            last_refill: now,
            last_access: now,
            rate,
            capacity,
        }
    }

    fn try_consume(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.rate).min(self.capacity);
        self.last_refill = now;
        self.last_access = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

pub struct RateLimiter {
    buckets: DashMap<String, Arc<Mutex<TokenBucket>>>,
    rate: f64,
    capacity: f64,
}

impl RateLimiter {
    pub fn new(requests_per_sec: f64, burst: f64) -> Self {
        Self {
            buckets: DashMap::new(),
            rate: requests_per_sec,
            capacity: burst,
        }
    }

    pub async fn check(&self, key: &str) -> bool {
        let arc = {
            let bucket = self.buckets.entry(key.to_string()).or_insert_with(|| {
                Arc::new(Mutex::new(TokenBucket::new(self.rate, self.capacity)))
            });
            bucket.value().clone()
        };
        let result = arc.lock().await.try_consume();
        result
    }

    pub async fn evict_stale(&self, max_idle: Duration) {
        let now = Instant::now();
        let keys: Vec<String> = self.buckets.iter().map(|e| e.key().clone()).collect();
        let mut stale_keys = Vec::new();
        for key in &keys {
            if let Some(bucket_ref) = self.buckets.get(key) {
                let arc = bucket_ref.value().clone();
                drop(bucket_ref);
                if now.duration_since(arc.lock().await.last_access) > max_idle {
                    stale_keys.push(key.clone());
                }
            }
        }
        for key in &stale_keys {
            self.buckets.remove(key);
        }
        if !stale_keys.is_empty() {
            tracing::debug!("Evicted {} stale rate-limit buckets", stale_keys.len());
        }
    }
}
