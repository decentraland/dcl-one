//! The per-wallet storage allowance and its quota blocks, as the old
//! service's `limits-manager`, `wallet-stats` and `blocking`.
//!
//! A DCL NAME world's publish may use what is left of its owner's allowance
//! (`DCL_NAME_STATS_URL/account-holdings/<wallet>` `spaceAllowance`, MiB)
//! after the owner's other DCL worlds, crediting back only the scenes the
//! publish replaces, capped per scene; the publish transaction re-checks it
//! under the wallet's quota lock. A wallet's DCL worlds are the NAMEs it
//! holds now (the stored owner without a names reader). Personal and ENS
//! worlds, and worlds with a size override, have a fixed cap and stay
//! outside the allowance.
//! A wallet whose worlds outgrow its allowance (a NAME sold, holdings
//! dropped) is recorded in `worlds.quota_blocks`: warned at once, its worlds
//! refused on `/about` and comms once [`QUOTA_BLOCK_GRACE`] has passed,
//! and restored when it is back under the allowance.

use std::time::Duration;

use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::Row;

use crate::comms::world_identity::WorldName;
use crate::http::Outbound;
use crate::policy::WorldLimits;
use crate::shared::identity::Address;

use super::CoreState;

#[cfg(test)]
use crate::comms::access::QUOTA_BLOCK_GRACE;

pub const MIB: i64 = 1024 * 1024;
/// WCS `DCL_NAME_STATS_TIMEOUT_MS`' default.
pub const DEFAULT_STATS_TIMEOUT: Duration = Duration::from_millis(3_000);
const STATS_ATTEMPTS: u32 = 3;
const STATS_BASE_DELAY: Duration = Duration::from_millis(300);
/// The quota pass runs as often as the old update-owner job.
pub const QUOTA_PASS_INTERVAL: Duration = Duration::from_secs(12 * 3600);
/// Failures in a row after which a pass stops asking and keeps every block.
const QUOTA_PASS_MAX_CONSECUTIVE_FAILURES: usize = 5;

/// SQL over `w` (`worlds.worlds`): a local DCL NAME or ENS world, the ones
/// its owner's quota block refuses (not personal, not mirrored).
macro_rules! quota_blockable_world {
    () => {
        "(w.mirror_source IS NULL AND w.name !~ '^0x[0-9a-f]{40}\\.dcl\\.eth$' \
          AND (w.name ~ '^[a-z0-9]{1,64}\\.dcl\\.eth$' \
               OR (w.name !~ '\\.dcl\\.eth$' AND w.name ~ '^([^.]+\\.)+eth$')))"
    };
}
pub(crate) use quota_blockable_world;

/// Counted against its owner's allowance: a DCL NAME world, not a personal one.
fn counts_against_allowance(world: &WorldName) -> bool {
    world.is_dcl_world_name() && world.personal_owner().is_none()
}

/// A wallet's storage allowance in bytes.
#[async_trait::async_trait]
pub trait SpaceAllowance: Send + Sync {
    async fn allowance_bytes(&self, wallet: &Address) -> Result<i64, String>;

    /// Whether a source is configured (else every allowance is unlimited).
    fn is_limited(&self) -> bool {
        true
    }
}

/// No allowance source configured: every wallet may use any space.
pub struct Unlimited;

#[async_trait::async_trait]
impl SpaceAllowance for Unlimited {
    async fn allowance_bytes(&self, _: &Address) -> Result<i64, String> {
        Ok(i64::MAX)
    }

    fn is_limited(&self) -> bool {
        false
    }
}

/// How long an answered allowance is reused.
const ALLOWANCE_CACHE_TTL: Duration = Duration::from_secs(60);
/// How long a failed lookup is answered from memory.
const ALLOWANCE_FAILURE_TTL: Duration = Duration::from_secs(15);
const ALLOWANCE_CACHE_MAX: usize = 10_000;
/// Account-holdings requests in flight at once, node-wide.
const ALLOWANCE_MAX_IN_FLIGHT: usize = 8;

type CachedAllowance = (std::time::Instant, Result<i64, String>);

/// The account-holdings service; no URL is an unlimited allowance.
pub struct AccountHoldings {
    http: Outbound,
    url: Option<String>,
    timeout: Duration,
    cache: dashmap::DashMap<Address, CachedAllowance>,
    in_flight: tokio::sync::Semaphore,
}

impl AccountHoldings {
    pub fn new(http: Outbound, url: Option<&str>, timeout: Duration) -> Self {
        Self {
            http,
            url: url
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty()),
            timeout,
            cache: dashmap::DashMap::new(),
            in_flight: tokio::sync::Semaphore::new(ALLOWANCE_MAX_IN_FLIGHT),
        }
    }

    fn fresh(entry: &CachedAllowance) -> bool {
        let ttl = match entry.1 {
            Ok(_) => ALLOWANCE_CACHE_TTL,
            Err(_) => ALLOWANCE_FAILURE_TTL,
        };
        entry.0.elapsed() < ttl
    }

    fn cached(&self, wallet: &Address) -> Option<Result<i64, String>> {
        self.cache
            .get(wallet)
            .filter(|e| Self::fresh(e))
            .map(|e| e.1.clone())
    }

    /// Full of fresh entries, a new answer is not kept (no wholesale clear).
    fn remember(&self, wallet: &Address, answer: Result<i64, String>) {
        if self.cache.len() >= ALLOWANCE_CACHE_MAX && !self.cache.contains_key(wallet) {
            self.cache.retain(|_, e| Self::fresh(e));
            if self.cache.len() >= ALLOWANCE_CACHE_MAX {
                return;
            }
        }
        self.cache
            .insert(wallet.clone(), (std::time::Instant::now(), answer));
    }

    async fn fetch(&self, wallet: &Address, base: &str) -> Result<i64, String> {
        let url = format!("{base}/account-holdings/{}", wallet.as_str());
        let mut attempt = 0;
        loop {
            match self.attempt(&url).await {
                Ok(bytes) => return Ok(bytes),
                Err(e) if attempt + 1 >= STATS_ATTEMPTS => return Err(e),
                Err(e) => {
                    let delay = STATS_BASE_DELAY * 2u32.pow(attempt);
                    tracing::warn!(error = %e, attempt = attempt + 1, ?delay, "account holdings failed, retrying");
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
            }
        }
    }

    async fn attempt(&self, url: &str) -> Result<i64, String> {
        let resp = self
            .http
            .post(url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| format!("account holdings request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!(
                "account holdings answered HTTP {}",
                resp.status().as_u16()
            ));
        }
        let body: Value = resp
            .json()
            .await
            .map_err(|e| format!("account holdings bad body: {e}"))?;
        let mib = body
            .pointer("/data/spaceAllowance")
            .and_then(allowance_mib)
            .ok_or("account holdings answered no integer data.spaceAllowance")?;
        Ok(mib.saturating_mul(MIB))
    }
}

/// `spaceAllowance` as a JSON integer or an integer string (a serialized BigInt).
fn allowance_mib(v: &Value) -> Option<i64> {
    match v {
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => v
            .as_i64()
            .or_else(|| v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64)),
    }
}

#[async_trait::async_trait]
impl SpaceAllowance for AccountHoldings {
    async fn allowance_bytes(&self, wallet: &Address) -> Result<i64, String> {
        let Some(base) = &self.url else {
            return Ok(i64::MAX);
        };
        if let Some(answer) = self.cached(wallet) {
            return answer;
        }
        let _permit = self
            .in_flight
            .acquire()
            .await
            .map_err(|_| "account holdings closed".to_string())?;
        // A caller queued behind the same wallet's lookup reuses its answer.
        if let Some(answer) = self.cached(wallet) {
            return answer;
        }
        let answer = self.fetch(wallet, base).await;
        self.remember(wallet, answer.clone());
        answer
    }

    fn is_limited(&self) -> bool {
        self.url.is_some()
    }
}

/// One of the wallet's worlds and the bytes its local scenes hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldUsage {
    pub name: String,
    pub size: i64,
    /// A `world_limit_overrides` size: the world does not consume allowance.
    pub size_overridden: bool,
}

/// The wallet's local worlds, split as the old wallet stats.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WalletUsage {
    /// DCL NAME worlds, the ones the allowance covers.
    pub dcl_names: Vec<WorldUsage>,
    pub ens_names: Vec<WorldUsage>,
    /// Sum of `dcl_names`.
    pub used_space: i64,
}

impl WalletUsage {
    /// What a quota block compares against the allowance.
    fn charged_space(&self) -> i64 {
        self.used_space
            - self
                .dcl_names
                .iter()
                .filter(|w| w.size_overridden)
                .map(|w| w.size)
                .sum::<i64>()
    }
}

/// `wallet`'s local worlds: the DCL worlds in `held` (the NAMEs it holds
/// now; `None` without a names reader: those stored as its own) and the ENS
/// worlds stored as its own.
pub async fn wallet_usage<'e>(
    db: impl sqlx::PgExecutor<'e>,
    wallet: &Address,
    held: Option<&[String]>,
) -> Result<WalletUsage, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT w.name, w.owner, COALESCE(sum(s.size), 0)::bigint AS size, \
                bool_or(o.max_size_bytes IS NOT NULL) AS overridden \
           FROM worlds.worlds w \
           LEFT JOIN worlds.world_scenes s ON s.world_name = w.name AND s.mirror_source IS NULL \
           LEFT JOIN worlds.world_limit_overrides o ON o.world_name = w.name \
          WHERE (w.owner = $1 OR w.name = ANY($2::text[])) AND w.mirror_source IS NULL \
          GROUP BY w.name, w.owner ORDER BY w.name",
    )
    .bind(wallet)
    .bind(held.unwrap_or_default())
    .fetch_all(db)
    .await?;
    let mut usage = WalletUsage::default();
    for r in rows {
        let name: String = r.get("name");
        let Ok(world) = WorldName::parse(&name) else {
            continue;
        };
        let stored_mine = r.get::<Option<String>, _>("owner").as_deref() == Some(wallet.as_str());
        let mine = match held {
            Some(held) if world.is_dcl_eth() => held.iter().any(|h| h == world.key()),
            _ => stored_mine,
        };
        if !mine {
            continue;
        }
        let entry = WorldUsage {
            name,
            size: r.get("size"),
            size_overridden: r.get::<Option<bool>, _>("overridden").unwrap_or(false),
        };
        if world.is_ens_world() {
            usage.ens_names.push(entry);
        } else if counts_against_allowance(&world) {
            usage.used_space += entry.size;
            usage.dcl_names.push(entry);
        }
    }
    Ok(usage)
}

#[derive(Debug)]
pub enum CapError {
    /// Nobody holds the world's NAME (the old `WorldOwnerNotFoundError`).
    OwnerNotFound,
    /// The allowance or the owner could not be read right now.
    Unavailable(String),
}

impl From<sqlx::Error> for CapError {
    fn from(e: sqlx::Error) -> Self {
        Self::Unavailable(format!("database: {e}"))
    }
}

/// What a publish charged to a wallet's allowance re-checks in its transaction.
#[derive(Debug, Clone)]
pub struct QuotaCharge {
    pub wallet: Address,
    pub allowance: i64,
    /// [`wallet_usage`]'s `held`, the published world included.
    pub held: Option<Vec<String>>,
    pub scene_cap: i64,
}

/// A publish's byte cap and, inside the allowance, what it is charged to.
#[derive(Debug, Clone)]
pub struct SceneCap {
    pub max: i64,
    pub charge: Option<QuotaCharge>,
}

/// The publish refusal for `size` bytes over a `max` cap.
pub fn too_big(max: i64, size: i64) -> String {
    format!(
        "The deployment is too big. The maximum total size allowed is {max} bytes for scenes. You can upload up to {max} bytes but you tried to upload {size}."
    )
}

/// Holds `wallet`'s quota lock until the transaction ends.
async fn lock_wallet(conn: &mut sqlx::PgConnection, wallet: &Address) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(format!("worlds.quota:{}", wallet.as_str()))
        .execute(conn)
        .await?;
    Ok(())
}

/// Re-checks `charge` in the publish transaction that stored `size` bytes,
/// under the wallet's quota lock so concurrent publishes are counted in
/// turn. `Some(max)`: the publish is over the cap.
pub async fn recheck_charge(
    conn: &mut sqlx::PgConnection,
    charge: &QuotaCharge,
    size: i64,
) -> Result<Option<i64>, sqlx::Error> {
    lock_wallet(&mut *conn, &charge.wallet).await?;
    let usage = wallet_usage(&mut *conn, &charge.wallet, charge.held.as_deref()).await?;
    let max = charge
        .allowance
        .saturating_sub(usage.charged_space() - size)
        .clamp(0, charge.scene_cap);
    Ok((size > max).then_some(max))
}

impl CoreState {
    /// The DCL world keys `wallet` holds now; `None` without a names reader.
    pub async fn held_worlds(&self, wallet: &Address) -> Result<Option<Vec<String>>, String> {
        let labels = self
            .policy
            .held_name_labels(wallet)
            .await
            .map_err(|e| e.to_string())?;
        Ok(labels.map(|labels| labels.iter().map(|l| format!("{l}.dcl.eth")).collect()))
    }

    /// The byte cap of a publish to `world` (WCS `getMaxAllowedSizeInBytesFor`):
    /// `limits`' fixed size for override, personal and ENS worlds, else the
    /// owner's remaining allowance capped at `limits.max_size_bytes`. `parcels`
    /// credits back the scenes they overlap; `None` the whole world.
    pub async fn max_scene_size(
        &self,
        world: &WorldName,
        limits: &WorldLimits,
        owner: Option<&Address>,
        parcels: Option<&[String]>,
    ) -> Result<i64, CapError> {
        Ok(self.scene_cap(world, limits, owner, parcels).await?.max)
    }

    /// [`Self::max_scene_size`] with the charge the publish re-checks.
    pub async fn scene_cap(
        &self,
        world: &WorldName,
        limits: &WorldLimits,
        owner: Option<&Address>,
        parcels: Option<&[String]>,
    ) -> Result<SceneCap, CapError> {
        if limits.size_overridden || !counts_against_allowance(world) {
            return Ok(SceneCap {
                max: limits.max_size_bytes,
                charge: None,
            });
        }
        let owner = match owner {
            Some(owner) => owner.clone(),
            None => self
                .policy
                .current_owner(world)
                .await
                .map_err(|e| CapError::Unavailable(e.to_string()))?
                .ok_or(CapError::OwnerNotFound)?,
        };
        let allowance = self
            .allowance
            .allowance_bytes(&owner)
            .await
            .map_err(CapError::Unavailable)?;
        let mut held = self
            .held_worlds(&owner)
            .await
            .map_err(CapError::Unavailable)?;
        // `owner` was resolved as this world's holder.
        if let Some(held) = held.as_mut()
            && !held.iter().any(|h| h == world.key())
        {
            held.push(world.key().to_string());
        }
        let usage = wallet_usage(&self.pool, &owner, held.as_deref()).await?;
        let charged_world = usage.dcl_names.iter().find(|w| w.name == world.key());
        // Only scenes the owner's usage counts are credited.
        let credited: i64 = match (parcels, charged_world) {
            (_, None) => 0,
            (Some(parcels), Some(_)) => {
                sqlx::query_scalar(
                    "SELECT COALESCE(sum(s.size), 0)::bigint FROM worlds.world_scenes s \
                  WHERE s.world_name = $1 AND s.parcels && $2::text[] AND s.mirror_source IS NULL",
                )
                .bind(world.key())
                .bind(parcels)
                .fetch_one(&self.pool)
                .await?
            }
            (None, Some(w)) => w.size,
        };
        let remaining = allowance.saturating_sub(usage.charged_space() - credited);
        Ok(SceneCap {
            max: remaining.clamp(0, limits.max_size_bytes),
            charge: Some(QuotaCharge {
                wallet: owner,
                allowance,
                held,
                scene_cap: limits.max_size_bytes,
            }),
        })
    }

    /// Opens a transaction holding `wallet`'s quota lock, so a block and an
    /// unblock never interleave between evaluation and write.
    async fn quota_locked(
        &self,
        wallet: &Address,
    ) -> Result<sqlx::Transaction<'static, sqlx::Postgres>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        lock_wallet(&mut tx, wallet).await?;
        Ok(tx)
    }

    /// Records (or refreshes) `wallet`'s block when it is over its allowance
    /// and notifies it in the same transaction. `true` when blocked.
    pub async fn block_if_over_quota(
        &self,
        wallet: &Address,
        now: NaiveDateTime,
    ) -> Result<bool, String> {
        let allowance = self.allowance.allowance_bytes(wallet).await?;
        let held = self.held_worlds(wallet).await?;
        let write = async {
            let mut tx = self.quota_locked(wallet).await?;
            let usage = wallet_usage(&mut *tx, wallet, held.as_deref()).await?;
            if allowance >= usage.charged_space() {
                return Ok(None);
            }
            let (created_at, updated_at): (NaiveDateTime, NaiveDateTime) = sqlx::query_as(
                "INSERT INTO worlds.quota_blocks (wallet, created_at, updated_at) VALUES ($1, $2, $2) \
                 ON CONFLICT (wallet) DO UPDATE SET updated_at = EXCLUDED.updated_at \
                 RETURNING created_at, updated_at",
            )
            .bind(wallet)
            .bind(now)
            .fetch_one(&mut *tx)
            .await?;
            let notice = self.notices.quota_block(wallet, created_at, updated_at);
            self.notices
                .send(&mut tx, std::slice::from_ref(&notice))
                .await?;
            tx.commit().await?;
            Ok::<_, sqlx::Error>(Some(created_at))
        };
        let Some(created_at) = write.await.map_err(|e| e.to_string())? else {
            return Ok(false);
        };
        tracing::info!(wallet = %wallet.as_str(), %created_at, "wallet over its storage allowance: quota block recorded");
        Ok(true)
    }

    /// Lifts `wallet`'s block once it is back under its allowance, with the
    /// restored notice in the same transaction. Best effort: a fault keeps
    /// the block. `true` when lifted.
    pub async fn unblock_if_under_quota(&self, wallet: &Address, now: NaiveDateTime) -> bool {
        let result = async {
            let blocked: Option<NaiveDateTime> =
                sqlx::query_scalar("SELECT created_at FROM worlds.quota_blocks WHERE wallet = $1")
                    .bind(wallet)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|e| e.to_string())?;
            if blocked.is_none() {
                return Ok(false);
            }
            let allowance = self.allowance.allowance_bytes(wallet).await?;
            let held = self.held_worlds(wallet).await?;
            let mut tx = self.quota_locked(wallet).await.map_err(|e| e.to_string())?;
            let usage = wallet_usage(&mut *tx, wallet, held.as_deref())
                .await
                .map_err(|e| e.to_string())?;
            if allowance < usage.charged_space() {
                return Ok(false);
            }
            let removed: Option<NaiveDateTime> = sqlx::query_scalar(
                "DELETE FROM worlds.quota_blocks WHERE wallet = $1 RETURNING created_at",
            )
            .bind(wallet)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            let Some(created_at) = removed else {
                return Ok(false);
            };
            let notice = self.notices.quota_restored(wallet, created_at, now);
            self.notices
                .send(&mut tx, std::slice::from_ref(&notice))
                .await
                .map_err(|e| e.to_string())?;
            tx.commit().await.map_err(|e| e.to_string())?;
            Ok::<_, String>(true)
        }
        .await;
        match result {
            Ok(lifted) => {
                if lifted {
                    tracing::info!(wallet = %wallet.as_str(), "wallet back under its storage allowance: quota block lifted");
                }
                lifted
            }
            Err(error) => {
                tracing::warn!(wallet = %wallet.as_str(), %error, "quota recheck failed; block kept");
                false
            }
        }
    }

    /// Lifts the blocks no pass since `run_started_at` refreshed, except
    /// `keep`'s (wallets the pass could not evaluate), notifying each.
    pub async fn collect_stale_quota_blocks(
        &self,
        run_started_at: NaiveDateTime,
        keep: &[Address],
        now: NaiveDateTime,
    ) -> Result<u64, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let rows: Vec<(String, NaiveDateTime)> = sqlx::query_as(
            "DELETE FROM worlds.quota_blocks WHERE updated_at < $1 AND wallet <> ALL($2::text[]) \
             RETURNING wallet, created_at",
        )
        .bind(run_started_at)
        .bind(keep)
        .fetch_all(&mut *tx)
        .await?;
        let notices: Vec<_> = rows
            .iter()
            .filter_map(|(wallet, created_at)| {
                Address::parse(wallet)
                    .ok()
                    .map(|w| self.notices.quota_restored(&w, *created_at, now))
            })
            .collect();
        self.notices.send(&mut tx, &notices).await?;
        tx.commit().await?;
        if !rows.is_empty() {
            tracing::info!(lifted = rows.len(), "stale quota blocks lifted");
        }
        Ok(rows.len() as u64)
    }

    /// The 12-hourly quota pass (the old update-owner job's second step):
    /// every owner of a local DCL world with a scene is blocked when over
    /// its allowance; blocks this pass did not refresh are lifted, except
    /// those of owners it failed to evaluate.
    pub async fn quota_pass(&self) -> anyhow::Result<()> {
        let started = chrono::Utc::now().naive_utc();
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT w.name, w.owner FROM worlds.worlds w \
              WHERE w.owner IS NOT NULL AND w.mirror_source IS NULL \
                AND EXISTS (SELECT 1 FROM worlds.world_scenes s \
                             WHERE s.world_name = w.name AND s.mirror_source IS NULL)",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut owners: Vec<Address> = rows
            .iter()
            .filter(|(name, _)| WorldName::parse(name).is_ok_and(|w| counts_against_allowance(&w)))
            .filter_map(|(_, owner)| Address::parse(owner).ok())
            .collect();
        owners.sort();
        owners.dedup();
        let mut failed = Vec::new();
        let mut consecutive = 0;
        for owner in owners {
            // A source that keeps failing is not asked about every owner.
            if consecutive >= QUOTA_PASS_MAX_CONSECUTIVE_FAILURES {
                failed.push(owner);
                continue;
            }
            match self
                .block_if_over_quota(&owner, chrono::Utc::now().naive_utc())
                .await
            {
                Ok(_) => consecutive = 0,
                Err(error) => {
                    tracing::error!(wallet = %owner.as_str(), %error, "quota pass: wallet not evaluated; its block is kept");
                    consecutive += 1;
                    failed.push(owner);
                }
            }
        }
        if consecutive >= QUOTA_PASS_MAX_CONSECUTIVE_FAILURES {
            tracing::error!(
                skipped = failed.len(),
                "quota pass stopped early on repeated failures; blocks kept"
            );
        }
        self.collect_stale_quota_blocks(started, &failed, chrono::Utc::now().naive_utc())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::collections::HashMap;

    /// Serializes the tests that write `worlds.quota_blocks` or run a pass
    /// (a pass lifts every block it does not refresh, other tests' too).
    pub(crate) static QUOTA_DB: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    /// Allowances by wallet in bytes (`None`: an outage); unknown wallets are unlimited.
    #[derive(Default)]
    pub(crate) struct FixedAllowance(parking_lot::Mutex<HashMap<Address, Option<i64>>>);

    impl FixedAllowance {
        pub(crate) fn set(&self, wallet: &Address, bytes: Option<i64>) {
            self.0.lock().insert(wallet.clone(), bytes);
        }
    }

    #[async_trait::async_trait]
    impl SpaceAllowance for FixedAllowance {
        async fn allowance_bytes(&self, wallet: &Address) -> Result<i64, String> {
            match self.0.lock().get(wallet) {
                None => Ok(i64::MAX),
                Some(Some(bytes)) => Ok(*bytes),
                Some(None) => Err("allowance outage".to_string()),
            }
        }
    }

    #[test]
    fn the_grace_is_the_old_two_days() {
        assert_eq!(QUOTA_BLOCK_GRACE, chrono::TimeDelta::hours(48));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// An account-holdings stub failing its first `failures` requests.
    async fn holdings(failures: usize, body: Value) -> (String, Arc<AtomicUsize>) {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let app = axum::Router::new().route(
            "/account-holdings/{wallet}",
            axum::routing::post(move || {
                let counter = counter.clone();
                let body = body.clone();
                async move {
                    if counter.fetch_add(1, Ordering::SeqCst) < failures {
                        (axum::http::StatusCode::BAD_GATEWAY, axum::Json(Value::Null))
                    } else {
                        (axum::http::StatusCode::OK, axum::Json(body))
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (url, hits)
    }

    fn wallet() -> Address {
        Address::parse("0x1111111111111111111111111111111111111111").unwrap()
    }

    #[tokio::test]
    async fn the_allowance_is_retried_three_times_then_fails() {
        let body = serde_json::json!({ "data": { "spaceAllowance": 5 } });
        let (url, hits) = holdings(2, body.clone()).await;
        let source = AccountHoldings::new(Outbound::for_tests(), Some(&url), DEFAULT_STATS_TIMEOUT);
        assert_eq!(source.allowance_bytes(&wallet()).await, Ok(5 * MIB));
        assert_eq!(hits.load(Ordering::SeqCst), 3);

        let (url, hits) = holdings(3, body).await;
        let source = AccountHoldings::new(Outbound::for_tests(), Some(&url), DEFAULT_STATS_TIMEOUT);
        assert!(source.allowance_bytes(&wallet()).await.is_err());
        assert_eq!(hits.load(Ordering::SeqCst), 3);
        // A failure is answered from memory for a while.
        assert!(source.allowance_bytes(&wallet()).await.is_err());
        assert_eq!(hits.load(Ordering::SeqCst), 3);

        let (url, _) = holdings(0, serde_json::json!({ "data": {} })).await;
        let source = AccountHoldings::new(Outbound::for_tests(), Some(&url), DEFAULT_STATS_TIMEOUT);
        assert!(source.allowance_bytes(&wallet()).await.is_err());

        let unset = AccountHoldings::new(Outbound::for_tests(), None, DEFAULT_STATS_TIMEOUT);
        assert_eq!(unset.allowance_bytes(&wallet()).await, Ok(i64::MAX));
    }

    #[tokio::test]
    async fn a_string_allowance_is_read_and_an_answer_is_reused() {
        let body = serde_json::json!({ "data": { "spaceAllowance": "7" } });
        let (url, hits) = holdings(0, body).await;
        let source = AccountHoldings::new(Outbound::for_tests(), Some(&url), DEFAULT_STATS_TIMEOUT);
        assert_eq!(source.allowance_bytes(&wallet()).await, Ok(7 * MIB));
        assert_eq!(source.allowance_bytes(&wallet()).await, Ok(7 * MIB));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(allowance_mib(&serde_json::json!("1.5")), None);
    }
}
