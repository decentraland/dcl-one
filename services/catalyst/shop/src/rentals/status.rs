// from: signatures-server@70fb704c:src/ports/rentals/component.ts (refreshRentalListing, isAssetControlledByLessor, updateMetadata's dissolved-estate and owner cancellations, updateRentalsListings' expiry close)
//! The rental-listing cancellations the Rentals contract's logs do not
//! carry, the old signatures-server's metadata and rentals jobs:
//!
//! - `transferred`: the asset's owner is no longer the lessor, unless the
//!   Rentals contract holds it for the lessor (its latest rent names them);
//! - `dissolved`: an estate without parcels;
//! - `expired`: an open listing past its `expiration`.
//!
//! The leased `rental-listing-status` job runs [`run_pass`] every minute:
//! expiry on the database clock it is given, ownership and dissolution from
//! the land-authz index (`land_authz.token_right`, `estate_land`,
//! `authz_event`) only while that index is published. The refresh route
//! ([`sync_listing`]) applies the same rules to one listing from the asset
//! row it has just read, after folding the listing's own Rentals logs.
//!
//! An unknown never cancels, since a cancellation does not reopen: no
//! published index, no indexed row, or an index that has not yet seen past
//! the listing's creation (the create route checks the owner against the
//! marketplace subgraph, which the land-authz index can trail) leave the
//! listing open. Every write is guarded by `status = 'open'`, so a rent the
//! fold recorded meanwhile wins.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::index;
use super::nft::LandNft;
use crate::jobs::{Jobs, PeriodicCfg};
use crate::land_authz::LandNetwork;
use crate::market::MarketplaceState;

pub const REASON_TRANSFERRED: &str = "transferred";
pub const REASON_DISSOLVED: &str = "dissolved";
pub const REASON_EXPIRED: &str = "expired";

const POLL_SECS: u64 = 60;

/// Why an open listing must close, or `None` to keep it open.
///
/// `owner` is the asset's current owner (`None`: unknown);
/// `custody_lessor` the lessor of the asset's latest rent, read only when
/// the owner is the Rentals contract (`None`: unknown, or no rent).
pub fn cancel_reason(
    expired: bool,
    dissolved: bool,
    owner: Option<&str>,
    lessor: &str,
    rentals_contract: &str,
    custody_lessor: Option<&str>,
) -> Option<&'static str> {
    if dissolved {
        return Some(REASON_DISSOLVED);
    }
    if let Some(owner) = owner
        && !owner.eq_ignore_ascii_case(lessor)
    {
        // The Rentals contract holds a rented asset on the lessor's behalf.
        let held_for_lessor = owner.eq_ignore_ascii_case(rentals_contract)
            && custody_lessor.is_none_or(|l| l.eq_ignore_ascii_case(lessor));
        if !held_for_lessor {
            return Some(REASON_TRANSFERRED);
        }
    }
    expired.then_some(REASON_EXPIRED)
}

/// The lessor of the asset's latest indexed rent at `venue`, the legacy
/// `isAssetControlledByLessor` read.
const CUSTODY_SQL: &str = "SELECT lower(x.signer) FROM shop.rental_event x \
     WHERE x.network = 'ethereum' AND x.rentals_address = $1 AND x.kind = 'rented' \
       AND x.contract_address = $2 AND x.token_id = $3::numeric \
     ORDER BY x.block_number DESC, x.log_index DESC LIMIT 1";

async fn cancel(
    pool: &PgPool,
    cancels: &[(Uuid, &'static str)],
    now: NaiveDateTime,
) -> Result<u64, sqlx::Error> {
    if cancels.is_empty() {
        return Ok(0);
    }
    let ids: Vec<Uuid> = cancels.iter().map(|c| c.0).collect();
    let reasons: Vec<&str> = cancels.iter().map(|c| c.1).collect();
    Ok(sqlx::query(
        "UPDATE signatures.rentals r SET status = 'cancelled', cancel_reason = u.reason, updated_at = $3 \
         FROM UNNEST($1::uuid[], $2::text[]) AS u(id, reason) \
         WHERE r.id = u.id AND r.status = 'open'",
    )
    .bind(&ids)
    .bind(&reasons)
    .bind(now)
    .execute(pool)
    .await?
    .rows_affected())
}

/// Closes every open listing past its expiration.
pub async fn cancel_expired(pool: &PgPool, now: DateTime<Utc>) -> Result<u64, sqlx::Error> {
    let now = now.naive_utc();
    Ok(sqlx::query(
        "UPDATE signatures.rentals SET status = 'cancelled', cancel_reason = 'expired', updated_at = $1 \
         WHERE status = 'open' AND expiration < $1",
    )
    .bind(now)
    .execute(pool)
    .await?
    .rows_affected())
}

/// Every open listing of the index's registries whose asset the index
/// holds, with what decides it. `$1`/`$2` the LAND and Estate registries,
/// `$3` the folded block, `$4` whether the rentals index is published.
/// `moved` is whether the latest folded transfer came after the listing;
/// `dissolved` needs the estate's row, no parcels, and a folded head newer
/// than the listing (`$5`, the head's block time).
const OWNERSHIP_SQL: &str = "
SELECT r.id, l.lessor, r.rental_contract_address, t.owner,
       COALESCE(to_timestamp(lt.block_time) AT TIME ZONE 'UTC' > r.created_at, false) AS moved,
       (r.contract_address = $2
        AND to_timestamp($5) AT TIME ZONE 'UTC' > r.created_at
        AND NOT EXISTS (SELECT 1 FROM land_authz.estate_land el WHERE el.estate_token_id = r.token_id)) AS dissolved,
       ren.signer AS custody_lessor
FROM signatures.rentals r
JOIN signatures.rental_listings l ON l.id = r.id
JOIN land_authz.token_right t ON t.token_address = r.contract_address AND t.token_id = r.token_id
LEFT JOIN LATERAL (
    SELECT e.block_time FROM land_authz.authz_event e
    WHERE e.token_address = r.contract_address AND e.token_id = r.token_id AND e.kind = 'transfer'
      AND e.block_number <= $3
    ORDER BY e.block_number DESC, e.log_index DESC LIMIT 1
) lt ON true
LEFT JOIN LATERAL (
    SELECT lower(x.signer) AS signer FROM shop.rental_event x
    WHERE $4 AND x.network = 'ethereum' AND x.rentals_address = r.rental_contract_address AND x.kind = 'rented'
      AND x.contract_address = r.contract_address AND x.token_id = r.token_id
    ORDER BY x.block_number DESC, x.log_index DESC LIMIT 1
) ren ON true
WHERE r.status = 'open' AND r.contract_address IN ($1, $2)";

/// Closes the open listings whose asset changed owner or whose estate was
/// dissolved, from the land-authz index of `land`; `None` while that index
/// is not published.
pub async fn cancel_moved(
    pool: &PgPool,
    land: LandNetwork,
    now: DateTime<Utc>,
) -> Result<Option<u64>, sqlx::Error> {
    let rentals_ready = index::ready(pool, now).await;
    let Some(mut snapshot) = crate::market::nft_reads::land_snapshot(pool, land).await else {
        return Ok(None);
    };
    let Some(folded) = snapshot.cursor.folded_block else {
        return Ok(None);
    };
    let head_time: Option<i64> = sqlx::query_scalar(
        "SELECT block_time FROM land_authz.authz_event WHERE block_number <= $1 \
         ORDER BY block_number DESC, log_index DESC LIMIT 1",
    )
    .bind(folded)
    .fetch_optional(&mut *snapshot.tx)
    .await?;
    let rows = sqlx::query(OWNERSHIP_SQL)
        .bind(land.land_registry.to_ascii_lowercase())
        .bind(land.estate_registry.to_ascii_lowercase())
        .bind(folded)
        .bind(rentals_ready)
        .bind(head_time.unwrap_or(0) as f64)
        .fetch_all(&mut *snapshot.tx)
        .await?;
    snapshot.tx.rollback().await?;
    let cancels: Vec<(Uuid, &'static str)> = rows
        .iter()
        .filter_map(|r| {
            let lessor: String = r.get("lessor");
            let venue: String = r.get("rental_contract_address");
            let owner: Option<String> = r.get("owner");
            let moved: bool = r.get("moved");
            let custody: Option<String> = r.get("custody_lessor");
            // A burned token (no owner) after the listing is a transfer away.
            let owner = match (owner, moved) {
                (_, false) => None,
                (Some(o), true) => Some(o),
                (None, true) => Some(alloy_primitives::Address::ZERO.to_string()),
            };
            cancel_reason(
                false,
                r.get("dissolved"),
                owner.as_deref(),
                &lessor,
                &venue,
                custody.as_deref(),
            )
            .map(|reason| (r.get("id"), reason))
        })
        .collect();
    cancel(pool, &cancels, now.naive_utc()).await.map(Some)
}

/// What one pass did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pass {
    pub expired: u64,
    /// `None` while the land-authz index is not published.
    pub moved: Option<u64>,
}

/// One pass: expiry, then transfers and dissolved estates.
pub async fn run_pass(
    pool: &PgPool,
    land: Option<LandNetwork>,
    now: DateTime<Utc>,
) -> Result<Pass, sqlx::Error> {
    let expired = cancel_expired(pool, now).await?;
    let moved = match land {
        Some(land) => cancel_moved(pool, land, now).await?,
        None => None,
    };
    Ok(Pass { expired, moved })
}

/// The refresh route's status sync of one listing: its own Rentals logs
/// first (while the rentals index is published), then, if it is still
/// open, expiry and the asset row the route has just read (`nft`; `None`
/// when the reader found no such token, which decides nothing).
pub async fn sync_listing(
    pool: &PgPool,
    id: Uuid,
    nft: Option<&LandNft>,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let venue: Option<String> =
        sqlx::query_scalar("SELECT rental_contract_address FROM signatures.rentals WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    let Some(venue) = venue else {
        return Ok(());
    };
    let rentals_ready = index::ready(pool, now).await;
    if rentals_ready {
        index::fold_listing(pool, &venue, id, now).await?;
    }
    let row = sqlx::query(
        "SELECT r.status::text AS status, r.expiration < $2 AS expired, l.lessor, \
                r.contract_address, r.token_id::text AS token_id \
         FROM signatures.rentals r JOIN signatures.rental_listings l ON l.id = r.id WHERE r.id = $1",
    )
    .bind(id)
    .bind(now.naive_utc())
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(());
    };
    if row.get::<String, _>("status") != "open" {
        return Ok(());
    }
    let lessor: String = row.get("lessor");
    let owner = nft.map(|n| n.owner_address.as_str());
    let custody: Option<String> = match owner {
        Some(o) if rentals_ready && o.eq_ignore_ascii_case(&venue) => {
            sqlx::query_scalar(CUSTODY_SQL)
                .bind(&venue)
                .bind(row.get::<String, _>("contract_address"))
                .bind(row.get::<String, _>("token_id"))
                .fetch_optional(pool)
                .await?
        }
        _ => None,
    };
    let dissolved = nft.is_some_and(|n| n.category == "estate" && n.estate_size == Some(0));
    if let Some(reason) = cancel_reason(
        row.get("expired"),
        dissolved,
        owner,
        &lessor,
        &venue,
        custody.as_deref(),
    ) {
        cancel(pool, &[(id, reason)], now.naive_utc()).await?;
    }
    Ok(())
}

/// Attaches the leased `rental-listing-status` job: [`run_pass`] every
/// minute over the land-authz index of `ETH_NETWORK`.
pub fn spawn(state: &Arc<MarketplaceState>, jobs: &Jobs) {
    let pool = state.pool.clone();
    let land = state.land_network();
    jobs.spawn_periodic_leased(
        "rental-listing-status",
        PeriodicCfg::every(Duration::from_secs(POLL_SECS))
            .with_jitter(Duration::from_secs(10))
            .with_error_backoff(Duration::from_secs(600)),
        move |ctx| {
            let pool = pool.clone();
            async move {
                let pass = run_pass(&pool, land, Utc::now()).await?;
                if pass.expired > 0 || pass.moved.unwrap_or(0) > 0 {
                    tracing::info!(expired = pass.expired, moved = ?pass.moved, "rental listings cancelled");
                }
                ctx.progress(match pass.moved {
                    Some(m) => format!("{} expired, {m} transferred or dissolved cancelled", pass.expired),
                    None => format!(
                        "{} expired cancelled; transfers wait for the land-authz index",
                        pass.expired
                    ),
                });
                Ok(())
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const LESSOR: &str = "0x00000000000000000000000000000000000000a1";
    const OTHER: &str = "0x00000000000000000000000000000000000000c3";
    const RENTALS: &str = "0x3a1469499d0be105d4f77045ca403a5f6dc2f3f5";

    #[test]
    fn the_reasons_follow_the_legacy_checks() {
        assert_eq!(
            cancel_reason(false, false, Some(LESSOR), LESSOR, RENTALS, None),
            None
        );
        assert_eq!(
            cancel_reason(
                false,
                false,
                Some(&LESSOR.to_uppercase()),
                LESSOR,
                RENTALS,
                None
            ),
            None,
            "addresses compare case-insensitively"
        );
        assert_eq!(
            cancel_reason(false, false, Some(OTHER), LESSOR, RENTALS, None),
            Some(REASON_TRANSFERRED)
        );
        assert_eq!(
            cancel_reason(false, false, None, LESSOR, RENTALS, None),
            None,
            "an unknown owner never cancels"
        );
        assert_eq!(
            cancel_reason(false, false, Some(RENTALS), LESSOR, RENTALS, Some(LESSOR)),
            None,
            "the Rentals contract holds it for the lessor"
        );
        assert_eq!(
            cancel_reason(false, false, Some(RENTALS), LESSOR, RENTALS, None),
            None,
            "custody unknown keeps it"
        );
        assert_eq!(
            cancel_reason(false, false, Some(RENTALS), LESSOR, RENTALS, Some(OTHER)),
            Some(REASON_TRANSFERRED),
            "held for another lessor"
        );
        assert_eq!(
            cancel_reason(true, true, Some(OTHER), LESSOR, RENTALS, None),
            Some(REASON_DISSOLVED)
        );
        assert_eq!(
            cancel_reason(true, false, Some(LESSOR), LESSOR, RENTALS, None),
            Some(REASON_EXPIRED)
        );
    }

    use crate::db::scratch::{OwnedDatabase, Schema};
    use crate::land_authz::{ESTATE_REGISTRY_MAINNET, LAND_MAINNET, LAND_REGISTRY_MAINNET};
    use crate::rentals::index::tests::seed_listing;

    async fn listing(pool: &PgPool, contract: &str, token: &str, sig: u8) -> Uuid {
        seed_listing(
            pool,
            contract,
            token,
            ["0", "0", "0"],
            &format!("0x{}", hex::encode([sig; 65])),
            LESSOR,
            RENTALS,
        )
        .await
    }

    async fn state_of(pool: &PgPool, id: Uuid) -> (String, Option<String>) {
        sqlx::query_as("SELECT status::text, cancel_reason FROM signatures.rentals WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    fn open() -> (String, Option<String>) {
        ("open".into(), None)
    }

    fn cancelled(reason: &str) -> (String, Option<String>) {
        ("cancelled".into(), Some(reason.into()))
    }

    async fn expire(pool: &PgPool, id: Uuid) {
        sqlx::query("UPDATE signatures.rentals SET expiration = timezone('UTC', now()) - interval '1 minute' WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await
            .unwrap();
    }

    /// Expiry needs no index; an unexpired listing stays open.
    #[tokio::test]
    async fn the_pass_cancels_expired_open_listings() {
        let Some(db) = OwnedDatabase::new("rental_expiry", Schema::Full, 2).await else {
            return;
        };
        let pool = &db.pool;
        let expired = listing(pool, LAND_REGISTRY_MAINNET, "1", 1).await;
        let live = listing(pool, LAND_REGISTRY_MAINNET, "2", 2).await;
        expire(pool, expired).await;
        let pass = run_pass(pool, None, Utc::now()).await.unwrap();
        assert_eq!(
            pass,
            Pass {
                expired: 1,
                moved: None
            }
        );
        assert_eq!(state_of(pool, expired).await, cancelled(REASON_EXPIRED));
        assert_eq!(state_of(pool, live).await, open());
        db.drop().await;
    }

    /// Ownership and dissolution from the land-authz index: a transfer after
    /// the listing cancels it, one before it (the index trailing the
    /// subgraph the create read) does not; an estate without parcels is
    /// dissolved; the Rentals contract's custody and an unindexed token keep
    /// the listing; nothing moves while the index is not published.
    #[tokio::test]
    async fn the_pass_cancels_transferred_assets_and_dissolved_estates() {
        let Some(db) = OwnedDatabase::new("rental_moved", Schema::Full, 2).await else {
            return;
        };
        let pool = &db.pool;
        let (land, estate) = (LAND_REGISTRY_MAINNET, ESTATE_REGISTRY_MAINNET);
        let transferred = listing(pool, land, "1", 1).await;
        let trailing = listing(pool, land, "2", 2).await;
        let kept = listing(pool, land, "3", 3).await;
        let custody = listing(pool, land, "4", 4).await;
        let unindexed = listing(pool, land, "5", 5).await;
        let dissolved = listing(pool, estate, "10", 6).await;
        let whole = listing(pool, estate, "11", 7).await;
        let later = Utc::now().timestamp() + 3_600;
        let before = 1_600_000_000_i64;
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "INSERT INTO land_authz.token_right (token_address, token_id, owner, updated_block, updated_log) VALUES \
                ('{land}', 1, '{OTHER}', 5, 0), ('{land}', 2, '{OTHER}', 2, 0), ('{land}', 3, '{LESSOR}', 5, 1), \
                ('{land}', 4, '{RENTALS}', 5, 2), ('{estate}', 10, '{LESSOR}', 1, 0), ('{estate}', 11, '{LESSOR}', 1, 1), \
                ('{land}', 20, '{estate}', 1, 2); \
             INSERT INTO land_authz.estate_land VALUES (20, 11, 1, 3); \
             INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) VALUES \
                (2, 0, {before}, '{land}', 'transfer', 2, '{OTHER}'), (5, 0, {later}, '{land}', 'transfer', 1, '{OTHER}'), \
                (5, 1, {later}, '{land}', 'transfer', 3, '{LESSOR}'), (5, 2, {later}, '{land}', 'transfer', 4, '{RENTALS}')"
        )))
        .execute(pool)
        .await
        .unwrap();

        let pass = run_pass(pool, Some(LAND_MAINNET), Utc::now())
            .await
            .unwrap();
        assert_eq!(pass.moved, None, "no published land-authz cursor");
        assert_eq!(state_of(pool, transferred).await, open());

        sqlx::query(
            "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) \
             VALUES ($1, $2, $3, 5, 5, timezone('UTC', now()), true)",
        )
        .bind(LAND_MAINNET.network)
        .bind(land)
        .bind(estate)
        .execute(pool)
        .await
        .unwrap();
        let pass = run_pass(pool, Some(LAND_MAINNET), Utc::now())
            .await
            .unwrap();
        assert_eq!(pass.moved, Some(2));
        assert_eq!(
            state_of(pool, transferred).await,
            cancelled(REASON_TRANSFERRED)
        );
        assert_eq!(state_of(pool, dissolved).await, cancelled(REASON_DISSOLVED));
        for id in [trailing, kept, custody, unindexed, whole] {
            assert_eq!(state_of(pool, id).await, open(), "{id}");
        }
        db.drop().await;
    }

    /// The refresh route's sync: the listing's own Rentals logs while the
    /// index is published (a nonce bump), then the asset row it read (a
    /// transfer, a dissolved estate) and expiry; a closed listing is left.
    #[tokio::test]
    async fn the_refresh_syncs_the_status() {
        let Some(db) = OwnedDatabase::new("rental_refresh", Schema::Full, 2).await else {
            return;
        };
        let pool = &db.pool;
        let row = |owner: &str, category: &str, estate_size: Option<i32>| LandNft {
            nft_id: "x".into(),
            category: category.into(),
            owner_address: owner.into(),
            search_text: String::new(),
            distance_to_plaza: None,
            adjacent_to_road: None,
            estate_size,
            created_at: 0,
            updated_at: 0,
        };
        let now = Utc::now();

        let held = listing(pool, LAND_REGISTRY_MAINNET, "1", 1).await;
        sync_listing(pool, held, Some(&row(LESSOR, "parcel", None)), now)
            .await
            .unwrap();
        assert_eq!(state_of(pool, held).await, open());
        sync_listing(pool, held, None, now).await.unwrap();
        assert_eq!(
            state_of(pool, held).await,
            open(),
            "no asset row decides nothing"
        );
        sync_listing(pool, held, Some(&row(OTHER, "parcel", None)), now)
            .await
            .unwrap();
        assert_eq!(state_of(pool, held).await, cancelled(REASON_TRANSFERRED));
        sync_listing(pool, held, Some(&row(OTHER, "estate", Some(0))), now)
            .await
            .unwrap();
        assert_eq!(
            state_of(pool, held).await,
            cancelled(REASON_TRANSFERRED),
            "a closed listing is left"
        );

        let estate = listing(pool, ESTATE_REGISTRY_MAINNET, "10", 2).await;
        sync_listing(pool, estate, Some(&row(LESSOR, "estate", None)), now)
            .await
            .unwrap();
        assert_eq!(
            state_of(pool, estate).await,
            open(),
            "an unknown size is not dissolved"
        );
        sync_listing(pool, estate, Some(&row(LESSOR, "estate", Some(0))), now)
            .await
            .unwrap();
        assert_eq!(state_of(pool, estate).await, cancelled(REASON_DISSOLVED));

        let stale = listing(pool, LAND_REGISTRY_MAINNET, "2", 3).await;
        expire(pool, stale).await;
        sync_listing(pool, stale, Some(&row(LESSOR, "parcel", None)), now)
            .await
            .unwrap();
        assert_eq!(state_of(pool, stale).await, cancelled(REASON_EXPIRED));

        // A nonce bump the rentals index holds counts once it is published.
        let bumped = listing(pool, LAND_REGISTRY_MAINNET, "3", 4).await;
        let bump = index::decode_log(
            &index::tests::asset_index_log(
                LESSOR,
                LAND_REGISTRY_MAINNET,
                alloy_primitives::U256::from(3u8),
                1,
                50,
                0,
            ),
            1_700_000_000,
        )
        .unwrap()
        .unwrap();
        let mut conn = pool.acquire().await.unwrap();
        index::insert_events(
            &mut conn,
            crate::market::chain_walk::ChainNet::Ethereum,
            &[bump],
        )
        .await
        .unwrap();
        drop(conn);
        sync_listing(pool, bumped, Some(&row(LESSOR, "parcel", None)), now)
            .await
            .unwrap();
        assert_eq!(
            state_of(pool, bumped).await,
            open(),
            "the rentals index is not published"
        );
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history) \
             VALUES ('rentals', 'ethereum', 1, 50, 50, timezone('UTC', now()), true)",
        )
        .execute(pool)
        .await
        .unwrap();
        sync_listing(pool, bumped, Some(&row(LESSOR, "parcel", None)), Utc::now())
            .await
            .unwrap();
        assert_eq!(state_of(pool, bumped).await, cancelled(index::CANCEL_NONCE));
        db.drop().await;
    }
}
