//! The `friend_first_wear` producer: a profile deployment whose wearables
//! include a marketplace item the wallet never wore before notifies the
//! wallet's active friends, rate limited per recipient by the `ffw_rules`
//! arms. One pool now: profiles come from `content.deployments`, friends
//! from `social.friendships`; the item's name/creator/image/buyability come
//! from the node's Shop catalogue through an [`ItemSource`] `serve` wires
//! (the old crate read the squid mirror, which v2 does not run). The telemetry store
//! that could override the arms and the funnel events are not carried.

use crate::shared::now_utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDateTime;
use serde_json::{Value as Json, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::store::{NewNotification, insert_notifications, ms_to_naive, naive_to_ms};
use crate::badges::jobs::{IdSource, Keyed, Pass, ProcessorSettings, read_ids};
use crate::chain_index::graph::Network;
use crate::identity::Address;

pub const POLL_SECS: u64 = 60;
pub const FIRST_WEAR_TYPE: &str = "friend_first_wear";

pub const EXPERIMENT_KEY: &str = "ffw_rules";
pub const RATE_WINDOW_MS: i64 = 3_600_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FfwRule {
    Off,
    Limit1h,
    OnlineBypass,
    Unlimited,
}

impl FfwRule {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Limit1h => "limit_1h",
            Self::OnlineBypass => "online_bypass",
            Self::Unlimited => "unlimited",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "off" => Self::Off,
            "limit_1h" => Self::Limit1h,
            "online_bypass" => Self::OnlineBypass,
            "unlimited" => Self::Unlimited,
            _ => return None,
        })
    }
}

pub const DEFAULT_ARMS: &[(FfwRule, u32)] = &[
    (FfwRule::Off, 10),
    (FfwRule::Limit1h, 20),
    (FfwRule::OnlineBypass, 60),
    (FfwRule::Unlimited, 10),
];

#[derive(Clone, Debug, Default)]
pub struct FfwExperiment {
    pub killed: bool,
    pub forced: Option<FfwRule>,
    pub weights: Vec<(FfwRule, u32)>,
}

impl FfwExperiment {
    pub fn arms(&self) -> &[(FfwRule, u32)] {
        if self.weights.is_empty() {
            DEFAULT_ARMS
        } else {
            &self.weights
        }
    }
    pub fn rule_for(&self, address: &str) -> FfwRule {
        if let Some(f) = self.forced {
            return f;
        }
        bucket_rule(address, self.arms())
    }
}

pub fn cyrb53(s: &str) -> u64 {
    let mut h1: u32 = 0xdead_beef;
    let mut h2: u32 = 0x41c6_ce57;
    for ch in s.encode_utf16() {
        h1 = (h1 ^ u32::from(ch)).wrapping_mul(2_654_435_761);
        h2 = (h2 ^ u32::from(ch)).wrapping_mul(1_597_334_677);
    }
    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2_246_822_507);
    h1 ^= (h2 ^ (h2 >> 13)).wrapping_mul(3_266_489_909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2_246_822_507);
    h2 ^= (h1 ^ (h1 >> 13)).wrapping_mul(3_266_489_909);
    4_294_967_296u64 * u64::from(h2 & 2_097_151) + u64::from(h1)
}

fn bucket_rule(address: &str, arms: &[(FfwRule, u32)]) -> FfwRule {
    if arms.is_empty() {
        return FfwRule::OnlineBypass;
    }
    if arms.len() == 1 {
        return arms[0].0;
    }
    let total: u64 = arms.iter().map(|(_, w)| u64::from(*w)).sum();
    let total = if total == 0 { arms.len() as u64 } else { total };
    let unit = cyrb53(&format!("{}:{}", address, EXPERIMENT_KEY)) as f64 / 9_007_199_254_740_992f64;
    let target = unit * total as f64;
    let mut acc = 0f64;
    for (rule, w) in arms {
        let w = if *w > 0 { f64::from(*w) } else { 1.0 };
        acc += w;
        if target < acc {
            return *rule;
        }
    }
    arms[arms.len() - 1].0
}

pub fn rate_allows(
    rule: FfwRule,
    last_emit_ms: Option<i64>,
    last_fetch_ms: Option<i64>,
    now_ms: i64,
) -> bool {
    match rule {
        FfwRule::Off => false,
        FfwRule::Unlimited => true,
        FfwRule::Limit1h => last_emit_ms.is_none_or(|t| now_ms - t >= RATE_WINDOW_MS),
        FfwRule::OnlineBypass => last_emit_ms
            .is_none_or(|t| now_ms - t >= RATE_WINDOW_MS || last_fetch_ms.is_some_and(|f| f > t)),
    }
}

fn normalize_urn(raw: &str) -> String {
    let parts: Vec<&str> = raw.split(':').collect();
    let take = parts.len().min(6);
    parts[..take].join(":").to_lowercase()
}

fn is_market_urn(urn: &str) -> bool {
    urn.contains(":collections-")
}

fn profile_wearables(metadata: &Json) -> Vec<String> {
    let avatar = &metadata["v"]["avatars"][0];
    let list = avatar["avatar"]["wearables"].as_array();
    let mut out: Vec<String> = list
        .map(|ws| {
            ws.iter()
                .filter_map(|w| w.as_str())
                .map(normalize_urn)
                .filter(|u| is_market_urn(u))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out.dedup();
    out
}

fn profile_name(metadata: &Json) -> Option<String> {
    // Deployers store the metadata under `v`; older rows are bare.
    metadata.get("v").unwrap_or(metadata)["avatars"][0]["name"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// `FIRST_WEAR_IMAGE_BASE`: repoints a `/lambdas/...` image at this node.
fn repoint_image(image: Option<String>, base: Option<&str>) -> Option<String> {
    let img = image?;
    match (img.find("/lambdas/"), base) {
        (Some(ix), Some(base)) => Some(format!("{}{}", base, &img[ix..])),
        _ => Some(img),
    }
}

fn short_address(addr: &str) -> String {
    if addr.len() > 12 {
        format!("{}...{}", &addr[..6], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}

#[derive(Clone, Debug)]
pub struct FirstWearSettings {
    /// `SHOP_ITEM_BASE_URL`: the marketplace item link base.
    pub shop_item_base_url: String,
    /// `FIRST_WEAR_IMAGE_BASE`: optional catalyst base the item image is repointed at.
    pub image_base: Option<String>,
    /// `BADGES_PROCESSOR_BATCH_SIZE`: deployments per transaction, as for
    /// the badge processor's id cursors.
    pub batch_size: i64,
}

impl Default for FirstWearSettings {
    fn default() -> Self {
        Self {
            shop_item_base_url: "https://decentraland.org/marketplace/".to_string(),
            image_base: None,
            batch_size: ProcessorSettings::default().batch_size,
        }
    }
}

/// The item a first wear names, when it can be bought now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuyableItem {
    /// `<contract>-<item id>`, the marketplace item page's id.
    pub item_id: String,
    pub name: String,
    /// Lowercase.
    pub creator: String,
    pub image: Option<String>,
}

/// Why an [`ItemSource`] gave no answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemLookupError {
    /// Asking again cannot help (a request the source refuses): the wear is
    /// passed without a notice.
    Invalid(String),
    /// The source cannot answer now (not ready, a fault): the wear is
    /// retried with backoff.
    Transient(String),
}

/// Where the producer looks up a worn item: the node's Shop catalogue, read
/// as its `/v1/items` and `/v1/orders` routes read it. `Ok(None)`: unknown,
/// or nothing to buy.
#[async_trait::async_trait]
pub trait ItemSource: Send + Sync {
    async fn buyable_item(&self, urn: &str) -> Result<Option<BuyableItem>, ItemLookupError>;
}

/// The producer's row of `social.badge_cursors` (and its gaps in
/// `social.badge_cursor_gaps`): the last `content.deployments.id` passed.
pub const CURSOR_SOURCE: &str = "friend_first_wear";

/// A profile the node stored more than this after its signed timestamp is
/// history a syncing node caught up on, not an outfit worn now: passed
/// without effect. Measured at storage, not at the pass, so a producer that
/// stalls still works through its backlog.
const MAX_STORAGE_LAG_HOURS: i64 = 24;

/// Batches one pass drains at most before yielding. A batch is
/// [`FirstWearSettings::batch_size`] deployments (`BADGES_PROCESSOR_BATCH_SIZE`,
/// as for the badge processor's id cursors), with the id cursor's gap grace.
const MAX_BATCHES_PER_PASS: usize = 50;
const INTAKE_HIGH_WATER_MARK: i64 = 5_000;
const OUTBOX_PAGE_SIZE: i64 = 16;
const DRAIN_BUDGET: Duration = Duration::from_secs(60);
const NETWORK_FAILURE_LIMIT: u8 = 2;
const ITEM_LOOKUP_BUDGET: Duration = Duration::from_secs(20);
const RECIPIENT_LOCK: &str =
    "SELECT pg_advisory_xact_lock(hashtextextended('social.first_wear_recipient_gate', 0))";

/// Every deployment by `id`, the profile metadata only: reading only the
/// profiles would leave every other deployment's id as a gap to re-read.
const READ: IdSource = IdSource {
    name: CURSOR_SOURCE,
    select: "SELECT d.id, d.entity_type, d.entity_pointers[1] AS address, d.entity_timestamp, \
                    d.local_timestamp, \
                    CASE WHEN d.entity_type = 'profile' THEN d.entity_metadata::text END AS meta \
             FROM content.deployments d",
    key: "d.id",
    head: "SELECT coalesce(max(id), 0)::bigint FROM content.deployments",
};

#[derive(sqlx::FromRow)]
struct Deployment {
    id: i32,
    entity_type: String,
    address: Option<String>,
    entity_timestamp: NaiveDateTime,
    local_timestamp: NaiveDateTime,
    meta: Option<String>,
}

impl Keyed for Deployment {
    fn key(&self) -> i64 {
        i64::from(self.id)
    }
}

/// A durable first-wear obligation, independent of source deployment retention.
#[derive(sqlx::FromRow)]
struct FirstWear {
    attempts: i32,
    address: Address,
    wearer: Option<String>,
    urn: String,
}

/// One pass: the profile deployments past the cursor, in `id` order (the
/// order the node stored them, whatever their signed timestamps), batch by
/// batch. Returns the notifications emitted. Without an item source
/// nothing can be resolved as buyable, so the pass only seeds baselines and
/// claims first wears.
pub async fn run_once(
    pool: &PgPool,
    items: Option<&dyn ItemSource>,
    settings: &FirstWearSettings,
) -> anyhow::Result<u64> {
    let pass = Pass {
        pool: pool.clone(),
        asset_base: String::new(),
        settings: ProcessorSettings {
            batch_size: settings.batch_size,
            ..ProcessorSettings::default()
        },
        pinned_xmin: None,
    };
    // A cursor or source-table failure must not strand already committed work.
    let intake = async {
        for _ in 0..MAX_BATCHES_PER_PASS {
            let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM social.first_wear_outbox")
                .fetch_one(pool)
                .await?;
            if pending >= INTAKE_HIGH_WATER_MARK {
                tracing::warn!(
                    pending,
                    "first wear intake paused at the queue high-water mark"
                );
                break;
            }
            if !claim_batch(&pass).await? {
                break;
            }
        }
        anyhow::Ok(())
    }
    .await;
    let emitted = drain_outbox(pool, items, settings).await?;
    intake?;
    crate::badges::jobs::report_id_lag(pool, &READ).await;
    Ok(emitted)
}

async fn drain_outbox(
    pool: &PgPool,
    items: Option<&dyn ItemSource>,
    settings: &FirstWearSettings,
) -> anyhow::Result<u64> {
    let experiment = FfwExperiment::default();
    let expired = sqlx::query(
        "DELETE FROM social.first_wear_outbox q USING social.worn_history h
        WHERE (h.address, h.urn) = (q.address, q.urn)
          AND least(h.first_seen, q.created_at) <= timezone('UTC', now()) - interval '24 hours'",
    )
    .execute(pool)
    .await?
    .rows_affected();
    if expired > 0 {
        tracing::warn!(expired, "expired first wear obligations");
    }
    let cutoff: NaiveDateTime = sqlx::query_scalar("SELECT timezone('UTC', clock_timestamp())")
        .fetch_one(pool)
        .await?;
    let deadline = tokio::time::Instant::now() + DRAIN_BUDGET;
    let mut emitted = 0;
    let mut failures: HashMap<Network, u8> = HashMap::new();
    let mut paused_segments: Vec<&str> = Vec::new();
    for _ in 0..MAX_BATCHES_PER_PASS {
        let pending: Vec<FirstWear> = sqlx::query_as(
            "WITH due AS (SELECT address, urn FROM social.first_wear_outbox
                WHERE retry_at <= $1 AND split_part(urn, ':', 3) <> ALL($3::text[])
                ORDER BY retry_at, address, urn LIMIT $2 FOR UPDATE SKIP LOCKED)
             UPDATE social.first_wear_outbox q SET retry_at = timezone('UTC', clock_timestamp()) + interval '2 minutes'
             FROM due WHERE (q.address, q.urn) = (due.address, due.urn)
             RETURNING q.address, q.urn, q.wearer, q.attempts"
        ).bind(cutoff).bind(OUTBOX_PAGE_SIZE).bind(&paused_segments).fetch_all(pool).await?;
        if pending.is_empty() {
            break;
        }
        for wear in pending {
            if tokio::time::Instant::now() >= deadline {
                return Ok(emitted);
            }
            let network = Network::of_urn(&wear.urn);
            if network.is_some_and(|n| failures.get(&n) == Some(&NETWORK_FAILURE_LIMIT)) {
                continue;
            }
            let (count, transient_failure) =
                notify(pool, items, settings, &experiment, &wear).await?;
            emitted += count;
            if let Some(network) = network {
                if transient_failure {
                    let streak = failures.entry(network).or_default();
                    *streak += 1;
                    if *streak == NETWORK_FAILURE_LIMIT {
                        paused_segments.extend_from_slice(match network {
                            Network::Ethereum => &["ethereum", "mainnet"],
                            Network::Matic => &["matic", "polygon"],
                        });
                        tracing::warn!(
                            network = network.db_name(),
                            "pausing first wear lookups on this network for the pass"
                        );
                    }
                } else {
                    failures.remove(&network);
                }
            }
        }
    }
    Ok(emitted)
}

/// One transaction: reads a batch past the cursor, seeds the baselines and
/// claims and enqueues the new URNs of its fresh profiles, and advances the
/// cursor. A database error rolls all of it back and the next pass reads the same
/// rows. The first pass records the head of `content.deployments` and replays
/// no history. Returns whether source rows remain.
async fn claim_batch(pass: &Pass) -> anyhow::Result<bool> {
    let mut tx = pass.pool.begin().await?;
    let Some(read) = read_ids::<Deployment>(&mut tx, pass, &READ).await? else {
        tx.commit().await?;
        tracing::info!("friend_first_wear cursor initialized (no history replay)");
        return Ok(false);
    };
    if read.is_idle() {
        tx.rollback().await?;
        return Ok(false);
    }
    let max_lag = chrono::Duration::hours(MAX_STORAGE_LAG_HOURS);
    for row in &read.rows {
        if row.entity_type != "profile" || row.local_timestamp - row.entity_timestamp > max_lag {
            continue;
        }
        let entity_ts = row.entity_timestamp;
        let Ok(address) =
            Address::parse(&row.address.as_deref().unwrap_or_default().to_lowercase())
        else {
            continue;
        };
        let Some(meta) = row
            .meta
            .as_deref()
            .and_then(|m| serde_json::from_str::<Json>(m).ok())
        else {
            continue;
        };
        let worn_now = profile_wearables(&meta);

        let has_baseline: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM social.first_wear_baseline WHERE address = $1)",
        )
        .bind(&address)
        .fetch_one(&mut *tx)
        .await?;

        if !has_baseline {
            let history = full_history_wearables(&mut tx, &address).await?;
            let all: Vec<String> = history.into_iter().chain(worn_now).collect();
            seed_history(&mut tx, &address, &all, entity_ts).await?;
            sqlx::query("INSERT INTO social.first_wear_baseline (address, seeded_at) VALUES ($1, $2) ON CONFLICT (address) DO NOTHING")
                .bind(&address)
                .bind(entity_ts)
                .execute(&mut *tx)
                .await?;
            continue;
        }
        if worn_now.is_empty() {
            continue;
        }

        let baseline: Vec<String> =
            sqlx::query_scalar("SELECT urn FROM social.worn_history WHERE address = $1")
                .bind(&address)
                .fetch_all(&mut *tx)
                .await?;

        for urn in worn_now {
            if baseline.contains(&urn) {
                continue;
            }
            let claimed = sqlx::query(
                "INSERT INTO social.worn_history (address, urn, first_seen) VALUES ($1, $2, $3)
                 ON CONFLICT (address, urn) DO NOTHING",
            )
            .bind(&address)
            .bind(&urn)
            .bind(entity_ts)
            .execute(&mut *tx)
            .await?;
            if claimed.rows_affected() == 0 {
                continue;
            }
            sqlx::query(
                "INSERT INTO social.first_wear_outbox (address, urn, wearer) VALUES ($1, $2, $3)",
            )
            .bind(&address)
            .bind(&urn)
            .bind(profile_name(&meta))
            .execute(&mut *tx)
            .await?;
        }
    }
    read.finish(&mut tx).await?;
    tx.commit().await?;
    Ok(read.more)
}

/// Notifies the wearer's active friends of one claimed first wear, when its
/// item resolves as buyable. The obligation stays queued through preparation;
/// notification insertion and queue completion commit together.
async fn notify(
    pool: &PgPool,
    items: Option<&dyn ItemSource>,
    settings: &FirstWearSettings,
    experiment: &FfwExperiment,
    wear: &FirstWear,
) -> anyhow::Result<(u64, bool)> {
    let (address, urn) = (&wear.address, wear.urn.as_str());
    let Some(items) = items else {
        tracing::debug!(urn, "first wear not notifiable (no item source configured)");
        return Ok((complete_wear(pool, wear, &[]).await?, false));
    };
    let result = tokio::time::timeout(ITEM_LOOKUP_BUDGET, buyable_item(items, urn))
        .await
        .unwrap_or_else(|_| {
            Err(ItemLookupError::Transient(
                "first wear lookup timed out".into(),
            ))
        });
    let item = match result {
        Ok(item) => item,
        Err(ItemLookupError::Invalid(error)) => {
            tracing::warn!(urn, %error, "invalid first wear lookup");
            return Ok((complete_wear(pool, wear, &[]).await?, false));
        }
        Err(ItemLookupError::Transient(error)) => {
            let attempts = wear.attempts + 1;
            if attempts >= super::delivery::MAX_ATTEMPTS {
                tracing::warn!(urn, attempts, %error, "first wear lookup retries exhausted");
                return Ok((complete_wear(pool, wear, &[]).await?, true));
            }
            let delay = super::delivery::backoff(attempts).as_secs() as f64;
            sqlx::query(
                "UPDATE social.first_wear_outbox SET attempts = $3,
                retry_at = timezone('UTC', clock_timestamp()) + make_interval(secs => $4)
                WHERE address = $1 AND urn = $2 AND attempts = $3 - 1",
            )
            .bind(address)
            .bind(urn)
            .bind(attempts)
            .bind(delay)
            .execute(pool)
            .await?;
            tracing::warn!(urn, attempts, %error, "first wear lookup scheduled for retry");
            return Ok((0, true));
        }
    };
    let Some(item) = item else {
        tracing::debug!(
            urn,
            "first wear not notifiable (item unknown or not buyable)"
        );
        return Ok((complete_wear(pool, wear, &[]).await?, false));
    };
    let friends = active_friends(pool, address).await?;
    if friends.is_empty() {
        return Ok((complete_wear(pool, wear, &[]).await?, false));
    }

    let wearer = wear
        .wearer
        .clone()
        .unwrap_or_else(|| short_address(address.as_str()));
    let creator_name = creator_profile_name(pool, &item.creator)
        .await?
        .unwrap_or_else(|| short_address(&item.creator));
    let image = repoint_image(item.image.clone(), settings.image_base.as_deref());
    let now_ms = chrono::Utc::now().timestamp_millis();

    let mut batch = Vec::new();
    for friend in &friends {
        let rule = experiment.rule_for(friend.as_str());
        if experiment.killed || rule == FfwRule::Off {
            continue;
        }
        let nid = Uuid::new_v4();
        let link = format!(
            "{}{}?src=ffw&nid={}",
            settings.shop_item_base_url, item.item_id, nid
        );
        let metadata = json!({
            "title": format!("{} is wearing something new", wearer),
            "description": format!(
                "Your friend {} is now wearing {} by {} for the first time!",
                wearer, item.name, creator_name
            ),
            "link": link,
            "image": image,
            "friendAddress": address.as_str(),
            "friendName": wearer,
            "wearableUrn": urn,
            "wearableName": item.name,
            "creatorName": creator_name,
            "itemId": item.item_id,
            "arm": rule.as_str(),
        });
        batch.push(NewNotification {
            id: nid,
            address: friend.clone(),
            kind: FIRST_WEAR_TYPE.to_string(),
            metadata,
            occurred_at: ms_to_naive(now_ms).unwrap_or_else(now_utc),
        });
    }
    Ok((complete_wear(pool, wear, &batch).await?, false))
}

#[cfg(test)]
async fn pending_wears(pool: &PgPool, limit: i64) -> sqlx::Result<Vec<FirstWear>> {
    sqlx::query_as("SELECT attempts, address, wearer, urn FROM social.first_wear_outbox ORDER BY retry_at, address, urn LIMIT $1")
        .bind(limit).fetch_all(pool).await
}

// Short completion transactions serialize recipient gates, never external IO.
async fn complete_wear(
    pool: &PgPool,
    wear: &FirstWear,
    batch: &[NewNotification],
) -> anyhow::Result<u64> {
    let mut tx = pool.begin().await?;
    if !batch.is_empty() {
        sqlx::query(RECIPIENT_LOCK).execute(&mut *tx).await?;
    }
    let completed =
        sqlx::query("DELETE FROM social.first_wear_outbox WHERE address = $1 AND urn = $2")
            .bind(&wear.address)
            .bind(&wear.urn)
            .execute(&mut *tx)
            .await?;
    let emitted = if completed.rows_affected() == 1 && !batch.is_empty() {
        let friends: Vec<Address> = batch.iter().map(|notice| notice.address.clone()).collect();
        let gates = friend_gates(&mut *tx, &friends).await?;
        let filtered: Vec<NewNotification> = batch
            .iter()
            .filter(|notice| {
                let rule = notice.metadata["arm"]
                    .as_str()
                    .and_then(FfwRule::parse)
                    .unwrap_or(FfwRule::Unlimited);
                let (last_emit, last_fetch) = gates
                    .get(notice.address.as_str())
                    .copied()
                    .unwrap_or((None, None));
                rate_allows(
                    rule,
                    last_emit.map(naive_to_ms),
                    last_fetch.map(naive_to_ms),
                    naive_to_ms(notice.occurred_at),
                )
            })
            .cloned()
            .collect();
        insert_notifications(&mut *tx, &filtered).await?
    } else {
        0
    };
    tx.commit().await?;
    Ok(emitted)
}

async fn full_history_wearables(
    conn: &mut PgConnection,
    address: &Address,
) -> anyhow::Result<Vec<String>> {
    let rows = sqlx::query_scalar::<_, Option<String>>(
        "SELECT entity_metadata::text FROM content.deployments
         WHERE entity_type = 'profile' AND entity_pointers @> ARRAY[$1::text]",
    )
    .bind(address.as_str())
    .fetch_all(conn)
    .await?;
    let mut urns = Vec::new();
    for raw in rows.into_iter().flatten() {
        if let Ok(meta) = serde_json::from_str::<Json>(&raw) {
            urns.extend(profile_wearables(&meta));
        }
    }
    urns.sort();
    urns.dedup();
    Ok(urns)
}

async fn seed_history(
    conn: &mut PgConnection,
    address: &Address,
    urns: &[String],
    ts: chrono::NaiveDateTime,
) -> anyhow::Result<()> {
    if urns.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO social.worn_history (address, urn, first_seen)
         SELECT $1, u, $3 FROM unnest($2::text[]) AS t(u)
         ON CONFLICT (address, urn) DO NOTHING",
    )
    .bind(address)
    .bind(urns)
    .bind(ts)
    .execute(conn)
    .await?;
    Ok(())
}

/// The item behind a normalized URN, if its catalogue knows it and it can
/// be bought. Only collection items (`collections-v1`, `collections-v2`) on
/// the two networks are in a catalogue: a third-party or off-chain URN is
/// never looked up, so it spends no retries and pauses no network.
async fn buyable_item(
    items: &dyn ItemSource,
    urn: &str,
) -> Result<Option<BuyableItem>, ItemLookupError> {
    let collection_item = matches!(
        urn.split(':').nth(3),
        Some("collections-v1" | "collections-v2")
    );
    if Network::of_urn(urn).is_none() || !collection_item {
        return Ok(None);
    }
    items.buyable_item(urn).await
}

async fn active_friends(pool: &PgPool, address: &Address) -> anyhow::Result<Vec<Address>> {
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT CASE WHEN address_requester = $1 THEN address_requested ELSE address_requester END
         FROM social.friendships
         WHERE is_active AND (address_requester = $1 OR address_requested = $1)",
    )
    .bind(address)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .filter_map(|f| Address::parse(f).ok())
        .filter(|f| f != address)
        .collect())
}

/// The two per-recipient gates of one first wear, read for every friend of
/// the wearer in one statement: the last `friend_first_wear` emitted to the
/// friend (the 1 h rate limit) and the friend's last authenticated list
/// fetch (the online bypass). A wearer can have 10^3..10^4 friends,
/// so two probes per friend were 2 x degree round trips per
/// first wear. A friend with neither row is absent from the map.
async fn friend_gates<'e, E>(
    executor: E,
    friends: &[Address],
) -> anyhow::Result<HashMap<String, (Option<NaiveDateTime>, Option<NaiveDateTime>)>>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let addresses: Vec<String> = friends.iter().map(|f| f.as_str().to_string()).collect();
    let rows: Vec<(String, Option<NaiveDateTime>, Option<NaiveDateTime>)> = sqlx::query_as(
        "SELECT f.address, e.last_emit, s.last_fetch_at \
         FROM unnest($1::text[]) AS f(address) \
         LEFT JOIN LATERAL (SELECT max(occurred_at) AS last_emit FROM social.notifications WHERE address = f.address AND type = $2) e ON true \
         LEFT JOIN social.notification_reader_seen s ON s.address = f.address \
         WHERE e.last_emit IS NOT NULL OR s.last_fetch_at IS NOT NULL",
    )
    .bind(&addresses)
    .bind(FIRST_WEAR_TYPE)
    .fetch_all(executor)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(address, last_emit, last_fetch)| (address, (last_emit, last_fetch)))
        .collect())
}

/// `avatars[0].name` of `address`'s profile as `/lambdas/profiles` serves it:
/// the entity active on the pointer, unless denylisted; the name as deployed
/// (lambdas only recomputes `hasClaimedName`, never the name).
pub(super) async fn creator_profile_name(
    pool: &PgPool,
    address: &str,
) -> anyhow::Result<Option<String>> {
    if address.is_empty() {
        return Ok(None);
    }
    let raw: Option<Option<String>> = sqlx::query_scalar(
        "SELECT d.entity_metadata::text FROM content.active_pointers ap
         JOIN content.deployments d ON d.entity_id = ap.entity_id
         WHERE ap.pointer = $1 AND d.entity_type = 'profile' AND d.deleter_deployment IS NULL
           AND NOT EXISTS (SELECT 1 FROM content.denylist l WHERE l.id = d.entity_id)
         LIMIT 1",
    )
    .bind(address.to_lowercase())
    .fetch_optional(pool)
    .await?;
    Ok(raw
        .flatten()
        .and_then(|r| serde_json::from_str::<Json>(&r).ok())
        .and_then(|m| profile_name(&m)))
}

pub fn spawn(state: &Arc<super::State>, jobs: &crate::jobs::Jobs) {
    let state = state.clone();
    jobs.spawn_periodic_leased(
        "friend-first-wear",
        crate::jobs::PeriodicCfg::every(std::time::Duration::from_secs(POLL_SECS))
            .with_jitter(std::time::Duration::from_secs(10))
            .with_error_backoff(std::time::Duration::from_secs(900)),
        move |ctx| {
            let state = state.clone();
            async move {
                let emitted = tokio::select! {
                    _ = ctx.cancel.cancelled() => return Ok(()),
                    result = run_once(
                    state.store.pool(),
                    state.items.get().map(AsRef::as_ref),
                    &state.settings.first_wear,
                ) => result?,
                };
                if emitted > 0 {
                    tracing::info!(emitted, "friend_first_wear notifications emitted");
                }
                Ok(())
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyrb53_matches_the_js_reference() {
        assert_eq!(cyrb53("a"), 7929297801672961);
        assert_eq!(cyrb53("hello"), 4625896200565286);
        assert_eq!(
            cyrb53("0x229d59c24f50698600117ed1ae40aa0ddf6e2c69:ffw_rules"),
            2734530341693953
        );
    }

    #[test]
    fn bucketing_is_deterministic_and_respects_forced() {
        let exp = FfwExperiment::default();
        let a = exp.rule_for("0x229d59c24f50698600117ed1ae40aa0ddf6e2c69");
        let b = exp.rule_for("0x229d59c24f50698600117ed1ae40aa0ddf6e2c69");
        assert_eq!(a, b);
        let forced = FfwExperiment {
            forced: Some(FfwRule::Off),
            ..Default::default()
        };
        assert_eq!(forced.rule_for("0xanything"), FfwRule::Off);
    }

    #[test]
    fn rate_decision_table() {
        let now = 10_000_000;
        let recent_ts = now - RATE_WINDOW_MS / 2;
        let recent = Some(recent_ts);
        let old = Some(now - RATE_WINDOW_MS - 1);
        assert!(!rate_allows(FfwRule::Off, None, None, now));
        assert!(rate_allows(FfwRule::Unlimited, recent, None, now));
        assert!(rate_allows(FfwRule::Limit1h, None, None, now));
        assert!(!rate_allows(FfwRule::Limit1h, recent, Some(now), now));
        assert!(rate_allows(FfwRule::Limit1h, old, None, now));
        assert!(rate_allows(FfwRule::OnlineBypass, None, None, now));
        assert!(!rate_allows(FfwRule::OnlineBypass, recent, None, now));
        assert!(!rate_allows(
            FfwRule::OnlineBypass,
            recent,
            Some(recent_ts - 1),
            now
        ));
        assert!(rate_allows(
            FfwRule::OnlineBypass,
            recent,
            Some(recent_ts + 1),
            now
        ));
        assert!(rate_allows(FfwRule::OnlineBypass, old, None, now));
    }

    #[test]
    fn urn_normalization_strips_token_and_lowercases() {
        assert_eq!(
            normalize_urn("urn:decentraland:matic:collections-v2:0xABC:1:4567"),
            "urn:decentraland:matic:collections-v2:0xabc:1"
        );
        assert_eq!(
            normalize_urn("urn:decentraland:off-chain:base-avatars:eyes_00"),
            "urn:decentraland:off-chain:base-avatars:eyes_00"
        );
    }

    #[test]
    fn image_repointed_to_our_catalyst() {
        let img = "https://peer.decentraland.org/lambdas/collections/contents/urn:x/thumbnail";
        assert_eq!(repoint_image(Some(img.into()), None), Some(img.into()));
        assert_eq!(
            repoint_image(Some(img.into()), Some("https://catalyst.example")),
            Some("https://catalyst.example/lambdas/collections/contents/urn:x/thumbnail".into())
        );
        assert_eq!(
            repoint_image(Some("https://other/img.png".into()), Some("https://c")),
            Some("https://other/img.png".into())
        );
        assert_eq!(repoint_image(None, None), None);
    }

    #[test]
    fn market_urn_filter_excludes_base_avatars() {
        assert!(is_market_urn(
            "urn:decentraland:matic:collections-v2:0xabc:1"
        ));
        assert!(is_market_urn(
            "urn:decentraland:mainnet:collections-v1:halloween_2019:jester_feet"
        ));
        assert!(!is_market_urn(
            "urn:decentraland:off-chain:base-avatars:balbo_beard"
        ));
    }

    #[test]
    fn wearables_extraction_normalizes_and_dedupes() {
        let meta = serde_json::json!({"v": {"avatars": [{"name": "Wearer", "avatar": {"wearables": [
            "urn:decentraland:off-chain:base-avatars:eyes_00",
            "urn:decentraland:matic:collections-v2:0xABC:1:999",
            "urn:decentraland:matic:collections-v2:0xabc:1"
        ]}}]}});
        assert_eq!(
            profile_wearables(&meta),
            vec!["urn:decentraland:matic:collections-v2:0xabc:1".to_string()]
        );
        assert_eq!(profile_name(&meta).as_deref(), Some("Wearer"));
        assert_eq!(
            short_address("0x229d59c24f50698600117ed1ae40aa0ddf6e2c69"),
            "0x229d...2c69"
        );
    }
}

#[cfg(test)]
mod pg_tests {
    use super::*;
    use crate::notifications::store::test_support::*;

    async fn deploy_profile(
        pool: &PgPool,
        address: &Address,
        ts: chrono::NaiveDateTime,
        wearables: &[&str],
    ) {
        deploy_profile_stored(pool, address, ts, ts, wearables).await;
    }

    async fn deploy_profile_stored(
        pool: &PgPool,
        address: &Address,
        ts: chrono::NaiveDateTime,
        stored: chrono::NaiveDateTime,
        wearables: &[&str],
    ) {
        let meta = json!({"v": {"avatars": [{"name": "W", "avatar": {"wearables": wearables}}]}});
        sqlx::query(
            "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain)
             VALUES ($1, 'v3', 'profile', $2, $3::json, $4, ARRAY[$1::text], $5, '[]'::json)",
        )
        .bind(address.as_str())
        .bind(format!("bafy-{}", Uuid::new_v4().simple()))
        .bind(meta)
        .bind(ts)
        .bind(stored)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn queued_wear(pool: &PgPool, urn: &str) -> FirstWear {
        let wear = FirstWear {
            attempts: 0,
            address: fresh_address(),
            wearer: Some("Wearer".into()),
            urn: urn.into(),
        };
        sqlx::query("INSERT INTO social.worn_history (address, urn, first_seen) VALUES ($1, $2, timezone('UTC', now()))")
            .bind(&wear.address).bind(&wear.urn).execute(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO social.first_wear_outbox (address, urn, wearer) VALUES ($1, $2, $3)",
        )
        .bind(&wear.address)
        .bind(&wear.urn)
        .bind(&wear.wearer)
        .execute(pool)
        .await
        .unwrap();
        wear
    }

    #[tokio::test]
    async fn completion_failure_retains_obligation_and_restart_emits_once() {
        let Some(pool) = private_pool("ffw_atomic").await else {
            return;
        };
        let wear = queued_wear(&pool, "urn:one").await;
        let notice =
            NewNotification::now(fresh_address(), FIRST_WEAR_TYPE, json!({"arm":"unlimited"}));
        sqlx::raw_sql("CREATE FUNCTION social.fail_first_wear_notice() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected notice failure'; END $$;
            CREATE TRIGGER fail_first_wear_notice BEFORE INSERT ON social.notifications FOR EACH ROW EXECUTE FUNCTION social.fail_first_wear_notice();")
            .execute(&pool).await.unwrap();
        assert!(
            complete_wear(&pool, &wear, std::slice::from_ref(&notice))
                .await
                .is_err()
        );
        assert_eq!(pending_wears(&pool, 10).await.unwrap().len(), 1);
        sqlx::query("DROP TRIGGER fail_first_wear_notice ON social.notifications")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            complete_wear(&pool, &wear, std::slice::from_ref(&notice))
                .await
                .unwrap(),
            1
        );
        let mut replay = notice.clone();
        replay.id = Uuid::new_v4();
        assert_eq!(complete_wear(&pool, &wear, &[replay]).await.unwrap(), 0);
        assert!(pending_wears(&pool, 10).await.unwrap().is_empty());
        let deliveries: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.notification_deliveries WHERE notification_id = $1",
        )
        .bind(notice.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(deliveries, 1);
    }

    #[tokio::test]
    async fn competing_completions_recheck_recipient_rate_gate() {
        let Some(pool) = private_pool("ffw_gate").await else {
            return;
        };
        let a = queued_wear(&pool, "urn:one").await;
        let b = queued_wear(&pool, "urn:two").await;
        let recipient = fresh_address();
        let first = [NewNotification::now(
            recipient.clone(),
            FIRST_WEAR_TYPE,
            json!({"arm":"limit_1h"}),
        )];
        let second = [NewNotification::now(
            recipient,
            FIRST_WEAR_TYPE,
            json!({"arm":"limit_1h"}),
        )];
        let mut held = pool.begin().await.unwrap();
        sqlx::query(RECIPIENT_LOCK)
            .execute(&mut *held)
            .await
            .unwrap();
        let (a, b, ()) = tokio::join!(
            complete_wear(&pool, &a, &first),
            complete_wear(&pool, &b, &second),
            async {
                tokio::time::timeout(Duration::from_secs(10), async {
                    loop {
                        let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity
                            WHERE datname = current_database() AND wait_event = 'advisory' AND query = $1")
                            .bind(RECIPIENT_LOCK).fetch_one(&pool).await.unwrap();
                        if waiting == 2 { break; }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }).await.expect("both completions waiting for the recipient lock");
                held.commit().await.unwrap();
            }
        );
        assert_eq!(a.unwrap() + b.unwrap(), 1);
        assert!(pending_wears(&pool, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn transient_lookup_backs_off_and_stops_at_the_attempt_limit() {
        let Some(pool) = private_pool("ffw_retry").await else {
            return;
        };
        let failed = queued_wear(&pool, "urn:decentraland:matic:collections-v2:0xeee:1").await;
        let items = Stub(|_| Err(ItemLookupError::Transient("not ready".into())));
        let settings = FirstWearSettings::default();
        run_once(&pool, Some(&items), &settings).await.unwrap();
        let (attempts, delayed): (i32, bool) = sqlx::query_as(
            "SELECT attempts, retry_at > timezone('UTC', now()) FROM social.first_wear_outbox",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!((attempts, delayed), (1, true));
        run_once(&pool, Some(&items), &settings).await.unwrap();
        assert_eq!(pending_wears(&pool, 1).await.unwrap()[0].attempts, 1);
        sqlx::query(
            "UPDATE social.first_wear_outbox SET attempts = $1, retry_at = timezone('UTC', now())",
        )
        .bind(super::super::delivery::MAX_ATTEMPTS - 1)
        .execute(&pool)
        .await
        .unwrap();
        run_once(&pool, Some(&items), &settings).await.unwrap();
        assert!(pending_wears(&pool, 1).await.unwrap().is_empty());
        assert_eq!(worn(&pool, &failed.address).await, vec![failed.urn]);
    }

    #[tokio::test]
    async fn transport_failures_pause_only_the_affected_network() {
        let Some(pool) = private_pool("ffw_chains").await else {
            return;
        };
        // Polygon's catalogue cannot answer; Ethereum's knows no such item.
        let items = Stub(|urn| {
            if urn.contains(":ethereum:") {
                Ok(None)
            } else {
                Err(ItemLookupError::Transient("not ready".into()))
            }
        });
        for n in 0..OUTBOX_PAGE_SIZE + 1 {
            let network = if n % 2 == 0 { "matic" } else { "polygon" };
            queued_wear(
                &pool,
                &format!("urn:decentraland:{network}:collections-v2:0xeee:{n}"),
            )
            .await;
        }
        let eth = queued_wear(&pool, "urn:decentraland:ethereum:collections-v2:0xeee:1").await;
        run_once(&pool, Some(&items), &FirstWearSettings::default())
            .await
            .unwrap();
        let pending = pending_wears(&pool, 100).await.unwrap();
        assert_eq!(pending.len() as i64, OUTBOX_PAGE_SIZE + 1);
        assert!(pending.iter().all(|w| w.address != eth.address));
        assert_eq!(
            pending.iter().map(|w| w.attempts).sum::<i32>(),
            i32::from(NETWORK_FAILURE_LIMIT)
        );
    }

    /// An item source answering every URN with `0`'s answer.
    struct Stub(fn(&str) -> Result<Option<BuyableItem>, ItemLookupError>);

    #[async_trait::async_trait]
    impl ItemSource for Stub {
        async fn buyable_item(&self, urn: &str) -> Result<Option<BuyableItem>, ItemLookupError> {
            (self.0)(urn)
        }
    }

    /// A buyable first wear notifies the wearer's active friends with the
    /// item the source names; a third-party URN, on Polygon or a testnet, is
    /// in no catalogue and is never looked up.
    #[tokio::test]
    async fn a_buyable_first_wear_notifies_the_active_friends() {
        let Some(pool) = private_pool("ffw_notify").await else {
            return;
        };
        let wear = queued_wear(&pool, "urn:decentraland:matic:collections-v2:0xeee:1").await;
        let friend = fresh_address();
        sqlx::query(
            "INSERT INTO social.friendships (id, address_requester, address_requested, is_active, last_action, last_actor) \
             VALUES ($1, $2, $3, true, 'accept', $3)",
        )
        .bind(Uuid::new_v4())
        .bind(&wear.address)
        .bind(&friend)
        .execute(&pool)
        .await
        .unwrap();
        let items = Stub(|urn| {
            assert!(urn.contains(":matic:collections-v2:"), "{urn}");
            Ok(Some(BuyableItem {
                item_id: "0xeee-1".into(),
                name: "Hat".into(),
                creator: "0x00000000000000000000000000000000000000c0".into(),
                image: None,
            }))
        });
        queued_wear(&pool, "urn:decentraland:amoy:collections-thirdparty:tp:1").await;
        queued_wear(&pool, "urn:decentraland:matic:collections-thirdparty:tp:1").await;
        let emitted = run_once(&pool, Some(&items), &FirstWearSettings::default())
            .await
            .unwrap();
        assert_eq!(emitted, 1);
        let metadata: Json = sqlx::query_scalar(
            "SELECT metadata FROM social.notifications WHERE address = $1 AND type = $2",
        )
        .bind(&friend)
        .bind(FIRST_WEAR_TYPE)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(metadata["itemId"], "0xeee-1", "{metadata}");
        assert_eq!(metadata["wearableName"], "Hat", "{metadata}");
        assert!(pending_wears(&pool, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn full_queue_pauses_intake_until_old_obligations_expire() {
        let Some(pool) = private_pool("ffw_bound").await else {
            return;
        };
        let settings = FirstWearSettings::default();
        run_once(&pool, None, &settings).await.unwrap();
        let before = cursor(&pool).await;
        let wearer = fresh_address();
        deploy_profile(&pool, &wearer, now_utc(), &[]).await;
        sqlx::query("WITH history AS (
            INSERT INTO social.worn_history (address, urn, first_seen)
            SELECT $1, i::text, timezone('UTC', now()) - interval '25 hours' FROM generate_series(1, $2) i RETURNING address, urn)
            INSERT INTO social.first_wear_outbox (address, urn, created_at, retry_at)
            SELECT address, urn, timezone('UTC', now()), timezone('UTC', now()) + interval '1 hour' FROM history")
            .bind(&wearer).bind(INTAKE_HIGH_WATER_MARK as i32).execute(&pool).await.unwrap();
        run_once(&pool, None, &settings).await.unwrap();
        assert_eq!(cursor(&pool).await, before);
        assert!(pending_wears(&pool, 1).await.unwrap().is_empty());
        run_once(&pool, None, &settings).await.unwrap();
        assert!(has_baseline(&pool, &wearer).await);
    }

    #[tokio::test]
    async fn failed_enqueue_rolls_back_claim_and_cursor() {
        let Some(pool) = private_pool("ffw_enqueue").await else {
            return;
        };
        let settings = FirstWearSettings::default();
        run_once(&pool, None, &settings).await.unwrap();
        let wearer = fresh_address();
        deploy_profile(&pool, &wearer, now_utc(), &[]).await;
        run_once(&pool, None, &settings).await.unwrap();
        let before = cursor(&pool).await;
        deploy_profile(
            &pool,
            &wearer,
            now_utc(),
            &["urn:decentraland:matic:collections-v2:0xabc:1"],
        )
        .await;
        queued_wear(&pool, "already-queued").await;
        sqlx::raw_sql("CREATE FUNCTION social.fail_first_wear_enqueue() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected enqueue failure'; END $$;
            CREATE TRIGGER fail_first_wear_enqueue BEFORE INSERT ON social.first_wear_outbox FOR EACH ROW EXECUTE FUNCTION social.fail_first_wear_enqueue();")
            .execute(&pool).await.unwrap();
        assert!(run_once(&pool, None, &settings).await.is_err());
        assert!(pending_wears(&pool, 1).await.unwrap().is_empty());
        assert_eq!(cursor(&pool).await, before);
        assert!(worn(&pool, &wearer).await.is_empty());
        sqlx::query("DROP TRIGGER fail_first_wear_enqueue ON social.first_wear_outbox")
            .execute(&pool)
            .await
            .unwrap();
        run_once(&pool, None, &settings).await.unwrap();
        assert_eq!(worn(&pool, &wearer).await.len(), 1);
    }

    async fn has_baseline(pool: &PgPool, address: &Address) -> bool {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM social.first_wear_baseline WHERE address = $1)",
        )
        .bind(address)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    async fn worn(pool: &PgPool, address: &Address) -> Vec<String> {
        sqlx::query_scalar("SELECT urn FROM social.worn_history WHERE address = $1 ORDER BY urn")
            .bind(address)
            .fetch_all(pool)
            .await
            .unwrap()
    }

    async fn cursor(pool: &PgPool) -> Option<i64> {
        sqlx::query_scalar("SELECT position FROM social.badge_cursors WHERE source = $1")
            .bind(CURSOR_SOURCE)
            .fetch_optional(pool)
            .await
            .unwrap()
    }

    /// The cursor is one row for the whole table and the pass reads every
    /// deployment past it, so the test runs on a database of its own.
    #[tokio::test]
    async fn first_profile_seeds_the_baseline_and_a_new_urn_is_claimed_once() {
        let Some(pool) = private_pool("ffw").await else {
            return;
        };
        let a = fresh_address();
        let settings = FirstWearSettings::default();

        let history = fresh_address();
        deploy_profile(
            &pool,
            &history,
            now_utc(),
            &["urn:decentraland:matic:collections-v2:0xCCC:1:5"],
        )
        .await;
        assert_eq!(
            run_once(&pool, None, &settings).await.unwrap(),
            0,
            "the first pass only initialises the cursor"
        );
        assert!(cursor(&pool).await.is_some_and(|p| p > 0), "at the head");
        assert!(
            !has_baseline(&pool, &history).await,
            "history before the cursor is never replayed"
        );

        deploy_profile(
            &pool,
            &a,
            now_utc(),
            &[
                "urn:decentraland:matic:collections-v2:0xAAA:1:5",
                "urn:decentraland:off-chain:base-avatars:eyes_00",
            ],
        )
        .await;
        assert_eq!(run_once(&pool, None, &settings).await.unwrap(), 0);
        assert!(has_baseline(&pool, &a).await);
        assert_eq!(
            worn(&pool, &a).await,
            vec!["urn:decentraland:matic:collections-v2:0xaaa:1".to_string()],
            "market urns only, normalized"
        );

        deploy_profile(
            &pool,
            &a,
            now_utc(),
            &[
                "urn:decentraland:matic:collections-v2:0xAAA:1:5",
                "urn:decentraland:matic:collections-v2:0xBBB:2:9",
            ],
        )
        .await;
        assert_eq!(
            run_once(&pool, None, &settings).await.unwrap(),
            0,
            "no marketplace reader: nothing resolves as buyable"
        );
        assert_eq!(
            worn(&pool, &a).await.len(),
            2,
            "the new urn is claimed even when not notifiable, so a restart never re-notifies it"
        );

        let before = cursor(&pool).await;
        assert_eq!(
            run_once(&pool, None, &settings).await.unwrap(),
            0,
            "nothing past the cursor"
        );
        assert_eq!(cursor(&pool).await, before);
    }

    /// The cursor follows the order the node stored deployments in, not the
    /// signer's clock: a profile signed ahead does not hide the ones stored
    /// after it, profiles sharing a timestamp across a batch edge are all
    /// read, and a profile stored long after it was signed is passed.
    #[tokio::test]
    async fn profiles_are_read_in_storage_order_whatever_their_timestamps() {
        let Some(pool) = private_pool("ffw_order").await else {
            return;
        };
        let settings = FirstWearSettings::default();
        run_once(&pool, None, &settings).await.unwrap();
        let now = chrono::Timelike::with_nanosecond(&now_utc(), 0).unwrap();

        let ahead = fresh_address();
        deploy_profile(&pool, &ahead, now + chrono::Duration::minutes(14), &[]).await;
        run_once(&pool, None, &settings).await.unwrap();
        assert!(has_baseline(&pool, &ahead).await);

        let late = fresh_address();
        sqlx::query("INSERT INTO social.first_wear_baseline (address, seeded_at) VALUES ($1, $2)")
            .bind(&late)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();
        deploy_profile(
            &pool,
            &late,
            now,
            &["urn:decentraland:matic:collections-v2:0xDDD:1:1"],
        )
        .await;
        run_once(&pool, None, &settings).await.unwrap();
        assert_eq!(
            worn(&pool, &late).await,
            vec!["urn:decentraland:matic:collections-v2:0xddd:1".to_string()],
            "a profile stored after one signed ahead is still read"
        );

        let tied: Vec<Address> = (0..201).map(|_| fresh_address()).collect();
        for address in &tied {
            deploy_profile(&pool, address, now, &[]).await;
        }
        run_once(&pool, None, &settings).await.unwrap();
        run_once(&pool, None, &settings).await.unwrap();
        for address in &tied {
            assert!(
                has_baseline(&pool, address).await,
                "every profile sharing one timestamp is read"
            );
        }

        let synced = fresh_address();
        deploy_profile_stored(&pool, &synced, now - chrono::Duration::days(2), now, &[]).await;
        let backlog = fresh_address();
        let two_days_ago = now - chrono::Duration::days(2);
        deploy_profile_stored(&pool, &backlog, two_days_ago, two_days_ago, &[]).await;
        run_once(&pool, None, &settings).await.unwrap();
        assert!(
            !has_baseline(&pool, &synced).await,
            "a profile stored long after it was signed is synced history: passed"
        );
        assert!(
            has_baseline(&pool, &backlog).await,
            "a profile stored when it was signed is read however late the pass"
        );
    }

    /// One pass drains batch after batch of the configured size, and a wear
    /// whose item cannot be resolved is logged without failing the pass or
    /// the wears after it.
    #[tokio::test]
    async fn a_pass_drains_small_batches_and_survives_a_failed_resolution() {
        let Some(pool) = private_pool("ffw_batches").await else {
            return;
        };
        let settings = FirstWearSettings {
            batch_size: 1,
            ..FirstWearSettings::default()
        };
        run_once(&pool, None, &settings).await.unwrap();

        let wearers: Vec<Address> = (0..3).map(|_| fresh_address()).collect();
        for wearer in &wearers {
            sqlx::query(
                "INSERT INTO social.first_wear_baseline (address, seeded_at) VALUES ($1, $2)",
            )
            .bind(wearer)
            .bind(now_utc())
            .execute(&pool)
            .await
            .unwrap();
            deploy_profile(
                &pool,
                wearer,
                now_utc(),
                &[
                    "urn:decentraland:matic:collections-v2:0xEEE:1:1",
                    "urn:decentraland:matic:collections-v2:0xFFF:2:2",
                ],
            )
            .await;
        }
        // A source that refuses every lookup.
        let items = Stub(|_| Err(ItemLookupError::Invalid("refused".into())));
        assert_eq!(
            run_once(&pool, Some(&items), &settings).await.unwrap(),
            0,
            "a failed resolution is logged, not returned"
        );
        assert_eq!(
            pending_wears(&pool, 10).await.unwrap().len(),
            0,
            "a refused lookup is terminal"
        );
        for wearer in &wearers {
            assert_eq!(
                worn(&pool, wearer).await.len(),
                2,
                "every batch of one deployment was read and claimed in one pass"
            );
        }
    }

    #[tokio::test]
    async fn a_pass_drains_more_than_one_outbox_batch() {
        let Some(pool) = private_pool("ffw_drain").await else {
            return;
        };
        let settings = FirstWearSettings {
            batch_size: 1,
            ..FirstWearSettings::default()
        };
        for n in 0..(OUTBOX_PAGE_SIZE + 1) {
            queued_wear(&pool, &format!("urn:wear:{n}")).await;
        }
        run_once(&pool, None, &settings).await.unwrap();
        assert!(pending_wears(&pool, 10).await.unwrap().is_empty());
    }

    /// Names come from the profile lambdas serves: the active entity, never a
    /// denylisted one, the name as deployed.
    #[tokio::test]
    async fn profile_names_follow_the_active_non_denylisted_entity() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let a = fresh_address();
        let deploy = |name: &'static str, active: bool| {
            let (pool, a) = (pool.clone(), a.clone());
            async move {
                let entity = format!("bafy-{}", Uuid::new_v4().simple());
                sqlx::query(
                    "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain)
                     VALUES ($1, 'v3', 'profile', $2, $3::json, timezone('UTC', now()), ARRAY[$1::text], timezone('UTC', now()), '[]'::json)",
                )
                .bind(&a)
                .bind(&entity)
                .bind(json!({"v": {"avatars": [{"name": name, "hasClaimedName": false}]}}))
                .execute(&pool)
                .await
                .unwrap();
                if active {
                    sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, 'profile') ON CONFLICT (pointer) DO UPDATE SET entity_id = EXCLUDED.entity_id")
                        .bind(&a)
                        .bind(&entity)
                        .execute(&pool)
                        .await
                        .unwrap();
                }
                entity
            }
        };
        assert_eq!(creator_profile_name(&pool, a.as_str()).await.unwrap(), None);
        let active = deploy("Active", true).await;
        deploy("Newer but not active", false).await;
        let upper = a.as_str().to_uppercase().replace("0X", "0x");
        assert_eq!(
            creator_profile_name(&pool, &upper)
                .await
                .unwrap()
                .as_deref(),
            Some("Active")
        );
        sqlx::query("INSERT INTO content.denylist (id, added_by) VALUES ($1, 'test')")
            .bind(&active)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(creator_profile_name(&pool, a.as_str()).await.unwrap(), None);
    }

    /// The rate-limit and online-bypass gates of every friend come back from
    /// one statement (review finding database-first-wear-per-friend-round-trips:
    /// two probes per friend, 2 x degree round trips per first wear).
    #[tokio::test]
    async fn friend_gates_read_every_recipient_in_one_statement() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let (emitted, online, silent) = (fresh_address(), fresh_address(), fresh_address());
        let now_ms = chrono::Utc::now().timestamp_millis();
        let emit_at = ms_to_naive(now_ms - 30 * 60 * 1000).unwrap();
        insert_notifications(
            &pool,
            &[
                NewNotification {
                    id: Uuid::new_v4(),
                    address: emitted.clone(),
                    kind: FIRST_WEAR_TYPE.to_string(),
                    metadata: json!({}),
                    occurred_at: emit_at - chrono::Duration::hours(2),
                },
                NewNotification {
                    id: Uuid::new_v4(),
                    address: emitted.clone(),
                    kind: FIRST_WEAR_TYPE.to_string(),
                    metadata: json!({}),
                    occurred_at: emit_at,
                },
                NewNotification {
                    id: Uuid::new_v4(),
                    address: online.clone(),
                    kind: "other_type".to_string(),
                    metadata: json!({}),
                    occurred_at: emit_at,
                },
            ],
        )
        .await
        .unwrap();
        let fetch_at = ms_to_naive(now_ms - 5 * 60 * 1000).unwrap();
        sqlx::query(
            "INSERT INTO social.notification_reader_seen (address, last_fetch_at) VALUES ($1, $2)",
        )
        .bind(&online)
        .bind(fetch_at)
        .execute(&pool)
        .await
        .unwrap();

        let gates = friend_gates(&pool, &[emitted.clone(), online.clone(), silent.clone()])
            .await
            .unwrap();
        let ms = |t: Option<NaiveDateTime>| t.map(naive_to_ms);
        let (e, f) = gates[emitted.as_str()];
        assert_eq!(
            (ms(e), ms(f)),
            (Some(naive_to_ms(emit_at)), None),
            "the newest first-wear emit, no fetch"
        );
        let (e, f) = gates[online.as_str()];
        assert_eq!(
            (ms(e), ms(f)),
            (None, Some(naive_to_ms(fetch_at))),
            "another type is no emit; the fetch counts"
        );
        assert!(
            !gates.contains_key(silent.as_str()),
            "a friend with neither row is absent"
        );
        assert!(friend_gates(&pool, &[]).await.unwrap().is_empty());
    }
}
