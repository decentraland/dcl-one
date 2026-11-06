//! The `item_discounted` producer (marketplace-server's favorite-discount
//! job): when a creator coupon takes effect, the people who favorited one of
//! the primary listings it discounts hear about it, in-app only. A coupon is
//! announced once, on the first pass after it takes effect, never when it
//! ends within 6 hours, started over a day ago, or before this producer first
//! ran (`shop.favorite_discount_cursor`). Per user: one notification per item,
//! at most 3 a rolling day and the same item at most once a week whatever the
//! coupon; `social.notifications` is the ledger those caps read. A coupon
//! whose listings the catalogue does not show yet is retried with a doubling
//! delay for 30 minutes (6 attempts at most). Each coupon's notifications
//! and its announcement mark commit together; the id is a uuid v5 of
//! `(coupon, user, item)`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDateTime;
use futures::future::BoxFuture;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::jobs::{Jobs, PeriodicCfg};
use crate::market::MarketplaceState;
use crate::market::shop::{SHOP_MAX_PAGE_SIZE, ShopCatalogFilters, ShopListingType, ShopSortBy};
use crate::shared::identity::Address;
use crate::social::notifications::store::{NewNotification, insert_notifications};

pub const KIND: &str = "item_discounted";
const INTERVAL: Duration = Duration::from_secs(5 * 60);
pub const MAX_PER_USER_PER_DAY: usize = 3;
const MIN_TIME_LEFT_SECS: i64 = 6 * 3600;
const COUPONS_PER_RUN: i64 = 20;
const CATALOGUE_GRACE_SECS: i64 = 30 * 60;
const RETRY_BASE_SECS: i64 = 5 * 60;
const MAX_ATTEMPTS: i32 = 6;
const MAX_ANNOUNCE_AGE_SECS: i64 = 24 * 3600;
const ITEM_COOLDOWN_SECS: i64 = 7 * 24 * 3600;

/// A coupon's discounted listings (the Shop catalogue; a fake in tests).
pub type Listings<'a> = &'a (
        dyn Fn(PendingCoupon) -> BoxFuture<'static, anyhow::Result<Vec<Discounted>>> + Send + Sync
    );

pub fn notification_id(coupon: Uuid, address: &str, item_key: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("discount:{coupon}:{address}:{item_key}").as_bytes(),
    )
}

pub fn item_key(contract: &str, item_id: &str) -> String {
    format!("{}-{item_id}", contract.to_ascii_lowercase())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PendingCoupon {
    pub id: Uuid,
    pub discount_ppm: i32,
    pub collections: Vec<String>,
    /// When it took effect, or when it was created if signed with a past start.
    pub effective_since: NaiveDateTime,
    pub expires_at: NaiveDateTime,
}

/// A primary listing this coupon is the applied discount of, and lowers.
#[derive(Debug, Clone)]
pub struct Discounted {
    pub contract_address: String,
    pub item_id: String,
    pub creator: String,
    pub name: String,
    pub thumbnail: String,
    pub category: String,
    pub rarity: String,
    pub network: String,
    pub list_price: i64,
    pub sale_price: i64,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub user: String,
    pub favorited_at: NaiveDateTime,
    pub listing: Discounted,
}

/// One per (user, item) not already sent, within each user's daily budget,
/// the most recently favorited first.
pub fn select(
    mut candidates: Vec<Candidate>,
    already: &HashSet<(String, String)>,
    sent_today: &HashMap<String, usize>,
) -> Vec<Candidate> {
    candidates.sort_by_key(|c| std::cmp::Reverse(c.favorited_at));
    let mut budget: HashMap<String, usize> = HashMap::new();
    let mut seen = HashSet::new();
    let mut selected = Vec::new();
    for c in candidates {
        let key = (
            c.user.clone(),
            item_key(&c.listing.contract_address, &c.listing.item_id),
        );
        if already.contains(&key) || !seen.insert(key) {
            continue;
        }
        let left = budget.entry(c.user.clone()).or_insert_with(|| {
            MAX_PER_USER_PER_DAY.saturating_sub(sent_today.get(&c.user).copied().unwrap_or(0))
        });
        if *left == 0 {
            continue;
        }
        *left -= 1;
        selected.push(c);
    }
    selected
}

/// `Math.round(ppm / 10_000)`.
fn discount_pct(ppm: i32) -> i64 {
    (ppm as i64 + 5_000).div_euclid(10_000)
}

/// The old event's metadata, field for field (`rarity`/`nftName` absent when empty).
pub fn metadata(c: &Candidate, coupon: &PendingCoupon, shop_base_url: &str) -> Value {
    let l = &c.listing;
    let pct = discount_pct(coupon.discount_ppm);
    let mut m = json!({
        "image": l.thumbnail,
        "category": l.category,
        "contractAddress": l.contract_address,
        "itemId": l.item_id,
        "link": format!("{}/item/{}/{}", shop_base_url.trim_end_matches('/'), l.contract_address, l.item_id),
        "discountPct": pct,
        "listPrice": l.list_price.to_string(),
        "salePrice": l.sale_price.to_string(),
        "endsAt": coupon.expires_at.and_utc().timestamp_millis(),
        "title": "A favorite is on sale",
        "description": if l.name.is_empty() {
            format!("An item you saved is {pct}% off.")
        } else {
            format!("{} is {pct}% off.", l.name)
        },
        "network": l.network,
    });
    let obj = m.as_object_mut().expect("object");
    if !l.rarity.is_empty() {
        obj.insert("rarity".into(), json!(l.rarity));
    }
    if !l.name.is_empty() {
        obj.insert("nftName".into(), json!(l.name));
    }
    m
}

async fn defer(pool: &PgPool, coupon: Uuid, now: NaiveDateTime) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO shop.coupon_favorites_announcement (coupon_id, attempts, next_attempt_at) \
         VALUES ($1, 1, $2 + make_interval(secs => $3)) \
         ON CONFLICT (coupon_id) DO UPDATE SET attempts = coupon_favorites_announcement.attempts + 1, \
           next_attempt_at = $2 + make_interval(secs => $3 * power(2, coupon_favorites_announcement.attempts))",
    )
    .bind(coupon)
    .bind(now)
    .bind(RETRY_BASE_SECS as f64)
    .execute(pool)
    .await?;
    Ok(())
}

/// One pass at `now`; `listings` answers a coupon's discounted listings.
/// Returns (coupons read, notifications inserted).
pub async fn run_once(
    pool: &PgPool,
    shop_base_url: &str,
    now: NaiveDateTime,
    listings: Listings<'_>,
) -> anyhow::Result<(usize, u64)> {
    sqlx::query(
        "INSERT INTO shop.favorite_discount_cursor (id, started_at) VALUES (1, $1) ON CONFLICT (id) DO NOTHING",
    )
    .bind(now)
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO shop.coupon_favorites_announcement (coupon_id, notified_at) \
         SELECT c.id, $1 FROM shop.coupons c \
         CROSS JOIN shop.favorite_discount_cursor cur \
         LEFT JOIN shop.coupon_favorites_announcement a ON a.coupon_id = c.id \
         LEFT JOIN shop.coupon_state s ON s.coupon_id = c.id \
         WHERE a.notified_at IS NULL AND ( \
           c.expires_at <= $1 + make_interval(secs => $2) \
           OR GREATEST(c.effective_since, c.created_at) <= $1 - make_interval(secs => $3) \
           OR GREATEST(c.effective_since, c.created_at) < cur.started_at \
           OR COALESCE(a.attempts, 0) >= $4 \
           OR COALESCE(s.cancelled, false) OR COALESCE(s.revoked, false) \
           OR COALESCE(s.uses, 0) >= (c.checks->>'uses')::numeric) \
         ON CONFLICT (coupon_id) DO UPDATE SET notified_at = EXCLUDED.notified_at",
    )
    .bind(now)
    .bind(MIN_TIME_LEFT_SECS as f64)
    .bind(MAX_ANNOUNCE_AGE_SECS as f64)
    .bind(MAX_ATTEMPTS)
    .execute(pool)
    .await?;
    let coupons: Vec<PendingCoupon> = sqlx::query_as(
        "SELECT c.id, c.discount_ppm, c.collections, \
                GREATEST(c.effective_since, c.created_at) AS effective_since, c.expires_at \
         FROM shop.coupons c \
         LEFT JOIN shop.coupon_favorites_announcement a ON a.coupon_id = c.id \
         WHERE a.notified_at IS NULL AND c.effective_since <= $1 \
           AND (a.next_attempt_at IS NULL OR a.next_attempt_at <= $1) \
         ORDER BY COALESCE(a.attempts, 0), c.discount_ppm DESC, c.effective_since \
         LIMIT $2",
    )
    .bind(now)
    .bind(COUPONS_PER_RUN)
    .fetch_all(pool)
    .await?;
    let mut sent = 0;
    for coupon in &coupons {
        match announce(pool, shop_base_url, now, coupon, listings).await {
            Ok(Some(n)) => {
                sent += n;
                tracing::info!(coupon = %coupon.id, notified = n, "favorite discount announced");
            }
            Ok(None) => defer(pool, coupon.id, now).await?,
            Err(e) => {
                tracing::error!(coupon = %coupon.id, error = %e, "could not announce coupon; deferring it");
                defer(pool, coupon.id, now).await?;
            }
        }
    }
    Ok((coupons.len(), sent))
}

/// `None`: the catalogue does not show the coupon's listings yet.
async fn announce(
    pool: &PgPool,
    shop_base_url: &str,
    now: NaiveDateTime,
    coupon: &PendingCoupon,
    listings: Listings<'_>,
) -> anyhow::Result<Option<u64>> {
    let found = listings(coupon.clone()).await?;
    if found.is_empty() && (now - coupon.effective_since).num_seconds() < CATALOGUE_GRACE_SECS {
        return Ok(None);
    }
    let by_key: HashMap<String, Discounted> = found
        .into_iter()
        .map(|l| (item_key(&l.contract_address, &l.item_id), l))
        .collect();
    let keys: Vec<&String> = by_key.keys().collect();
    let favoriters: Vec<(String, String, NaiveDateTime)> = sqlx::query_as(
        "SELECT user_address, item_id, MAX(created_at) FROM shop.picks \
         WHERE item_id = ANY($1) GROUP BY user_address, item_id",
    )
    .bind(&keys)
    .fetch_all(pool)
    .await?;
    let candidates: Vec<Candidate> = favoriters
        .into_iter()
        .filter_map(|(user, key, favorited_at)| {
            let listing = by_key.get(&key)?;
            (!user.eq_ignore_ascii_case(&listing.creator)).then(|| Candidate {
                user: user.to_ascii_lowercase(),
                favorited_at,
                listing: listing.clone(),
            })
        })
        .collect();
    let users: Vec<String> = candidates
        .iter()
        .map(|c| c.user.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let recent: Vec<(String, String, String, bool)> = sqlx::query_as(
        "SELECT address, COALESCE(metadata->>'contractAddress', ''), COALESCE(metadata->>'itemId', ''), \
                occurred_at > $2 - interval '24 hours' \
         FROM social.notifications \
         WHERE address = ANY($1) AND type = $3 AND occurred_at > $2 - make_interval(secs => $4)",
    )
    .bind(&users)
    .bind(now)
    .bind(KIND)
    .bind(ITEM_COOLDOWN_SECS as f64)
    .fetch_all(pool)
    .await?;
    let mut already = HashSet::new();
    let mut today: HashMap<String, usize> = HashMap::new();
    for (address, contract, item, in_day) in recent {
        if in_day {
            *today.entry(address.clone()).or_default() += 1;
        }
        already.insert((address, item_key(&contract, &item)));
    }
    let batch: Vec<NewNotification> = select(candidates, &already, &today)
        .into_iter()
        .filter_map(|c| {
            Some(NewNotification {
                id: notification_id(
                    coupon.id,
                    &c.user,
                    &item_key(&c.listing.contract_address, &c.listing.item_id),
                ),
                address: Address::parse(&c.user).ok()?,
                kind: KIND.into(),
                metadata: metadata(&c, coupon, shop_base_url),
                occurred_at: now,
            })
        })
        .collect();
    let mut txn = pool.begin().await?;
    let inserted = insert_notifications(&mut *txn, &batch).await?;
    sqlx::query(
        "INSERT INTO shop.coupon_favorites_announcement (coupon_id, notified_at) VALUES ($1, $2) \
         ON CONFLICT (coupon_id) DO UPDATE SET notified_at = EXCLUDED.notified_at",
    )
    .bind(coupon.id)
    .bind(now)
    .execute(&mut *txn)
    .await?;
    txn.commit().await?;
    Ok(Some(inserted))
}

/// The coupon's discounted primaries from the Shop catalogue.
async fn catalogue_listings(
    state: &MarketplaceState,
    coupon: &PendingCoupon,
) -> anyhow::Result<Vec<Discounted>> {
    let coupon_id = coupon.id.to_string();
    let mut out: HashMap<String, Discounted> = HashMap::new();
    for contract in &coupon.collections {
        let mut skip = 0;
        loop {
            let filters = ShopCatalogFilters {
                contract_address: Some(contract.clone()),
                discounted: Some(true),
                listing_type: Some(ShopListingType::Primary),
                sort_by: Some(ShopSortBy::Discount),
                first: Some(SHOP_MAX_PAGE_SIZE),
                skip: Some(skip),
                ..Default::default()
            };
            let (page, total) = crate::market::shop::shop_listings(state, &filters)
                .await
                .map_err(|e| anyhow::anyhow!("shop catalogue: {e:?}"))?;
            for l in page {
                let (Some(item_id), Some(list), Some(sale)) =
                    (l.item_id, l.sale.compare_at_credits, l.price_credits)
                else {
                    continue;
                };
                let applied = l
                    .sale
                    .coupon
                    .as_ref()
                    .and_then(|c| c.get("id")?.as_str().map(str::to_string));
                if applied.as_deref() != Some(coupon_id.as_str()) || sale as i64 >= list {
                    continue;
                }
                out.entry(l.trade_id).or_insert(Discounted {
                    contract_address: l.contract_address.to_ascii_lowercase(),
                    item_id,
                    creator: l.creator,
                    name: l.name,
                    thumbnail: l.thumbnail,
                    category: l.category,
                    rarity: l.rarity,
                    network: l.network.wire().to_string(),
                    list_price: list,
                    sale_price: sale as i64,
                });
            }
            skip += SHOP_MAX_PAGE_SIZE;
            if skip >= total {
                break;
            }
        }
    }
    Ok(out.into_values().collect())
}

pub fn spawn_job(state: &Arc<MarketplaceState>, shop_base_url: String, jobs: &Jobs) {
    let state = state.clone();
    jobs.spawn_periodic_leased(
        "favorite-discount-notifications",
        PeriodicCfg::every(INTERVAL)
            .with_jitter(Duration::from_secs(30))
            .with_error_backoff(Duration::from_secs(1800)),
        move |ctx| {
            let (state, base) = (state.clone(), shop_base_url.clone());
            async move {
                let catalogue = state.clone();
                let listings = move |c: PendingCoupon| -> BoxFuture<'static, _> {
                    let state = catalogue.clone();
                    Box::pin(async move { catalogue_listings(&state, &c).await })
                };
                let (coupons, sent) =
                    run_once(&state.pool, &base, crate::shared::now_utc(), &listings).await?;
                ctx.progress(format!("{coupons} coupon(s), {sent} notified"));
                Ok(())
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as D;
    use chrono::SubsecRound;

    fn listing(contract: &str, item: &str, creator: &str) -> Discounted {
        Discounted {
            contract_address: contract.into(),
            item_id: item.into(),
            creator: creator.into(),
            name: if item == "1" {
                "Hat".into()
            } else {
                String::new()
            },
            thumbnail: "https://example.com/t.png".into(),
            category: "wearable".into(),
            rarity: if item == "1" {
                "epic".into()
            } else {
                String::new()
            },
            network: "MATIC".into(),
            list_price: 10,
            sale_price: 7,
        }
    }

    /// History is never announced; a coupon that starts later tells each
    /// favoriter (not its creator) within the daily cap, once; a later coupon
    /// on the same items is under the cap and the item cooldown; a coupon the
    /// catalogue does not show yet is deferred.
    #[tokio::test]
    async fn a_starting_coupon_tells_favoriters_within_the_caps_once() {
        let Some(db) = crate::market::tests::support::OwnDb::create("favdisc").await else {
            return;
        };
        let pool = &db.pool;
        let contract = "0x00000000000000000000000000000000000000c0";
        let creator = "0x00000000000000000000000000000000000000cc";
        let (u, v) = (
            "0x00000000000000000000000000000000000000a1",
            "0x00000000000000000000000000000000000000a2",
        );
        let t0 = crate::shared::now_utc().trunc_subsecs(3);
        let coupon = |eff: NaiveDateTime, ppm: i32| async move {
            let sig = format!("0x{}", Uuid::new_v4().simple());
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO shop.coupons (network, chain_id, signer, signature, hashed_signature, state_key, coupon_manager, coupon_address, checks, \
                    discount_type, discount_ppm, root, collections, effective_since, expires_at, created_at) \
                 VALUES ('MATIC', 137, $1, $2, $2, $2, '0x00', '0x00', '{\"uses\": 10}'::jsonb, 1, $3, '0x00', ARRAY[$4], $5, $5 + interval '3 days', $5) RETURNING id",
            )
            .bind(creator)
            .bind(sig)
            .bind(ppm)
            .bind(contract)
            .bind(eff)
            .fetch_one(pool)
            .await
            .unwrap()
        };
        let old = coupon(t0 - D::hours(1), 300_000).await;
        let items = ["1", "2", "3", "4"];
        let fake = move |c: PendingCoupon| -> BoxFuture<'static, anyhow::Result<Vec<Discounted>>> {
            let found = if c.discount_ppm == 50_000 {
                Vec::new()
            } else {
                items
                    .iter()
                    .map(|i| listing(contract, i, creator))
                    .collect()
            };
            Box::pin(async move { Ok(found) })
        };
        assert_eq!(
            run_once(pool, "https://x/shop/", t0, &fake).await.unwrap(),
            (0, 0)
        );

        let list: Uuid = sqlx::query_scalar(
            "INSERT INTO shop.lists (name, user_address) VALUES ('Favorites', $1) RETURNING id",
        )
        .bind(u)
        .fetch_one(pool)
        .await
        .unwrap();
        for (n, item) in items.iter().enumerate() {
            sqlx::query("INSERT INTO shop.picks (item_id, user_address, list_id, created_at) VALUES ($1, $2, $3, $4)")
                .bind(item_key(contract, item))
                .bind(u)
                .bind(list)
                .bind(t0 - D::minutes(n as i64))
                .execute(pool)
                .await
                .unwrap();
        }
        for who in [v, creator] {
            let l: Uuid = sqlx::query_scalar(
                "INSERT INTO shop.lists (name, user_address) VALUES ('Favorites', $1) RETURNING id",
            )
            .bind(who)
            .fetch_one(pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO shop.picks (item_id, user_address, list_id) VALUES ($1, $2, $3)",
            )
            .bind(item_key(contract, "1"))
            .bind(who)
            .bind(l)
            .execute(pool)
            .await
            .unwrap();
        }
        let fresh = coupon(t0 + D::minutes(1), 300_000).await;
        let t1 = t0 + D::minutes(2);
        assert_eq!(
            run_once(pool, "https://x/shop/", t1, &fake).await.unwrap(),
            (1, 4)
        );
        assert_eq!(
            run_once(pool, "https://x/shop/", t1 + D::minutes(5), &fake)
                .await
                .unwrap(),
            (0, 0),
            "a coupon is announced once"
        );
        let rows: Vec<(String, Value, Uuid)> = sqlx::query_as(
            "SELECT address, metadata, id FROM social.notifications WHERE type = $1 ORDER BY address, metadata->>'itemId'",
        )
        .bind(KIND)
        .fetch_all(pool)
        .await
        .unwrap();
        let got: Vec<(&str, &str)> = rows
            .iter()
            .map(|(a, m, _)| (a.as_str(), m["itemId"].as_str().unwrap()))
            .collect();
        assert_eq!(
            got,
            vec![(u, "1"), (u, "2"), (u, "3"), (v, "1")],
            "the newest favorites, three a day"
        );
        assert_eq!(
            rows[0].2,
            notification_id(fresh, u, &item_key(contract, "1"))
        );
        assert_eq!(
            rows[0].1,
            json!({"image": "https://example.com/t.png", "category": "wearable", "rarity": "epic", "nftName": "Hat",
                   "contractAddress": contract, "itemId": "1", "link": format!("https://x/shop/item/{contract}/1"),
                   "discountPct": 30, "listPrice": "10", "salePrice": "7",
                   "endsAt": (t0 + D::minutes(1) + D::days(3)).and_utc().timestamp_millis(),
                   "title": "A favorite is on sale", "description": "Hat is 30% off.", "network": "MATIC"})
        );
        assert!(rows[1].1.get("rarity").is_none() && rows[1].1.get("nftName").is_none());
        assert_eq!(rows[1].1["description"], "An item you saved is 30% off.");

        coupon(t1 + D::minutes(10), 400_000).await;
        assert_eq!(
            run_once(pool, "https://x/shop/", t1 + D::minutes(11), &fake)
                .await
                .unwrap(),
            (1, 0),
            "the day's cap and the item cooldown hold across coupons"
        );
        let waiting = coupon(t1 + D::minutes(20), 50_000).await;
        assert_eq!(
            run_once(pool, "https://x/shop/", t1 + D::minutes(21), &fake)
                .await
                .unwrap(),
            (1, 0)
        );
        let state: (Option<NaiveDateTime>, i32) = sqlx::query_as(
            "SELECT notified_at, attempts FROM shop.coupon_favorites_announcement WHERE coupon_id = $1",
        )
        .bind(waiting)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(state, (None, 1), "deferred while the catalogue catches up");
        let closed: Option<NaiveDateTime> = sqlx::query_scalar(
            "SELECT notified_at FROM shop.coupon_favorites_announcement WHERE coupon_id = $1",
        )
        .bind(old)
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(closed.is_some(), "history is closed, not announced");
        db.drop().await;
    }

    #[test]
    fn the_daily_budget_takes_the_newest_favorites() {
        let t = crate::shared::now_utc();
        let c = |user: &str, item: &str, age: i64| Candidate {
            user: user.into(),
            favorited_at: t - D::minutes(age),
            listing: listing("0xc", item, "0xcc"),
        };
        let today = HashMap::from([("0xa".to_string(), 2)]);
        let already = HashSet::from([("0xb".to_string(), item_key("0xc", "1"))]);
        let picked = select(
            vec![
                c("0xa", "1", 5),
                c("0xa", "2", 1),
                c("0xb", "1", 1),
                c("0xb", "2", 2),
                c("0xb", "2", 3),
            ],
            &already,
            &today,
        );
        let got: Vec<(&str, &str)> = picked
            .iter()
            .map(|c| (c.user.as_str(), c.listing.item_id.as_str()))
            .collect();
        assert_eq!(got, vec![("0xa", "2"), ("0xb", "2")]);
        assert_eq!(discount_pct(250_000), 25);
        assert_eq!(discount_pct(55_000), 6);
    }
}
