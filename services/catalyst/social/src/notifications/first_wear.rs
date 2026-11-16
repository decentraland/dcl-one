//! The `friend_first_wear` producer: a profile deployment whose wearables
//! include a marketplace item the wallet never wore before notifies the
//! wallet's active friends, rate limited per recipient by the `ffw_rules`
//! arms. One pool now: profiles come from `content.deployments`, friends
//! from `social.friendships`; the item's name/creator/image/buyability come
//! from the collections subgraph through the marketplace reader (the old
//! crate read the squid mirror, which v2 does not run). The telemetry store
//! that could override the arms and the funnel events are not carried.

use crate::shared::now_utc;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDateTime;
use serde_json::{Value as Json, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::store::{NewNotification, insert_notifications, ms_to_naive, naive_to_ms};
use crate::badges::jobs::{IdSource, Keyed, Pass, ProcessorSettings, read_ids};
use crate::chain_index::ChainIndex;
use crate::chain_index::graph::{Network, graph_query, normalize_urn as graph_urn};
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

struct ResolvedItem {
    item_id: String,
    name: String,
    creator: String,
    image: Option<String>,
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

/// A URN claimed in `social.worn_history` by a batch, to notify after it
/// commits.
struct FirstWear {
    address: Address,
    wearer: Option<String>,
    urn: String,
}

/// One pass: the profile deployments past the cursor, in `id` order (the
/// order the node stored them, whatever their signed timestamps), batch by
/// batch. Returns the notifications emitted. Without a marketplace reader
/// nothing can be resolved as buyable, so the pass only seeds baselines and
/// claims first wears.
pub async fn run_once(
    pool: &PgPool,
    market: Option<&ChainIndex>,
    settings: &FirstWearSettings,
) -> anyhow::Result<u64> {
    let experiment = FfwExperiment::default();
    let pass = Pass {
        pool: pool.clone(),
        asset_base: String::new(),
        settings: ProcessorSettings {
            batch_size: settings.batch_size,
            ..ProcessorSettings::default()
        },
        pinned_xmin: None,
    };
    let mut emitted = 0u64;
    for _ in 0..MAX_BATCHES_PER_PASS {
        let (wears, more) = claim_batch(&pass).await?;
        for wear in &wears {
            match notify(pool, market, settings, &experiment, wear).await {
                Ok(n) => emitted += n,
                Err(e) => {
                    tracing::warn!(urn = %wear.urn, error = %e, "first wear not notified");
                }
            }
        }
        if !more {
            break;
        }
    }
    crate::badges::jobs::report_id_lag(pool, &READ).await;
    Ok(emitted)
}

/// One transaction: reads a batch past the cursor, seeds the baselines and
/// claims the new URNs of its fresh profiles, and advances the cursor, so a
/// database error rolls all of it back and the next pass reads the same
/// rows. The first pass records the head of `content.deployments` and replays
/// no history. Returns the claimed first wears and whether rows remain.
async fn claim_batch(pass: &Pass) -> anyhow::Result<(Vec<FirstWear>, bool)> {
    let mut tx = pass.pool.begin().await?;
    let Some(read) = read_ids::<Deployment>(&mut tx, pass, &READ).await? else {
        tx.commit().await?;
        tracing::info!("friend_first_wear cursor initialized (no history replay)");
        return Ok((Vec::new(), false));
    };
    if read.is_idle() {
        tx.rollback().await?;
        return Ok((Vec::new(), false));
    }
    let max_lag = chrono::Duration::hours(MAX_STORAGE_LAG_HOURS);
    let mut wears = Vec::new();
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
            wears.push(FirstWear {
                address: address.clone(),
                wearer: profile_name(&meta),
                urn,
            });
        }
    }
    read.finish(&mut tx).await?;
    tx.commit().await?;
    Ok((wears, read.more))
}

/// Notifies the wearer's active friends of one claimed first wear, when its
/// item resolves as buyable. The claim is committed first, so a restart
/// never re-notifies it.
async fn notify(
    pool: &PgPool,
    market: Option<&ChainIndex>,
    settings: &FirstWearSettings,
    experiment: &FfwExperiment,
    wear: &FirstWear,
) -> anyhow::Result<u64> {
    let (address, urn) = (&wear.address, wear.urn.as_str());
    let Some(market) = market else {
        tracing::debug!(
            urn,
            "first wear not notifiable (no marketplace reader configured)"
        );
        return Ok(0);
    };
    let Some(item) = resolve_buyable_item(market, urn).await? else {
        tracing::debug!(
            urn,
            "first wear not notifiable (item unknown or not buyable)"
        );
        return Ok(0);
    };
    let friends = active_friends(pool, address).await?;
    if friends.is_empty() {
        return Ok(0);
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

    let gates = friend_gates(pool, &friends).await?;
    let mut batch = Vec::new();
    for friend in &friends {
        let rule = experiment.rule_for(friend.as_str());
        let (last_emit, last_fetch) = gates.get(friend.as_str()).copied().unwrap_or((None, None));
        let allowed = !experiment.killed
            && rate_allows(
                rule,
                last_emit.map(naive_to_ms),
                last_fetch.map(naive_to_ms),
                now_ms,
            );
        if !allowed {
            let reason = if experiment.killed {
                "killed"
            } else if rule == FfwRule::Off {
                "arm_off"
            } else {
                "rate"
            };
            tracing::debug!(friend = %friend, rule = rule.as_str(), reason, "ffw suppressed");
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
    Ok(insert_notifications(pool, &batch).await?)
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

/// The item behind a normalized URN, if the collections subgraph of its
/// network knows it and it can be bought (store-minted with stock, or an
/// open order). Third-party and off-chain URNs have no reader.
async fn resolve_buyable_item(
    market: &ChainIndex,
    urn: &str,
) -> anyhow::Result<Option<ResolvedItem>> {
    let Some(network) = Network::of_urn(urn) else {
        return Ok(None);
    };
    let url = match network {
        Network::Ethereum => market.settings.eth_collections_subgraph_url.as_deref(),
        Network::Matic => market.settings.matic_collections_subgraph_url.as_deref(),
    };
    let Some(url) = url else {
        return Ok(None);
    };
    let doc = "query($urn: String!) { items(first: 1, where: { urn: $urn }) { id available searchIsStoreMinter creator image metadata { wearable { name } emote { name } } } }";
    let answer = graph_query(market, url, doc, json!({ "urn": graph_urn(urn) }))
        .await
        .map_err(anyhow::Error::msg)?;
    let item = &answer["data"]["items"][0];
    if item.is_null() {
        return Ok(None);
    }
    let Some(item_id) = item["id"].as_str() else {
        return Ok(None);
    };
    let name = item["metadata"]["wearable"]["name"]
        .as_str()
        .or_else(|| item["metadata"]["emote"]["name"].as_str())
        .map(str::to_string);
    let Some(name) = name else {
        return Ok(None);
    };
    let available = item["available"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let store_minter = item["searchIsStoreMinter"].as_bool().unwrap_or(false);
    let buyable = if store_minter && available > 0 {
        true
    } else {
        let now = chrono::Utc::now().timestamp().to_string();
        let orders = graph_query(
            market,
            url,
            "query($item: String!, $now: BigInt!) { orders(first: 1, where: { item: $item, status: open, expiresAt_gt: $now }) { id } }",
            json!({ "item": item_id, "now": now }),
        )
        .await
        .map_err(anyhow::Error::msg)?;
        orders["data"]["orders"]
            .as_array()
            .is_some_and(|o| !o.is_empty())
    };
    if !buyable {
        return Ok(None);
    }
    Ok(Some(ResolvedItem {
        item_id: item_id.to_string(),
        name,
        creator: item["creator"].as_str().unwrap_or_default().to_lowercase(),
        image: item["image"].as_str().map(str::to_string),
    }))
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
async fn friend_gates(
    pool: &PgPool,
    friends: &[Address],
) -> anyhow::Result<HashMap<String, (Option<NaiveDateTime>, Option<NaiveDateTime>)>> {
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
    .fetch_all(pool)
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
        move |_ctx| {
            let state = state.clone();
            async move {
                let emitted = run_once(
                    state.store.pool(),
                    state.market.as_deref(),
                    &state.settings.first_wear,
                )
                .await?;
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
        // A subgraph URL the reader refuses: every resolution errors.
        let market = ChainIndex::new(
            crate::http::Outbound::for_tests(),
            crate::chain_index::IndexSettings {
                matic_collections_subgraph_url: Some("http://127.0.0.1:9/graph".to_string()),
                ..crate::chain_index::IndexSettings::for_tests()
            },
        );
        assert_eq!(
            run_once(&pool, Some(&market), &settings).await.unwrap(),
            0,
            "a failed resolution is logged, not returned"
        );
        for wearer in &wearers {
            assert_eq!(
                worn(&pool, wearer).await.len(),
                2,
                "every batch of one deployment was read and claimed in one pass"
            );
        }
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
