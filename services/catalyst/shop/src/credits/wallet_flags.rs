//! Wallet flags over Postgres notifications. The `credits.wallet_flags`
//! channel carries `{action: "flag", addresses, source, ip}` (the
//! events-notifier's `/flag` body) or `{action: "unflag", address}`; one
//! leased listener per fleet applies them to `credits.wallet_flags` through
//! the admin store, audited as the HTTP admin's flag is, where the node's
//! writes are (`OPS_WRITES_ENABLED`). A notification reaches the listeners
//! connected when it commits: one raised while none is up is not applied,
//! and a lost connection is reconnected by the listener, the gap's
//! notifications gone.

use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tokio_util::sync::CancellationToken;

use super::admin::{FlagBody, normalize_address};
use super::errors::ApiError;
use super::store::Credits;
use crate::jobs::Jobs;

pub const CHANNEL: &str = "credits.wallet_flags";
/// The audit actor of a notified flag or unflag.
pub const ACTOR: &str = "pg:credits.wallet_flags";

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
enum Notification {
    Flag(FlagBody),
    Unflag { address: String },
}

/// The `credits-wallet-flags` job; stood down without writes.
pub fn spawn(jobs: &Jobs, credits: Credits, writes_enabled: bool) {
    if !writes_enabled {
        tracing::info!("OPS_WRITES_ENABLED=false: the wallet flags listener stands down");
        return;
    }
    jobs.spawn_leased("credits-wallet-flags", move |ctx| {
        let credits = credits.clone();
        async move {
            let listener = subscribe(&credits.pool).await?;
            run(&credits, listener, ctx.cancel).await
        }
    });
}

/// A connection subscribed to the channel.
pub async fn subscribe(pool: &PgPool) -> sqlx::Result<PgListener> {
    let mut listener = PgListener::connect_with(pool).await?;
    listener.listen(CHANNEL).await?;
    Ok(listener)
}

/// Applies every notification until `cancel`; `Err` when the connection is lost for good.
pub async fn run(
    credits: &Credits,
    mut listener: PgListener,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    tracing::info!(channel = CHANNEL, "wallet flags listener up");
    loop {
        let notification = tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            notification = listener.recv() => notification?,
        };
        if let Err(error) = apply(credits, notification.payload()).await {
            tracing::warn!(?error, "wallet flags: notification not applied");
        }
    }
}

/// One notification's write; a payload the admin would refuse is refused.
pub async fn apply(credits: &Credits, payload: &str) -> Result<(), ApiError> {
    let notification: Notification = serde_json::from_str(payload)
        .map_err(|error| ApiError::bad_request(format!("payload: {error}")))?;
    match notification {
        Notification::Flag(body) => {
            let flag = body.validate()?;
            credits
                .admin_flag_wallets(
                    &flag.addresses,
                    &flag.source,
                    flag.ip.as_deref(),
                    ACTOR,
                    &flag.detail(),
                )
                .await
        }
        Notification::Unflag { address } => {
            let address = normalize_address(&address)?;
            credits
                .admin_unflag_wallet(&address, ACTOR, &json!({ "address": address }))
                .await
                .map(drop)
        }
    }
}
