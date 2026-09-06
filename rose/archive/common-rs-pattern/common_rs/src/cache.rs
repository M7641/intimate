//! Single-flight TTL cache — port of `common.backend.cache.TTLQueryCache`.
//!
//! Built atop `moka::future::Cache` whose `get_with` provides single-flight semantics
//! identical to the python implementation: concurrent misses on the same key share a
//! single in-flight load, the rest of the cache is unaffected.

use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;

#[derive(Clone)]
pub struct TtlCache<K, V>
where
    K: Hash + Eq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    name: Arc<str>,
    inner: Cache<K, V>,
}

impl<K, V> TtlCache<K, V>
where
    K: Hash + Eq + Send + Sync + Clone + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(name: impl Into<Arc<str>>, max_capacity: u64, ttl: Duration) -> Self {
        Self {
            name: name.into(),
            inner: Cache::builder()
                .max_capacity(max_capacity)
                .time_to_live(ttl)
                .build(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Single-flight get-or-load. Concurrent misses for the same key share the same future.
    pub async fn get_with<F>(&self, key: K, fetch: F) -> V
    where
        F: Future<Output = V>,
    {
        self.inner.get_with(key, fetch).await
    }

    /// Single-flight get-or-load with errors propagated to all waiters.
    /// On Err, the value is NOT cached, so the next request will retry.
    pub async fn try_get_with<F, E>(&self, key: K, fetch: F) -> Result<V, Arc<E>>
    where
        F: Future<Output = Result<V, E>>,
        E: Send + Sync + 'static,
    {
        self.inner.try_get_with(key, fetch).await
    }

    pub async fn get(&self, key: &K) -> Option<V> {
        self.inner.get(key).await
    }

    pub async fn insert(&self, key: K, value: V) {
        self.inner.insert(key, value).await;
    }

    pub async fn invalidate(&self, key: &K) {
        self.inner.invalidate(key).await;
    }

    pub fn invalidate_all(&self) {
        self.inner.invalidate_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn single_flight_coalesces_concurrent_misses() {
        let cache: TtlCache<String, i32> = TtlCache::new("test", 10, Duration::from_secs(60));
        let counter = Arc::new(AtomicUsize::new(0));

        let mut handles = vec![];
        for _ in 0..50 {
            let cache = cache.clone();
            let counter = counter.clone();
            handles.push(tokio::spawn(async move {
                cache
                    .get_with("k".to_string(), async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        42
                    })
                    .await
            }));
        }

        for h in handles {
            assert_eq!(h.await.unwrap(), 42);
        }
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "fetch must run exactly once"
        );
    }

    #[tokio::test]
    async fn invalidate_drops_entry() {
        let cache: TtlCache<String, i32> = TtlCache::new("test", 10, Duration::from_secs(60));
        cache.insert("k".to_string(), 1).await;
        assert_eq!(cache.get(&"k".to_string()).await, Some(1));
        cache.invalidate(&"k".to_string()).await;
        assert_eq!(cache.get(&"k".to_string()).await, None);
    }
}
