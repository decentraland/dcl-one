//! The node-wide budget of contract-wallet checks per client source. Verifying
//! an EIP-1654 link costs an `isValidSignature` call over RPC, and any signed
//! route (a public read with an optional signer included) runs one for a chain
//! that carries such a link, so an anonymous client attaching refused
//! contract-wallet chains could spend the node's RPC quota and turn every real
//! contract wallet's request into a 503.
//!
//! [`SourceBudgeted`] wraps the RPC validator below the acceptance cache
//! (`cached_budgeted_validator`): each call that would reach the RPC first
//! spends one token of the request's client source, by the pre-auth budgets
//! of `limits::preauth_keys` (the `/forward-blockchain` ones), and a spent
//! budget is [`AuthError::Eip1654RateLimited`] before any RPC call. A cached
//! acceptance spends nothing.
//!
//! The client source is the server's own record of who is asking, a
//! task-local set once for every request ([`scope_client_source`]): a trusted
//! proxy's single `X-Real-IP`, or a public TCP peer
//! (`client_ip::distinct_client_ip`), never a header a client chose. Work
//! with no source is not counted, as on `/forward-blockchain`: sync, jobs and
//! the read-through's fills (spawned outside any request), and a request whose
//! peer names no single client (an internal peer behind an unlisted proxy).
//! A websocket verifies its frames after the upgrade, in the task the upgrade
//! spawns, so its handler upgrades through [`on_upgrade`], which keeps the
//! request's source; other work a request spawns takes [`with_source`] with
//! the [`current_source`].

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::{ConnectInfo, Request};
use axum::middleware::Next;

use crate::auth::AuthError;
use crate::auth::eip1654::Eip1654Validator;
use crate::limits::{KeyedLimiter, preauth_keys};

/// Client sources (and IPv6 /48s) whose budgets are kept; past it the least
/// recently used goes.
pub const BUDGET_KEYS: usize = 10_000;

tokio::task_local! {
    /// The client source of the request in flight: `Some` when its peer names
    /// one client, `None` when it does not. Unset outside any request.
    static CLIENT_SOURCE: Option<String>;
}

/// Runs every request `router` answers inside its client source, from the
/// socket peer and, for a peer in `trusted_proxies` (`HTTP_TRUSTED_PROXY_CIDRS`),
/// its single `X-Real-IP`. Needs the server's `ConnectInfo<SocketAddr>`; an
/// in-process request without one has no source.
pub fn scope_client_source(
    router: axum::Router,
    trusted_proxies: Vec<ipnet::IpNet>,
) -> axum::Router {
    let trusted_proxies: Arc<[ipnet::IpNet]> = trusted_proxies.into();
    router.layer(axum::middleware::from_fn(
        move |request: Request, next: Next| {
            let trusted_proxies = trusted_proxies.clone();
            async move {
                let peer = request
                    .extensions()
                    .get::<ConnectInfo<SocketAddr>>()
                    .map(|ConnectInfo(peer)| *peer);
                let source =
                    crate::client_ip::distinct_client_ip(request.headers(), peer, &trusted_proxies);
                CLIENT_SOURCE.scope(source, next.run(request)).await
            }
        },
    ))
}

/// The client source of the request this task serves, if it names one.
pub fn current_source() -> Option<String> {
    CLIENT_SOURCE.try_with(Clone::clone).ok().flatten()
}

/// `future` as if a request from `source` ran it: for work a request hands to
/// another task, `with_source(current_source(), future)` taken in the
/// request's own task.
pub fn with_source<F: Future>(
    source: Option<String>,
    future: F,
) -> impl Future<Output = F::Output> {
    CLIENT_SOURCE.scope(source, future)
}

/// `upgrade.on_upgrade(callback)` with the socket's future under this
/// request's client source. axum runs the callback in the task the upgrade
/// spawns, where the request's scope is gone, so the source is taken here, in
/// the handler. The socket's future always runs in a scope (with no source
/// outside a request), so it passes [`debug_assert_scoped`].
pub fn on_upgrade<C, Fut>(
    upgrade: axum::extract::ws::WebSocketUpgrade,
    callback: C,
) -> axum::response::Response
where
    C: FnOnce(axum::extract::ws::WebSocket) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let source = current_source();
    upgrade.on_upgrade(move |socket| CLIENT_SOURCE.scope(source, callback(socket)))
}

/// Debug builds: panics when a check a client started runs outside any
/// request scope, which would leave it unbudgeted. Each socket's frame
/// verification calls it, so an upgrade that skips [`on_upgrade`] fails its
/// own tests.
#[track_caller]
pub fn debug_assert_scoped(what: &str) {
    debug_assert!(
        CLIENT_SOURCE.try_with(|_| ()).is_ok(),
        "{what} verifies a signature outside a request's client-source scope; \
         upgrade through eip1654_budget::on_upgrade"
    );
}

/// The shared validator of a serving node: the RPC check behind the budget,
/// behind the acceptance cache, over the process-wide reqwest client.
pub fn cached_budgeted_validator(
    rpc_url: impl Into<String>,
    client: reqwest::Client,
) -> Arc<dyn Eip1654Validator> {
    let rpc = Arc::new(crate::auth::RpcEip1654Validator::new(rpc_url, client));
    Arc::new(crate::auth::ValidationCache::new(Arc::new(
        SourceBudgeted::new(rpc),
    )))
}

/// The budget [`SourceBudgeted`] and its fresh view spend from.
struct Budget {
    inner: Arc<dyn Eip1654Validator>,
    limiter: Arc<KeyedLimiter>,
}

impl Budget {
    /// One token of the current client source, or the refusal; work with no
    /// source is not counted.
    fn admit(&self) -> Result<(), AuthError> {
        match current_source() {
            Some(source) if !self.limiter.allow(&preauth_keys(&source)) => {
                Err(AuthError::Eip1654RateLimited)
            }
            _ => Ok(()),
        }
    }
}

/// `inner` with every call spending the current client source's budget first.
/// Place it directly over the RPC validator, below any cache: a cached
/// acceptance then spends nothing. Its fresh view (`fresh_validator`) asks
/// `inner`'s fresh one under the same budget.
pub struct SourceBudgeted {
    budget: Budget,
    fresh: Option<FreshBudgeted>,
}

impl SourceBudgeted {
    pub fn new(inner: Arc<dyn Eip1654Validator>) -> Self {
        Self::with_limiter(inner, Arc::new(KeyedLimiter::new(BUDGET_KEYS)))
    }

    fn with_limiter(inner: Arc<dyn Eip1654Validator>, limiter: Arc<KeyedLimiter>) -> Self {
        let fresh = inner.fresh_validator().is_some().then(|| {
            FreshBudgeted(Budget {
                inner: inner.clone(),
                limiter: limiter.clone(),
            })
        });
        Self {
            budget: Budget { inner, limiter },
            fresh,
        }
    }
}

#[async_trait]
impl Eip1654Validator for SourceBudgeted {
    fn fresh_validator(&self) -> Option<&dyn Eip1654Validator> {
        self.fresh
            .as_ref()
            .map(|fresh| fresh as &dyn Eip1654Validator)
    }

    async fn validate_signature(
        &self,
        contract_address: &str,
        hash: &[u8],
        signature: &[u8],
    ) -> Result<bool, AuthError> {
        self.budget.admit()?;
        self.budget
            .inner
            .validate_signature(contract_address, hash, signature)
            .await
    }

    async fn validate_signature_at_block(
        &self,
        contract_address: &str,
        hash: &[u8],
        signature: &[u8],
        block: u64,
    ) -> Result<bool, AuthError> {
        self.budget.admit()?;
        self.budget
            .inner
            .validate_signature_at_block(contract_address, hash, signature, block)
            .await
    }

    async fn verification_time_block(&self, started_ms: i64) -> Result<Option<u64>, AuthError> {
        self.budget.admit()?;
        self.budget.inner.verification_time_block(started_ms).await
    }
}

/// [`SourceBudgeted`]'s fresh view: `inner`'s fresh validator under the same budget.
struct FreshBudgeted(Budget);

impl FreshBudgeted {
    fn fresh(&self) -> Result<&dyn Eip1654Validator, AuthError> {
        self.0.inner.fresh_validator().ok_or_else(|| {
            AuthError::Eip1654ValidationFailed("the validator has no fresh path".into())
        })
    }
}

#[async_trait]
impl Eip1654Validator for FreshBudgeted {
    fn fresh_validator(&self) -> Option<&dyn Eip1654Validator> {
        Some(self)
    }

    async fn validate_signature(
        &self,
        contract_address: &str,
        hash: &[u8],
        signature: &[u8],
    ) -> Result<bool, AuthError> {
        self.0.admit()?;
        self.fresh()?
            .validate_signature(contract_address, hash, signature)
            .await
    }

    async fn validate_signature_at_block(
        &self,
        contract_address: &str,
        hash: &[u8],
        signature: &[u8],
        block: u64,
    ) -> Result<bool, AuthError> {
        self.0.admit()?;
        self.fresh()?
            .validate_signature_at_block(contract_address, hash, signature, block)
            .await
    }

    async fn verification_time_block(&self, started_ms: i64) -> Result<Option<u64>, AuthError> {
        self.0.admit()?;
        self.fresh()?.verification_time_block(started_ms).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::{PREAUTH_PER_MINUTE, PREAUTH_PER_MINUTE_PER_48};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Accepts every signature, counting the calls that reach it and the fresh ones.
    #[derive(Default)]
    struct Counting {
        calls: AtomicUsize,
        fresh: FreshCounting,
    }

    #[derive(Default)]
    struct FreshCounting(AtomicUsize);

    impl Counting {
        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst) + self.fresh.0.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl Eip1654Validator for Counting {
        fn fresh_validator(&self) -> Option<&dyn Eip1654Validator> {
            Some(&self.fresh)
        }
        async fn validate_signature(&self, _: &str, _: &[u8], _: &[u8]) -> Result<bool, AuthError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(true)
        }
        async fn validate_signature_at_block(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
            _: u64,
        ) -> Result<bool, AuthError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(true)
        }
        async fn verification_time_block(&self, _: i64) -> Result<Option<u64>, AuthError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Some(1))
        }
    }

    #[async_trait]
    impl Eip1654Validator for FreshCounting {
        fn fresh_validator(&self) -> Option<&dyn Eip1654Validator> {
            Some(self)
        }
        async fn validate_signature(&self, _: &str, _: &[u8], _: &[u8]) -> Result<bool, AuthError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(true)
        }
    }

    const WALLET: &str = "0x00000000000000000000000000000000000000aa";

    async fn ask(v: &dyn Eip1654Validator, n: u8) -> Result<bool, AuthError> {
        v.validate_signature(WALLET, &[n], &[n]).await
    }

    /// A source trips at its budget before the inner validator is asked;
    /// another source, and work with no source, keep theirs.
    #[tokio::test]
    async fn a_source_spends_its_budget_before_the_rpc() {
        let inner = Arc::new(Counting::default());
        let v = SourceBudgeted::new(inner.clone());
        let a = Some("203.0.113.10".to_string());
        with_source(a.clone(), async {
            for n in 0..PREAUTH_PER_MINUTE {
                assert!(ask(&v, n as u8).await.unwrap());
            }
            assert!(matches!(
                ask(&v, 0).await,
                Err(AuthError::Eip1654RateLimited)
            ));
            assert!(matches!(
                v.validate_signature_at_block(WALLET, &[0], &[0], 7).await,
                Err(AuthError::Eip1654RateLimited)
            ));
            assert!(matches!(
                v.verification_time_block(0).await,
                Err(AuthError::Eip1654RateLimited)
            ));
        })
        .await;
        assert_eq!(
            inner.calls(),
            PREAUTH_PER_MINUTE as usize,
            "no RPC past the budget"
        );

        with_source(Some("203.0.113.11".into()), async {
            assert!(
                ask(&v, 0).await.unwrap(),
                "another source has its own budget"
            );
        })
        .await;
        with_source(None, async {
            for n in 0..2 * PREAUTH_PER_MINUTE {
                assert!(ask(&v, n as u8).await.unwrap(), "no source is not counted");
            }
        })
        .await;
        for n in 0..2 * PREAUTH_PER_MINUTE {
            assert!(ask(&v, n as u8).await.unwrap(), "outside a request");
        }
    }

    /// Every /64 of one IPv6 /48 shares the /48's budget.
    #[tokio::test]
    async fn an_ipv6_48_shares_one_budget() {
        let inner = Arc::new(Counting::default());
        let v = SourceBudgeted::new(inner.clone());
        let mut allowed = 0;
        for n in 0..PREAUTH_PER_MINUTE_PER_48 + 40 {
            let source = format!("2001:db8:5:{n:x}::1");
            if with_source(Some(source), ask(&v, 0)).await.is_ok() {
                allowed += 1;
            }
        }
        assert_eq!(allowed, PREAUTH_PER_MINUTE_PER_48);
        assert_eq!(inner.calls(), PREAUTH_PER_MINUTE_PER_48 as usize);
    }

    /// Behind the cache a repeated acceptance spends nothing; the fresh view
    /// asks the inner fresh validator under the same budget.
    #[tokio::test]
    async fn cached_acceptances_are_free_and_fresh_checks_are_budgeted() {
        let inner = Arc::new(Counting::default());
        let cached =
            crate::auth::ValidationCache::new(Arc::new(SourceBudgeted::new(inner.clone())));
        let source = Some("198.51.100.20".to_string());
        with_source(source.clone(), async {
            for _ in 0..3 * PREAUTH_PER_MINUTE {
                assert!(ask(&cached, 1).await.unwrap());
            }
        })
        .await;
        assert_eq!(inner.calls(), 1, "one RPC call, then the cache");

        let fresh = cached
            .fresh_validator()
            .expect("the fresh path is forwarded");
        with_source(source, async {
            for n in 1..PREAUTH_PER_MINUTE {
                assert!(ask(fresh, n as u8).await.unwrap());
            }
            assert!(matches!(
                ask(fresh, 1).await,
                Err(AuthError::Eip1654RateLimited)
            ));
            assert!(
                matches!(ask(&cached, 2).await, Err(AuthError::Eip1654RateLimited)),
                "one budget for both views"
            );
            assert!(
                ask(&cached, 1).await.unwrap(),
                "a cached acceptance still answers"
            );
        })
        .await;
        assert_eq!(
            inner.fresh.0.load(Ordering::SeqCst),
            PREAUTH_PER_MINUTE as usize - 1,
            "fresh checks reached the inner fresh validator"
        );
    }

    /// A validator without a fresh path keeps none behind the budget.
    #[test]
    fn no_fresh_path_is_invented() {
        struct NoFresh;
        #[async_trait]
        impl Eip1654Validator for NoFresh {
            async fn validate_signature(
                &self,
                _: &str,
                _: &[u8],
                _: &[u8],
            ) -> Result<bool, AuthError> {
                Ok(true)
            }
        }
        assert!(
            SourceBudgeted::new(Arc::new(NoFresh))
                .fresh_validator()
                .is_none()
        );
    }

    /// A spawned task loses the request's source unless it is handed over,
    /// taken in the request's own task.
    #[tokio::test]
    async fn a_spawn_keeps_the_source_only_when_handed_over() {
        let kept = with_source(Some("192.0.2.44".into()), async {
            tokio::spawn(with_source(current_source(), async { current_source() }))
                .await
                .unwrap()
        })
        .await;
        assert_eq!(kept.as_deref(), Some("192.0.2.44"));
        let lost = with_source(Some("192.0.2.44".into()), async {
            tokio::spawn(async { current_source() }).await.unwrap()
        })
        .await;
        assert_eq!(lost, None, "a plain spawn loses it");
        tokio::spawn(with_source(None, async { debug_assert_scoped("test") }))
            .await
            .unwrap();
    }

    /// The layer names the client: a public peer, a trusted proxy's
    /// `X-Real-IP` (two clients behind one proxy, two sources), and nothing
    /// for an untrusted internal peer.
    #[tokio::test]
    async fn the_layer_names_each_client_behind_a_trusted_proxy() {
        use axum::body::Body;
        use axum::routing::get;
        use tower::ServiceExt;

        let app = scope_client_source(
            axum::Router::new().route(
                "/",
                get(|| async { current_source().unwrap_or_else(|| "-".into()) }),
            ),
            vec!["10.40.0.0/18".parse().unwrap()],
        );
        let source = |peer: &str, real_ip: Option<&str>| {
            let mut request = axum::http::Request::get("/");
            if let Some(ip) = real_ip {
                request = request.header("x-real-ip", ip);
            }
            let mut request = request.body(Body::empty()).unwrap();
            request
                .extensions_mut()
                .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
            let app = app.clone();
            async move {
                let response = app.oneshot(request).await.unwrap();
                let body = axum::body::to_bytes(response.into_body(), 1024)
                    .await
                    .unwrap();
                String::from_utf8(body.to_vec()).unwrap()
            }
        };
        assert_eq!(
            source("10.40.1.2:1", Some("203.0.113.1")).await,
            "203.0.113.1"
        );
        assert_eq!(
            source("10.40.1.2:1", Some("203.0.113.2")).await,
            "203.0.113.2"
        );
        assert_eq!(
            source("198.51.100.7:1", Some("203.0.113.1")).await,
            "198.51.100.7",
            "an untrusted peer's header is ignored"
        );
        assert_eq!(source("10.9.0.2:1", Some("203.0.113.1")).await, "-");
        assert_eq!(
            source("10.40.1.2:1", None).await,
            "-",
            "a trusted proxy without X-Real-IP"
        );
    }

    /// Two clients behind one trusted proxy spend separate budgets.
    #[tokio::test]
    async fn two_clients_behind_a_trusted_proxy_have_separate_budgets() {
        use axum::body::Body;
        use axum::routing::get;
        use tower::ServiceExt;

        let inner = Arc::new(Counting::default());
        let v: Arc<dyn Eip1654Validator> = Arc::new(SourceBudgeted::new(inner.clone()));
        let app = scope_client_source(
            axum::Router::new().route(
                "/",
                get(move || {
                    let v = v.clone();
                    async move {
                        match ask(v.as_ref(), 0).await {
                            Ok(_) => axum::http::StatusCode::OK,
                            Err(_) => axum::http::StatusCode::TOO_MANY_REQUESTS,
                        }
                    }
                }),
            ),
            vec!["10.40.0.0/18".parse().unwrap()],
        );
        let call = |real_ip: &'static str| {
            let mut request = axum::http::Request::get("/")
                .header("x-real-ip", real_ip)
                .body(Body::empty())
                .unwrap();
            request
                .extensions_mut()
                .insert(ConnectInfo("10.40.3.4:9".parse::<SocketAddr>().unwrap()));
            app.clone().oneshot(request)
        };
        for _ in 0..PREAUTH_PER_MINUTE {
            assert_eq!(call("203.0.113.50").await.unwrap().status(), 200);
        }
        assert_eq!(call("203.0.113.50").await.unwrap().status(), 429);
        assert_eq!(
            call("203.0.113.51").await.unwrap().status(),
            200,
            "the other client behind the same proxy"
        );
        assert_eq!(inner.calls(), PREAUTH_PER_MINUTE as usize + 1);
    }
}
