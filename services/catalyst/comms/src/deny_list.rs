//! External wallet deny list shared by comms admission surfaces.

use crate::auth::normalize_eth_address;
use crate::http::Outbound;
use anyhow::Context;
use serde::Deserialize;
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;
const DENY_LIST_TTL: Duration = Duration::from_secs(5 * 60);

#[derive(Deserialize)]
struct DenyListDoc {
    #[serde(default)]
    users: Option<Vec<DenyUser>>,
}

#[derive(Deserialize)]
struct DenyUser {
    #[serde(default)]
    wallet: Option<String>,
}

pub struct DenyList {
    url: Option<String>,
    http: Outbound,
    ttl: Duration,
    cache: TtlCell<HashSet<String>>,
}

impl DenyList {
    pub fn new(url: Option<String>, http: Outbound) -> Arc<Self> {
        Self::with_ttl(url, http, DENY_LIST_TTL)
    }

    pub fn with_ttl(url: Option<String>, http: Outbound, ttl: Duration) -> Arc<Self> {
        let url = url.filter(|u| !u.is_empty());
        Arc::new(Self {
            url,
            http,
            ttl,
            cache: TtlCell::new("archipelago-deny-list"),
        })
    }

    pub fn is_armed(&self) -> bool {
        self.url.is_some()
    }

    pub async fn is_denied(&self, address: &str) -> bool {
        if self.url.is_none() {
            return false;
        }
        let Some(normalized) = normalize_eth_address(address) else {
            return true;
        };
        let wallets = self.current().await;
        wallets.contains(&normalized)
    }

    async fn current(&self) -> HashSet<String> {
        self.cache
            .get_or_refresh_backoff(self.ttl, || self.fetch())
            .await
            .unwrap_or_default()
    }

    async fn fetch(&self) -> anyhow::Result<HashSet<String>> {
        let url = self.url.as_deref().context("deny list url is unset")?;
        let resp = match self.http.get(url).send().await {
            Ok(r) => r,
            Err(_) => {
                tracing::warn!("deny list fetch failed, keeping last known list");
                anyhow::bail!("deny list fetch failed");
            }
        };
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "deny list non-OK status, keeping last known list");
            anyhow::bail!("deny list non-OK status {}", resp.status());
        }
        let doc = match resp.json::<DenyListDoc>().await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(error = %e, "deny list malformed body, keeping last known list");
                return Err(e).context("deny list malformed body");
            }
        };
        match doc.users {
            Some(users) => Ok(users
                .into_iter()
                .filter_map(|u| u.wallet)
                .filter_map(|w| normalize_eth_address(&w))
                .collect()),
            None => {
                tracing::warn!("deny list missing 'users' field, treating as empty");
                Ok(HashSet::new())
            }
        }
    }
}

struct Cached<T> {
    value: T,
    fetched_at: Instant,
}

struct Slot<T> {
    cached: Option<Cached<T>>,
    attempted_at: Option<Instant>,
}

/// One value with a TTL. A failed refresh serves the previous value rather than propagating the
/// error, so a flaky upstream degrades into staleness instead of an outage; the refresh runs under
/// the slot mutex, so concurrent callers coalesce onto one fetch.
pub struct TtlCell<T> {
    name: &'static str,
    slot: Mutex<Slot<T>>,
}

impl<T: Clone> TtlCell<T> {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            slot: Mutex::new(Slot {
                cached: None,
                attempted_at: None,
            }),
        }
    }

    /// A failed fetch also starts the TTL clock, so an upstream that is down or refusing is
    /// retried once per `ttl` instead of on every call. Returns the freshest value available, or
    /// `None` while no fetch has ever succeeded.
    pub async fn get_or_refresh_backoff<F, Fut, E>(&self, ttl: Duration, fetch: F) -> Option<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut slot = self.slot.lock().await;

        if let Some(cached) = slot.cached.as_ref()
            && cached.fetched_at.elapsed() < ttl
        {
            return Some(cached.value.clone());
        }
        if let Some(attempted_at) = slot.attempted_at
            && attempted_at.elapsed() < ttl
        {
            return slot.cached.as_ref().map(|c| c.value.clone());
        }

        slot.attempted_at = Some(Instant::now());
        match fetch().await {
            Ok(value) => {
                slot.cached = Some(Cached {
                    value: value.clone(),
                    fetched_at: Instant::now(),
                });
                Some(value)
            }
            Err(err) => {
                tracing::warn!(cache = self.name, %err, "refresh failed; backing off for the ttl");
                slot.cached.as_ref().map(|c| c.value.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    #[derive(Clone, Default)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    impl Write for LogBuffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn http() -> Outbound {
        Outbound::for_tests().allowing_private_hosts()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn deny_list_transport_logs_never_retain_the_configured_url() {
        const CANARY: &str = "archipelago-ban-private-material-canary";
        let base = format!("http://127.0.0.1:1/{CANARY}");
        let buffer = LogBuffer::default();
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .without_time()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        let deny_list = DenyList::with_ttl(
            Some(format!("{base}/deny-list")),
            http(),
            Duration::from_secs(1),
        );
        assert!(deny_list.fetch().await.is_err());

        let logs = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        assert!(logs.contains("deny list fetch failed, keeping last known list"));
        assert!(!logs.contains(CANARY), "configured URL survived in {logs}");
    }

    #[tokio::test]
    async fn armed_deny_list_rejects_malformed_address_without_fetching() {
        let deny = DenyList::with_ttl(
            Some("http://127.0.0.1:9/denylist".to_string()),
            http(),
            Duration::from_secs(300),
        );
        assert!(deny.is_denied("not-an-address").await);
    }

    #[tokio::test]
    async fn a_failed_deny_list_fetch_backs_off_for_the_ttl_and_keeps_the_last_list() {
        let cell: TtlCell<u32> = TtlCell::new("t");
        let fetches = std::sync::atomic::AtomicUsize::new(0);
        let fetch = |ok: Result<u32, &'static str>| {
            fetches.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async move { ok }
        };
        assert_eq!(
            cell.get_or_refresh_backoff(Duration::from_secs(60), || fetch(Ok(1)))
                .await,
            Some(1)
        );
        assert_eq!(
            cell.get_or_refresh_backoff(Duration::from_secs(60), || fetch(Ok(2)))
                .await,
            Some(1)
        );
        assert_eq!(
            cell.get_or_refresh_backoff(Duration::ZERO, || fetch(Err("down")))
                .await,
            Some(1)
        );
        assert_eq!(fetches.load(std::sync::atomic::Ordering::SeqCst), 2);
        let empty: TtlCell<u32> = TtlCell::new("e");
        assert_eq!(
            empty
                .get_or_refresh_backoff(Duration::from_secs(60), || fetch(Err("down")))
                .await,
            None
        );
        assert_eq!(
            empty
                .get_or_refresh_backoff(Duration::from_secs(60), || fetch(Ok(3)))
                .await,
            None,
            "backing off"
        );
    }

    #[tokio::test]
    async fn disarmed_deny_list_allows_everyone() {
        let deny = DenyList::new(None, http());
        assert!(!deny.is_armed());
        assert!(!deny.is_denied("0xdeadbeef").await);
    }
}
