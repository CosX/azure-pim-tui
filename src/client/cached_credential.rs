use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use azure_core::credentials::{AccessToken, TokenCredential, TokenRequestOptions};
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;

/// Wraps a [`TokenCredential`] with a per-scope token cache so we don't
/// re-invoke the underlying credential (e.g. shelling out to `az`) on every
/// HTTP request. Concurrent callers for the same scope set serialize on the
/// mutex; the first refresh populates the cache and the rest reuse it.
#[derive(Debug)]
pub struct CachedTokenCredential {
    inner: Arc<dyn TokenCredential>,
    cache: Mutex<HashMap<Vec<String>, AccessToken>>,
}

impl CachedTokenCredential {
    pub fn new(inner: Arc<dyn TokenCredential>) -> Self {
        Self {
            inner,
            cache: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl TokenCredential for CachedTokenCredential {
    async fn get_token(
        &self,
        scopes: &[&str],
        options: Option<TokenRequestOptions<'_>>,
    ) -> azure_core::Result<AccessToken> {
        let key: Vec<String> = scopes.iter().map(|s| (*s).to_string()).collect();
        let mut cache = self.cache.lock().await;

        if let Some(token) = cache.get(&key) {
            if token.expires_on > OffsetDateTime::now_utc() + Duration::minutes(5) {
                return Ok(token.clone());
            }
        }

        let fresh = self.inner.get_token(scopes, options).await?;
        cache.insert(key, fresh.clone());
        Ok(fresh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use azure_core::credentials::Secret;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct CountingCredential {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl TokenCredential for CountingCredential {
        async fn get_token(
            &self,
            _scopes: &[&str],
            _options: Option<TokenRequestOptions<'_>>,
        ) -> azure_core::Result<AccessToken> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Ok(AccessToken {
                token: Secret::new("dummy".to_string()),
                expires_on: OffsetDateTime::now_utc() + Duration::hours(1),
            })
        }
    }

    #[tokio::test]
    async fn parallel_calls_hit_underlying_credential_once() {
        let inner = Arc::new(CountingCredential {
            calls: AtomicUsize::new(0),
        });
        let cached: Arc<dyn TokenCredential> = Arc::new(CachedTokenCredential::new(inner.clone()));

        let mut futs = Vec::new();
        for _ in 0..50 {
            let c = cached.clone();
            futs.push(tokio::spawn(async move {
                c.get_token(&["https://management.azure.com/.default"], None)
                    .await
                    .unwrap();
            }));
        }
        for f in futs {
            f.await.unwrap();
        }

        assert_eq!(inner.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn distinct_scopes_cached_independently() {
        let inner = Arc::new(CountingCredential {
            calls: AtomicUsize::new(0),
        });
        let cached: Arc<dyn TokenCredential> = Arc::new(CachedTokenCredential::new(inner.clone()));

        for _ in 0..5 {
            cached.get_token(&["scope_a"], None).await.unwrap();
            cached.get_token(&["scope_b"], None).await.unwrap();
        }

        assert_eq!(inner.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn expired_token_triggers_refresh() {
        #[derive(Debug)]
        struct ShortLivedCredential {
            calls: AtomicUsize,
        }
        #[async_trait]
        impl TokenCredential for ShortLivedCredential {
            async fn get_token(
                &self,
                _scopes: &[&str],
                _options: Option<TokenRequestOptions<'_>>,
            ) -> azure_core::Result<AccessToken> {
                self.calls.fetch_add(1, Ordering::SeqCst);
                Ok(AccessToken {
                    token: Secret::new("x".to_string()),
                    expires_on: OffsetDateTime::now_utc() + Duration::minutes(1),
                })
            }
        }

        let inner = Arc::new(ShortLivedCredential {
            calls: AtomicUsize::new(0),
        });
        let cached: Arc<dyn TokenCredential> = Arc::new(CachedTokenCredential::new(inner.clone()));

        for _ in 0..3 {
            cached.get_token(&["s"], None).await.unwrap();
        }
        // Each call sees expiry within the 5-min refresh window, so cache never returns.
        assert_eq!(inner.calls.load(Ordering::SeqCst), 3);
    }
}
