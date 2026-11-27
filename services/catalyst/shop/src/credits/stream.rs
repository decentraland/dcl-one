// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/controllers/handlers/credits-sse.ts, src/logic/credit-notifications.ts (the listener the stream follows)
//! `GET /credits/users/{address}/credits/stream` (canonical
//! `GET /shop-api/v1/users/{address}/credits/stream`): a server-sent event
//! stream of what `GET /users/{address}/credits` answers, for the signer
//! alone. It sends the balance at once, then again whenever it changes, and a
//! `: heartbeat` comment every [`HEARTBEAT`], as credits-server did; the web
//! dapps follow it instead of polling.
//!
//! Changes arrive on the `credits_balance` channel: the database announces a
//! wallet's address whenever what its balance answers from changes
//! (`credits.notify_balance`, schemas/60_ops.sql), whichever node wrote it.
//! One listener per process, started with the first stream on a connection
//! of its own, wakes the streams of the announced wallet alone
//! ([`BalanceChanges`]). While it is not listening (connecting, or after a
//! lost connection) changes can be missed, so every (re)connect wakes every
//! stream; an idle connection is pinged, so a half-open one is noticed.
//!
//! The streams share the request pool, so they are bounded: a wallet holds
//! at most [`MAX_STREAMS_PER_WALLET`] and the process [`MAX_STREAMS`] (429
//! past either), at most [`CONCURRENT_READS`] read at once, and their
//! unprompted reads (every [`REFRESH`], or a retry after a failed read) are
//! spread with jitter, so a reconnect or a failover never makes them read in
//! step. A stream sends a balance only when it differs from the last one it
//! sent, and ends when the node shuts down.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use sqlx::postgres::PgListener;
use tokio::sync::{Semaphore, watch};
use tokio_util::sync::CancellationToken;

use super::SharedState;
use super::dto::UserCreditsResponse;
use super::errors::ApiError;
use super::handlers::signer_from;
use super::handlers::users::user_credits_body;

/// The channel `credits.notify_balance` announces a wallet's address on.
pub const CHANNEL: &str = "credits_balance";
/// How often an idle stream sends a `: heartbeat` comment (credits-server's
/// 30 seconds), so proxies keep the connection open.
pub const HEARTBEAT: Duration = Duration::from_secs(30);
/// About how often a stream reads its balance again with no change announced:
/// what only the clock changes (a credit or a pledge expiring) reaches it
/// within this, and so does a read that failed.
pub const REFRESH: Duration = Duration::from_secs(60);
/// Open streams one wallet may hold (as many sockets as the social socket
/// allows an address by default).
pub const MAX_STREAMS_PER_WALLET: usize = 8;
/// Open streams one process may hold.
pub const MAX_STREAMS: usize = 4096;
/// Balance reads the streams run at once: each is a short transaction on the
/// request pool, so they stay well below its size.
pub const CONCURRENT_READS: usize = 4;
/// How long the listener waits for a notification before it pings its
/// connection.
const LISTENER_IDLE: Duration = Duration::from_secs(60);
/// How long the listener's connect and `LISTEN` may take before it gives the
/// connection up.
const LISTENER_REGISTER: Duration = Duration::from_secs(10);
/// Listener connections one process holds at once: the live one and those
/// given up on a stall whose cleanup has not ended. Past it the listener waits
/// for one to close, and the streams fall back on their refresh.
pub(crate) const MAX_LISTENER_CONNECTIONS: usize = 2;

/// The process's listener on [`CHANNEL`] and the streams it wakes.
pub struct BalanceChanges {
    refresh: Duration,
    max_per_wallet: usize,
    max_streams: usize,
    routes: Mutex<Routes>,
    /// Bumped when any balance may have changed unheard.
    everyone: watch::Sender<u64>,
    reads: Arc<Semaphore>,
    listening: AtomicBool,
    shutdown: OnceLock<CancellationToken>,
    /// How long the listener waits for a notification before it pings.
    listener_idle: Duration,
    /// How long the listener's connect and `LISTEN` may take.
    listener_register: Duration,
    /// Where the listener connects; `None` is the request pool's database.
    listener_at: Option<sqlx::postgres::PgConnectOptions>,
}

/// The wallets with open streams.
#[derive(Default)]
struct Routes {
    wallets: HashMap<Arc<str>, Route>,
    open: usize,
}

/// One wallet's streams and the signal that wakes them.
struct Route {
    changed: watch::Sender<u64>,
    streams: usize,
}

/// A stream past a cap.
#[derive(Debug, PartialEq, Eq)]
pub enum Full {
    Wallet,
    Process,
}

impl Default for BalanceChanges {
    fn default() -> Self {
        Self::with_limits(REFRESH, MAX_STREAMS_PER_WALLET, MAX_STREAMS)
    }
}

impl BalanceChanges {
    /// Streams that read their balance again about every `refresh` with no
    /// change, at most `max_per_wallet` for one wallet and `max_streams` in all.
    pub fn with_limits(refresh: Duration, max_per_wallet: usize, max_streams: usize) -> Self {
        Self {
            refresh,
            max_per_wallet,
            max_streams,
            routes: Mutex::default(),
            everyone: watch::channel(0).0,
            reads: Arc::new(Semaphore::new(CONCURRENT_READS)),
            listening: AtomicBool::new(false),
            shutdown: OnceLock::new(),
            listener_idle: LISTENER_IDLE,
            listener_register: LISTENER_REGISTER,
            listener_at: None,
        }
    }

    /// The listener connecting to `at`, pinging after `idle` and giving up a
    /// registration after `register`, for a test that stalls its connection.
    #[cfg(test)]
    pub(crate) fn listening_at(
        mut self,
        at: sqlx::postgres::PgConnectOptions,
        idle: Duration,
        register: Duration,
    ) -> Self {
        self.listener_at = Some(at);
        self.listener_idle = idle;
        self.listener_register = register;
        self
    }

    /// Takes every read permit, for a test that holds the streams' reads.
    #[cfg(test)]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "CONCURRENT_READS is a small constant"
    )]
    pub(crate) fn hold_reads(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.reads
            .clone()
            .try_acquire_many_owned(CONCURRENT_READS as u32)
            .expect("no read in flight")
    }

    /// Streams that read their balance again about every `refresh`.
    pub fn with_refresh(refresh: Duration) -> Self {
        Self::with_limits(refresh, MAX_STREAMS_PER_WALLET, MAX_STREAMS)
    }

    /// The node's shutdown: open streams and the listener end when it is
    /// cancelled, so the server's graceful shutdown never waits on them.
    pub fn end_on(&self, token: CancellationToken) {
        let _ = self.shutdown.set(token);
    }

    fn shutdown(&self) -> CancellationToken {
        self.shutdown.get().cloned().unwrap_or_default()
    }

    /// Takes a slot for one of `wallet`'s streams, or refuses it past a cap:
    /// the signal that wakes the wallet's streams, and the one that wakes all.
    fn register(&self, wallet: &Arc<str>) -> Result<Woken, Full> {
        let mut routes = self.routes.lock().unwrap_or_else(|e| e.into_inner());
        if routes.open >= self.max_streams {
            return Err(Full::Process);
        }
        let route = routes
            .wallets
            .entry(wallet.clone())
            .or_insert_with(|| Route {
                changed: watch::channel(0).0,
                streams: 0,
            });
        if route.streams >= self.max_per_wallet {
            return Err(Full::Wallet);
        }
        route.streams += 1;
        let changed = route.changed.subscribe();
        routes.open += 1;
        Ok(Woken {
            changed,
            everyone: self.everyone.subscribe(),
        })
    }

    /// Gives back one of `wallet`'s slots.
    fn close(&self, wallet: &str) {
        let mut routes = self.routes.lock().unwrap_or_else(|e| e.into_inner());
        routes.open = routes.open.saturating_sub(1);
        if let Some(route) = routes.wallets.get_mut(wallet) {
            route.streams = route.streams.saturating_sub(1);
            if route.streams == 0 {
                routes.wallets.remove(wallet);
            }
        }
    }

    /// Wakes `wallet`'s streams, if it has any.
    fn announce(&self, wallet: &str) {
        let routes = self.routes.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(route) = routes.wallets.get(wallet) {
            route.changed.send_modify(|n| *n = n.wrapping_add(1));
        }
    }

    /// Wakes every stream: changes may have gone unheard.
    fn announce_everyone(&self) {
        self.everyone.send_modify(|n| *n = n.wrapping_add(1));
    }
}

/// What wakes one stream.
struct Woken {
    changed: watch::Receiver<u64>,
    everyone: watch::Receiver<u64>,
}

/// A stream's slot, given back when the stream ends.
struct Slot {
    state: SharedState,
    wallet: Arc<str>,
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.state.balance_changes.close(&self.wallet);
    }
}

/// Starts the process's listener unless it runs.
fn ensure_listening(state: &SharedState) {
    if state.balance_changes.listening.swap(true, Ordering::AcqRel) {
        return;
    }
    let state = state.clone();
    crate::shared::task::spawn(async move {
        let shutdown = state.balance_changes.shutdown();
        tokio::select! {
            () = listen(&state) => {}
            () = shutdown.cancelled() => {}
        }
        // Ended: the next stream starts it again.
        state
            .balance_changes
            .listening
            .store(false, Ordering::Release);
    });
}

/// Listens on [`CHANNEL`] for as long as the process runs, reconnecting with
/// backoff; every (re)connect wakes every stream. The listener holds its
/// connection for good, so it gets its own, outside the request pool: it
/// never takes a slot a request needs. Registering (connect and `LISTEN`) and
/// a ping each have a deadline, and a connection idle for [`LISTENER_IDLE`]
/// is pinged, so a half-open one is replaced. A dropped listener keeps its
/// connection until its cleanup (`UNLISTEN *`) answers, which a stalled socket
/// never does: each attempt connects through a pool of its own, closed once
/// given up so the connection closes when that cleanup ends, and the pools
/// still holding one cap the attempts at [`MAX_LISTENER_CONNECTIONS`].
async fn listen(state: &SharedState) {
    use futures::FutureExt as _;
    let changes = &state.balance_changes;
    let at = changes
        .listener_at
        .clone()
        .unwrap_or_else(|| (*state.credits.pool.connect_options()).clone());
    // How long the server has to answer a ping.
    let answer = changes.listener_idle.min(Duration::from_secs(10));
    let mut closing: Vec<sqlx::PgPool> = Vec::new();
    let mut waiting = false;
    let mut backoff = Duration::from_secs(1);
    loop {
        closing.retain(|pool| pool.size() > 0);
        if closing.len() >= MAX_LISTENER_CONNECTIONS {
            if !std::mem::replace(&mut waiting, true) {
                tracing::warn!(
                    stalled = closing.len(),
                    channel = CHANNEL,
                    "the balance streams' listener waits for a stalled connection to close; the streams refresh on their own meanwhile"
                );
            }
            tokio::time::sleep(jittered(backoff)).await;
            continue;
        }
        waiting = false;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .min_connections(0)
            .connect_lazy_with(at.clone());
        let ended: sqlx::Result<()> = async {
            let registered = async {
                let mut listener = PgListener::connect_with(&pool).await?;
                listener.listen(CHANNEL).await?;
                Ok::<_, sqlx::Error>(listener)
            };
            let mut listener = tokio::time::timeout(changes.listener_register, registered)
                .await
                .map_err(|_| sqlx::Error::PoolTimedOut)??;
            backoff = Duration::from_secs(1);
            changes.announce_everyone();
            loop {
                match tokio::time::timeout(changes.listener_idle, listener.try_recv()).await {
                    Ok(Ok(Some(notification))) => changes.announce(notification.payload()),
                    // Reconnected: what was announced meanwhile is lost.
                    Ok(Ok(None)) => changes.announce_everyone(),
                    Ok(Err(error)) => return Err(error),
                    Err(_) => {
                        let ping = sqlx::query("SELECT 1").execute(&mut listener);
                        tokio::time::timeout(answer, ping)
                            .await
                            .map_err(|_| sqlx::Error::PoolTimedOut)??;
                    }
                }
            }
        }
        .await;
        // `close` marks the pool closed at once; polling it once only starts
        // the wait for the connection, which the cap does instead.
        let _ = pool.close().now_or_never();
        closing.push(pool);
        if let Err(error) = ended {
            tracing::warn!(%error, channel = CHANNEL, "the balance streams' listener lost its connection; reconnecting");
        }
        tokio::time::sleep(jittered(backoff)).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

/// `period` spread by up to a fifth either way, so many streams never act in
/// step.
fn jittered(period: Duration) -> Duration {
    period.mul_f64(rand::random_range(0.8..1.2))
}

/// credits-server's refusal of a flagged wallet's stream, with the balance
/// read's own flag.
fn flagged(body: &UserCreditsResponse) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "ok": false,
            "message": body.message,
            "reason": body.reason,
            "isBlockedForClaiming": true,
        })),
    )
        .into_response()
}

/// `GET /users/{address}/credits/stream`, signed by `address`.
pub async fn credits_stream(
    State(state): State<SharedState>,
    Path(wallet_id): Path<String>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let path = format!("/users/{wallet_id}/credits/stream");
    let signer = signer_from(&state, &headers, "get", &path, &original).await?;
    let wallet = wallet_id.to_ascii_lowercase();
    if crate::shared::identity::Address::parse(&wallet).is_err() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid address" })),
        )
            .into_response());
    }
    if signer.as_str() != wallet {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Address does not match the signed fetch authentication" })),
        )
            .into_response());
    }
    let wallet: Arc<str> = wallet.into();
    // Registered before the first read, so a change between the two is heard;
    // the first read's own flag decides the admission.
    let woken = match state.balance_changes.register(&wallet) {
        Ok(woken) => woken,
        Err(full) => {
            let error = match full {
                Full::Wallet => "Too many open balance streams for this wallet",
                Full::Process => "Too many open balance streams",
            };
            return Ok((
                StatusCode::TOO_MANY_REQUESTS,
                [(axum::http::header::RETRY_AFTER, "30")],
                Json(json!({ "error": error })),
            )
                .into_response());
        }
    };
    let slot = Slot {
        state: state.clone(),
        wallet: wallet.clone(),
    };
    ensure_listening(&state);
    let changes = &state.balance_changes;
    // The first read waits for a permit like every other, so a burst of
    // (re)connections queues here, not in the request pool.
    let shutdown = changes.shutdown();
    let first = tokio::select! {
        () = shutdown.cancelled() => {
            return Ok(StatusCode::SERVICE_UNAVAILABLE.into_response());
        }
        first = async {
            let _permit = changes.reads.acquire().await.map_err(|_| ApiError::Internal("the streams' reads are closed".into()))?;
            user_credits_body(&state, &wallet, None).await
        } => first?,
    };
    if first.is_blocked_for_claiming == Some(true) {
        return Ok(flagged(&first));
    }
    let first = serde_json::to_string(&first).map_err(|e| ApiError::Internal(e.to_string()))?;
    let follow = Follow {
        reader: Arc::new(WalletBalance {
            state: state.clone(),
            wallet,
        }),
        woken,
        reads: changes.reads.clone(),
        shutdown,
        last: None,
        first: Some(first),
        refresh: changes.refresh,
        retry: None,
        read_by: tokio::time::Instant::now() + jittered(changes.refresh),
        _slot: Some(slot),
    };
    let events = futures::stream::unfold(follow, Follow::next);
    Ok((
        // nginx would otherwise buffer the stream.
        [("x-accel-buffering", "no")],
        Sse::new(events).keep_alive(KeepAlive::new().interval(HEARTBEAT).text("heartbeat")),
    )
        .into_response())
}

/// The balance body as the stream sends it.
async fn body_of(state: &SharedState, wallet: &str) -> Result<String, ApiError> {
    let body = user_credits_body(state, wallet, None).await?;
    serde_json::to_string(&body).map_err(|e| ApiError::Internal(e.to_string()))
}

/// What a stream reads and sends: the balance body.
trait ReadBalance: Send + Sync {
    fn read(&self) -> futures::future::BoxFuture<'_, Result<String, ApiError>>;
}

/// A wallet's balance body, from the database.
struct WalletBalance {
    state: SharedState,
    wallet: Arc<str>,
}

impl ReadBalance for WalletBalance {
    fn read(&self) -> futures::future::BoxFuture<'_, Result<String, ApiError>> {
        Box::pin(body_of(&self.state, &self.wallet))
    }
}

/// One stream's state: what it last sent, what wakes it and when it reads
/// again unprompted.
struct Follow {
    reader: Arc<dyn ReadBalance>,
    woken: Woken,
    reads: Arc<Semaphore>,
    shutdown: CancellationToken,
    last: Option<String>,
    first: Option<String>,
    refresh: Duration,
    /// The backoff after a failed read, until one succeeds.
    retry: Option<Duration>,
    /// When the stream reads its balance again if nothing wakes it: the
    /// jittered refresh, or the retry of a failed read. A deadline, not a
    /// timer reset by every wake-up.
    read_by: tokio::time::Instant,
    /// Given back when the stream ends (`None` in a test).
    _slot: Option<Slot>,
}

impl Follow {
    /// The next balance to send: the first at once, then whenever a read
    /// (prompted by a change to this wallet, a missed change, the refresh or
    /// the retry of a failed read) differs from the last one sent. After a
    /// failed read nothing reads before the retry: that read covers whatever
    /// was announced meanwhile. Ends with the node.
    async fn next(mut self) -> Option<(Result<Event, Infallible>, Self)> {
        if let Some(first) = self.first.take() {
            self.last = Some(first.clone());
            return Some((Ok(Event::default().data(first)), self));
        }
        loop {
            let retrying = self.retry.is_some();
            tokio::select! {
                () = self.shutdown.cancelled() => return None,
                woke = self.woken.changed.changed(), if !retrying => {
                    if woke.is_err() {
                        return None;
                    }
                }
                woke = self.woken.everyone.changed(), if !retrying => {
                    if woke.is_err() {
                        return None;
                    }
                }
                () = tokio::time::sleep_until(self.read_by) => {}
            }
            // What this read covers: a change announced from now wakes it again.
            self.woken.changed.borrow_and_update();
            self.woken.everyone.borrow_and_update();
            // The node's shutdown ends the stream while it waits for a permit
            // or for the database too.
            let read = tokio::select! {
                () = self.shutdown.cancelled() => return None,
                read = async {
                    let _permit = self.reads.acquire().await.ok()?;
                    Some(self.reader.read().await)
                } => match read {
                    Some(read) => read,
                    None => return None,
                },
            };
            match read {
                Ok(body) => {
                    self.retry = None;
                    self.read_by = tokio::time::Instant::now() + jittered(self.refresh);
                    if self.last.as_deref() != Some(body.as_str()) {
                        self.last = Some(body.clone());
                        return Some((Ok(Event::default().data(body)), self));
                    }
                }
                Err(error) => {
                    let backoff = self
                        .retry
                        .map_or(Duration::from_secs(1), |b| b * 2)
                        .min(self.refresh);
                    self.retry = Some(backoff);
                    self.read_by = tokio::time::Instant::now() + jittered(backoff);
                    tracing::warn!(%error, retry_in = ?backoff, "a balance stream could not read the balance; retrying");
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    reason = "test fixtures use small literal values"
)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    /// A reader that counts its reads and fails until told otherwise.
    #[derive(Default)]
    struct Counting {
        reads: AtomicUsize,
        recovered: AtomicBool,
    }

    impl ReadBalance for Counting {
        fn read(&self) -> futures::future::BoxFuture<'_, Result<String, ApiError>> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let recovered = self.recovered.load(Ordering::SeqCst);
            Box::pin(async move {
                if recovered {
                    Ok("current".to_string())
                } else {
                    Err(ApiError::Internal("the database is away".into()))
                }
            })
        }
    }

    /// A stream of `wallet` over `changes`, reading with `reader`, whose
    /// first balance was already sent.
    fn follow(changes: &BalanceChanges, wallet: &str, reader: Arc<dyn ReadBalance>) -> Follow {
        let woken = changes.register(&wallet.into()).unwrap();
        Follow {
            reader,
            woken,
            reads: changes.reads.clone(),
            shutdown: changes.shutdown(),
            last: Some("before".into()),
            first: None,
            refresh: changes.refresh,
            retry: None,
            read_by: tokio::time::Instant::now() + Duration::from_secs(3600),
            _slot: None,
        }
    }

    /// After a failed read, wake-ups wait for the retry: one read, not one
    /// per announcement, and the retry sends the current balance with no
    /// further announcement. On a paused clock, so no timing can race it.
    #[tokio::test(start_paused = true)]
    async fn a_failed_read_waits_for_its_retry_whatever_is_announced() {
        let changes = BalanceChanges::with_refresh(Duration::from_secs(60));
        let reader = Arc::new(Counting::default());
        let next = tokio::spawn(follow(&changes, "0xab", reader.clone()).next());
        changes.announce("0xab");
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(
            reader.reads.load(Ordering::SeqCst),
            1,
            "the announcement's read fails"
        );
        for _ in 0..10 {
            changes.announce("0xab");
            changes.announce_everyone();
        }
        // The retry is about a second away (with jitter, at least 0.8 s).
        tokio::time::sleep(Duration::from_millis(700)).await;
        assert_eq!(
            reader.reads.load(Ordering::SeqCst),
            1,
            "nothing reads again before the retry"
        );
        reader.recovered.store(true, Ordering::SeqCst);
        let (_, follow) = tokio::time::timeout(Duration::from_secs(5), next)
            .await
            .expect("the retry sends")
            .unwrap()
            .expect("an event");
        assert_eq!(reader.reads.load(Ordering::SeqCst), 2);
        assert_eq!(follow.last.as_deref(), Some("current"));
        assert!(follow.retry.is_none());
    }

    /// An announcement wakes its own wallet's streams alone; a missed change
    /// wakes them all.
    #[tokio::test(start_paused = true)]
    async fn an_announcement_wakes_its_wallets_streams_alone() {
        let changes = BalanceChanges::with_refresh(Duration::from_secs(60));
        let (ours, theirs) = (Arc::new(Counting::default()), Arc::new(Counting::default()));
        ours.recovered.store(true, Ordering::SeqCst);
        theirs.recovered.store(true, Ordering::SeqCst);
        let mine = tokio::spawn(follow(&changes, "0xab", ours.clone()).next());
        let _other = tokio::spawn(follow(&changes, "0xcd", theirs.clone()).next());
        for _ in 0..100 {
            changes.announce("0xab");
        }
        let _sent = mine.await.unwrap().expect("its balance");
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(ours.reads.load(Ordering::SeqCst) >= 1);
        assert_eq!(
            theirs.reads.load(Ordering::SeqCst),
            0,
            "another wallet's stream sleeps"
        );
        changes.announce_everyone();
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(
            theirs.reads.load(Ordering::SeqCst),
            1,
            "a missed change wakes every stream"
        );
    }

    /// A reader that never answers.
    struct Stuck;

    impl ReadBalance for Stuck {
        fn read(&self) -> futures::future::BoxFuture<'_, Result<String, ApiError>> {
            Box::pin(std::future::pending())
        }
    }

    /// The node's shutdown ends a stream waiting for a read permit, and one
    /// waiting on its read, which gives its permit back.
    #[tokio::test(start_paused = true)]
    async fn the_nodes_shutdown_ends_a_stream_waiting_to_read() {
        let changes = BalanceChanges::with_refresh(Duration::from_secs(60));
        let token = CancellationToken::new();
        changes.end_on(token.clone());
        // Three permits held elsewhere; a stuck read takes the fourth.
        let others = changes
            .reads
            .clone()
            .try_acquire_many_owned(CONCURRENT_READS as u32 - 1)
            .unwrap();
        let reading = tokio::spawn(follow(&changes, "0xcd", Arc::new(Stuck)).next());
        changes.announce("0xcd");
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(
            changes.reads.available_permits(),
            0,
            "the stuck read holds the last permit"
        );
        // This stream now waits for a permit.
        let waiting = tokio::spawn(follow(&changes, "0xab", Arc::new(Counting::default())).next());
        changes.announce("0xab");
        tokio::time::sleep(Duration::from_millis(10)).await;
        token.cancel();
        assert!(
            waiting.await.unwrap().is_none(),
            "the stream waiting for a permit ends"
        );
        assert!(
            reading.await.unwrap().is_none(),
            "the stream waiting on its read ends"
        );
        drop(others);
        assert_eq!(
            changes.reads.available_permits(),
            CONCURRENT_READS,
            "every permit is back"
        );
    }

    /// The node's shutdown ends an open stream.
    #[tokio::test(start_paused = true)]
    async fn the_nodes_shutdown_ends_a_stream() {
        let changes = BalanceChanges::with_refresh(Duration::from_secs(60));
        let token = CancellationToken::new();
        changes.end_on(token.clone());
        let next = tokio::spawn(follow(&changes, "0xab", Arc::new(Counting::default())).next());
        tokio::time::sleep(Duration::from_millis(10)).await;
        token.cancel();
        assert!(next.await.unwrap().is_none(), "the stream ends");
    }

    /// A wallet holds at most its share of streams and the process its total;
    /// a closed stream gives its slot back.
    #[test]
    fn streams_are_capped_per_wallet_and_per_process() {
        let changes = BalanceChanges::with_limits(REFRESH, 2, 3);
        let (a, b, c): (Arc<str>, Arc<str>, Arc<str>) = ("0xa".into(), "0xb".into(), "0xc".into());
        assert!(changes.register(&a).is_ok());
        assert!(changes.register(&a).is_ok());
        assert_eq!(changes.register(&a).err(), Some(Full::Wallet));
        assert!(changes.register(&b).is_ok());
        assert_eq!(changes.register(&c).err(), Some(Full::Process));
        changes.close(&a);
        assert!(
            changes.register(&c).is_ok(),
            "a closed stream's slot is free again"
        );
        assert_eq!(changes.register(&a).err(), Some(Full::Process));
        changes.close(&a);
        changes.close(&a);
        changes.close(&b);
        changes.close(&c);
        let routes = changes.routes.lock().unwrap();
        assert_eq!(
            (routes.open, routes.wallets.len()),
            (0, 0),
            "nothing kept for closed streams"
        );
    }
}
