// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/controllers/handlers/credits-sse.ts, src/logic/credit-notifications.ts (the listener the stream follows)
//! `GET /credits/users/{address}/credits/stream` (canonical
//! `GET /shop-api/v1/users/{address}/credits/stream`): a server-sent event
//! stream of what `GET /users/{address}/credits` answers, for the signer
//! alone. It sends the balance at once, then again whenever it changes, and a
//! `: heartbeat` comment every [`HEARTBEAT`], as credits-server did; the web
//! dapps follow it instead of polling.
//!
//! Changes arrive on the `credits_balance` channel: the database announces a
//! wallet's address whenever its wallet row, the status of its credits'
//! pledges or its flag changes (`credits.notify_balance`, schemas/60_ops.sql),
//! whichever node wrote it. One listener per process ([`BalanceChanges`]),
//! started with the first stream, fans the addresses out. While it is not
//! listening (connecting, or after a lost connection) changes can be missed,
//! so on every (re)connect it tells each stream to read its balance again. A
//! stream that falls behind does the same. A stream sends a balance only when
//! it differs from the last one it sent.

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use serde_json::json;
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tokio::sync::broadcast;

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
/// How often a stream reads its balance again with no change announced: what
/// only the clock changes (a credit or a pledge expiring) reaches it within
/// this, and so does a read that failed.
pub const REFRESH: Duration = Duration::from_secs(60);
/// Changes a slow stream may fall behind by before it reads its balance again.
const BACKLOG: usize = 1024;

/// A change the streams hear about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BalanceChange {
    /// This wallet's balance may have changed.
    Wallet(Arc<str>),
    /// Any balance may have changed while nothing was listening.
    Everyone,
}

/// The process's listener on [`CHANNEL`] and the streams that follow it.
pub struct BalanceChanges {
    sender: broadcast::Sender<BalanceChange>,
    started: AtomicBool,
    refresh: Duration,
}

impl Default for BalanceChanges {
    fn default() -> Self {
        Self::with_refresh(REFRESH)
    }
}

impl BalanceChanges {
    /// Streams that read their balance again every `refresh` with no change.
    pub fn with_refresh(refresh: Duration) -> Self {
        Self {
            sender: broadcast::channel(BACKLOG).0,
            started: AtomicBool::new(false),
            refresh,
        }
    }

    /// A stream's subscription; the first one starts the listener over `pool`.
    pub fn subscribe(&self, pool: &PgPool) -> broadcast::Receiver<BalanceChange> {
        let receiver = self.sender.subscribe();
        if !self.started.swap(true, Ordering::AcqRel) {
            crate::shared::task::spawn(listen(pool.clone(), self.sender.clone()));
        }
        receiver
    }
}

/// Listens on [`CHANNEL`] for as long as the process runs, reconnecting with
/// backoff; after every (re)connect every stream reads its balance again. The
/// listener holds its connection for good, so it gets its own, outside the
/// request pool: it never takes a slot a request needs.
async fn listen(requests: PgPool, sender: broadcast::Sender<BalanceChange>) {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .min_connections(0)
        .connect_lazy_with((*requests.connect_options()).clone());
    let mut backoff = Duration::from_secs(1);
    loop {
        let connected = async {
            let mut listener = PgListener::connect_with(&pool).await?;
            listener.listen(CHANNEL).await?;
            Ok::<_, sqlx::Error>(listener)
        }
        .await;
        let mut listener = match connected {
            Ok(listener) => listener,
            Err(error) => {
                tracing::warn!(%error, channel = CHANNEL, "the balance streams' listener could not connect; retrying");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(30));
                continue;
            }
        };
        backoff = Duration::from_secs(1);
        let _ = sender.send(BalanceChange::Everyone);
        loop {
            match listener.try_recv().await {
                Ok(Some(notification)) => {
                    let _ = sender.send(BalanceChange::Wallet(notification.payload().into()));
                }
                // Reconnected: what was announced meanwhile is lost.
                Ok(None) => {
                    let _ = sender.send(BalanceChange::Everyone);
                }
                Err(error) => {
                    tracing::warn!(%error, channel = CHANNEL, "the balance streams' listener lost its connection; reconnecting");
                    break;
                }
            }
        }
    }
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
    // Subscribed before the first read, so a change between the two is heard;
    // the first read's own flag decides the admission.
    let changes = state.balance_changes.subscribe(&state.credits.pool);
    let refresh = state.balance_changes.refresh;
    let first = user_credits_body(&state, &wallet, None).await?;
    if first.is_blocked_for_claiming == Some(true) {
        return Ok(flagged(&first));
    }
    let first = serde_json::to_string(&first).map_err(|e| ApiError::Internal(e.to_string()))?;
    let follow = Follow {
        state,
        wallet: wallet.into(),
        changes,
        last: None,
        first: Some(first),
        refresh,
        retry: None,
        read_by: tokio::time::Instant::now() + refresh,
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

/// One stream's state: what it last sent, what it hears and when it reads
/// again unprompted.
struct Follow {
    state: SharedState,
    wallet: Arc<str>,
    changes: broadcast::Receiver<BalanceChange>,
    last: Option<String>,
    first: Option<String>,
    refresh: Duration,
    /// The backoff after a failed read, until one succeeds.
    retry: Option<Duration>,
    /// When the stream reads its balance again if nothing is announced: the
    /// refresh, or the retry of a failed read. A deadline, not a timer reset
    /// by every announcement, so other wallets' changes never put it off.
    read_by: tokio::time::Instant,
}

impl Follow {
    /// The next balance to send: the first at once, then whenever a read
    /// (prompted by a change to this wallet, a missed change, the refresh or
    /// the retry of a failed read) differs from the last one sent.
    async fn next(mut self) -> Option<(Result<Event, Infallible>, Self)> {
        if let Some(first) = self.first.take() {
            self.last = Some(first.clone());
            return Some((Ok(Event::default().data(first)), self));
        }
        loop {
            tokio::select! {
                change = self.changes.recv() => match change {
                    Ok(BalanceChange::Wallet(wallet)) if wallet != self.wallet => continue,
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return None,
                },
                () = tokio::time::sleep_until(self.read_by) => {}
            }
            match body_of(&self.state, &self.wallet).await {
                Ok(body) => {
                    self.retry = None;
                    self.read_by = tokio::time::Instant::now() + self.refresh;
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
                    self.read_by = tokio::time::Instant::now() + backoff;
                    tracing::warn!(wallet = %self.wallet, %error, retry_in = ?backoff, "a balance stream could not read the balance; retrying");
                }
            }
        }
    }
}
