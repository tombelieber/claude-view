//! Generic TTL cache for upstream API responses.
//!
//! Usage:
//! ```ignore
//! let cache = CachedUpstream::<MyResponse>::new(Duration::from_secs(300));
//! let value = cache.get_or_fetch(|| async { fetch_from_api().await }).await?;
//! ```

use std::future::Future;
use std::time::{Duration, Instant};

use tokio::sync::{RwLock, Semaphore};

struct CacheEntry<T> {
    value: T,
    fetched_at: Instant,
}

/// Error type for `force_refresh` — typed to avoid fragile string matching.
#[derive(Debug)]
pub enum CacheError {
    /// Last force-refresh attempt was too recent. `wait_secs` = seconds until retry.
    TooSoon { wait_secs: u64 },
    /// The upstream fetch itself failed.
    Fetch(String),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::TooSoon { wait_secs } => {
                write!(f, "Too soon. Try again in {wait_secs}s")
            }
            CacheError::Fetch(e) => write!(f, "{e}"),
        }
    }
}

/// A TTL-based cache for a single upstream API response.
///
/// Thread-safe (behind `tokio::sync::RwLock`) and `Send + Sync`.
/// Designed to sit on `AppState` as a field.
///
/// Uses `Semaphore(1)` to serialize concurrent fetches on cache miss,
/// preventing thundering herd on cold start (per project rule:
/// "Semaphore(1) for external calls").
pub struct CachedUpstream<T: Clone + Send + Sync> {
    inner: RwLock<Option<CacheEntry<T>>>,
    ttl: Duration,
    /// Serializes concurrent fetch attempts — only one in-flight fetch at a time.
    fetch_semaphore: Semaphore,
    /// Tracks last force-refresh *attempt* (not success) for spam guard.
    /// Separate from `fetched_at` so the guard works even when upstream errors.
    last_force_refresh_at: RwLock<Option<Instant>>,
}

impl<T: Clone + Send + Sync> CachedUpstream<T> {
    /// Create a new cache with the given TTL.
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: RwLock::new(None),
            ttl,
            fetch_semaphore: Semaphore::new(1),
            last_force_refresh_at: RwLock::new(None),
        }
    }

    /// Return cached value if within TTL, otherwise call `fetcher` and cache the result.
    ///
    /// On fetch error, the cache is NOT updated (stale data is better than no data,
    /// but we don't cache errors). Concurrent cache misses are serialized by a
    /// semaphore — only the first caller fetches, subsequent callers get the result.
    pub async fn get_or_fetch<F, Fut>(&self, fetcher: F) -> Result<T, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        // Fast path: read lock, check TTL.
        {
            let guard = self.inner.read().await;
            if let Some(entry) = guard.as_ref() {
                if entry.fetched_at.elapsed() < self.ttl {
                    return Ok(entry.value.clone());
                }
            }
        }

        // Slow path: serialize concurrent fetches via semaphore.
        let _permit = self
            .fetch_semaphore
            .acquire()
            .await
            .expect("semaphore closed");

        // Double-check after acquiring permit — another task may have just fetched.
        {
            let guard = self.inner.read().await;
            if let Some(entry) = guard.as_ref() {
                if entry.fetched_at.elapsed() < self.ttl {
                    return Ok(entry.value.clone());
                }
            }
        }

        let value = fetcher().await?;
        {
            let mut guard = self.inner.write().await;
            *guard = Some(CacheEntry {
                value: value.clone(),
                fetched_at: Instant::now(),
            });
        }
        Ok(value)
    }

    /// Bypass TTL and fetch fresh data. Returns `Err(CacheError::TooSoon)` if the
    /// last force-refresh *attempt* was less than `min_interval` ago.
    ///
    /// The spam guard tracks attempt time (not success), so repeated failures
    /// during an upstream outage are still rate-limited.
    pub async fn force_refresh<F, Fut>(
        &self,
        min_interval: Duration,
        fetcher: F,
    ) -> Result<T, CacheError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        // Spam guard: check last attempt time (not last success).
        if !min_interval.is_zero() {
            let guard = self.last_force_refresh_at.read().await;
            if let Some(last) = *guard {
                let elapsed = last.elapsed();
                if elapsed < min_interval {
                    let wait_secs = (min_interval - elapsed).as_secs().max(1);
                    return Err(CacheError::TooSoon { wait_secs });
                }
            }
        }

        // Record attempt time BEFORE fetching (so even failures count).
        {
            let mut guard = self.last_force_refresh_at.write().await;
            *guard = Some(Instant::now());
        }

        let value = fetcher().await.map_err(CacheError::Fetch)?;
        {
            let mut guard = self.inner.write().await;
            *guard = Some(CacheEntry {
                value: value.clone(),
                fetched_at: Instant::now(),
            });
        }
        Ok(value)
    }

    /// Return the cached value without fetching. `None` if cache is empty or lock contention.
    pub fn get_cached(&self) -> Option<T> {
        self.inner
            .try_read()
            .ok()
            .and_then(|guard| guard.as_ref().map(|e| e.value.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn get_or_fetch_caches_within_ttl() {
        let cache = CachedUpstream::<String>::new(Duration::from_secs(60));
        let call_count = Arc::new(AtomicU32::new(0));

        let cc = call_count.clone();
        let v1 = cache
            .get_or_fetch(|| {
                let cc = cc.clone();
                async move {
                    cc.fetch_add(1, Ordering::SeqCst);
                    Ok("hello".to_string())
                }
            })
            .await
            .unwrap();
        assert_eq!(v1, "hello");

        let cc = call_count.clone();
        let v2 = cache
            .get_or_fetch(|| {
                let cc = cc.clone();
                async move {
                    cc.fetch_add(1, Ordering::SeqCst);
                    Ok("world".to_string())
                }
            })
            .await
            .unwrap();
        assert_eq!(v2, "hello"); // cached, not "world"
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn get_or_fetch_refreshes_after_ttl() {
        let cache = CachedUpstream::<String>::new(Duration::from_millis(10));
        let call_count = Arc::new(AtomicU32::new(0));

        let cc = call_count.clone();
        let v1 = cache
            .get_or_fetch(|| {
                let cc = cc.clone();
                async move {
                    cc.fetch_add(1, Ordering::SeqCst);
                    Ok("first".to_string())
                }
            })
            .await
            .unwrap();
        assert_eq!(v1, "first");

        tokio::time::sleep(Duration::from_millis(20)).await;

        let cc = call_count.clone();
        let v2 = cache
            .get_or_fetch(|| {
                let cc = cc.clone();
                async move {
                    cc.fetch_add(1, Ordering::SeqCst);
                    Ok("second".to_string())
                }
            })
            .await
            .unwrap();
        assert_eq!(v2, "second");
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn force_refresh_bypasses_ttl() {
        let cache = CachedUpstream::<String>::new(Duration::from_secs(60));

        cache
            .get_or_fetch(|| async { Ok("old".to_string()) })
            .await
            .unwrap();

        let v = cache
            .force_refresh(Duration::ZERO, || async { Ok("new".to_string()) })
            .await
            .unwrap();
        assert_eq!(v, "new");

        // Subsequent get_or_fetch should return the force-refreshed value.
        let v2 = cache
            .get_or_fetch(|| async { Ok("should not be called".to_string()) })
            .await
            .unwrap();
        assert_eq!(v2, "new");
    }

    #[tokio::test]
    async fn force_refresh_rejects_within_min_interval() {
        let cache = CachedUpstream::<String>::new(Duration::from_secs(60));

        let r1 = cache
            .force_refresh(Duration::from_secs(60), || async {
                Ok("first".to_string())
            })
            .await;
        assert!(r1.is_ok());

        let r2 = cache
            .force_refresh(Duration::from_secs(60), || async {
                Ok("second".to_string())
            })
            .await;
        assert!(matches!(r2, Err(CacheError::TooSoon { .. })));
    }

    #[tokio::test]
    async fn force_refresh_spam_guard_works_on_fetch_error() {
        let cache = CachedUpstream::<String>::new(Duration::from_secs(60));

        // First force_refresh fails with a Fetch error.
        let r1 = cache
            .force_refresh(Duration::from_secs(60), || async {
                Err::<String, String>("upstream down".to_string())
            })
            .await;
        assert!(matches!(r1, Err(CacheError::Fetch(_))));

        // Second force_refresh within interval is STILL rejected (tracks attempt, not success).
        let r2 = cache
            .force_refresh(Duration::from_secs(60), || async {
                Ok("second".to_string())
            })
            .await;
        assert!(matches!(r2, Err(CacheError::TooSoon { .. })));
    }

    #[tokio::test]
    async fn get_cached_returns_none_when_empty() {
        let cache = CachedUpstream::<String>::new(Duration::from_secs(60));
        assert!(cache.get_cached().is_none());
    }

    #[tokio::test]
    async fn get_cached_returns_value_after_fetch() {
        let cache = CachedUpstream::<String>::new(Duration::from_secs(60));

        cache
            .get_or_fetch(|| async { Ok("cached_value".to_string()) })
            .await
            .unwrap();

        assert_eq!(cache.get_cached(), Some("cached_value".to_string()));
    }

    #[tokio::test]
    async fn fetch_error_does_not_cache() {
        let cache = CachedUpstream::<String>::new(Duration::from_secs(60));

        let result = cache
            .get_or_fetch(|| async { Err::<String, String>("boom".to_string()) })
            .await;
        assert!(result.is_err());

        assert!(cache.get_cached().is_none());
    }
}
