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
//! listing open. So does a token in the Rentals contract whose rent the
//! rentals index has not reached: the latest indexed rent names who the
//! contract holds the token for only when it is no older than the token's
//! arrival there (the job compares blocks, the refresh route the rent's
//! block time with the asset row's `updated_at`). Every write is guarded by
//! `status = 'open'`, so a rent the fold recorded meanwhile wins.

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
/// `custody_lessor` the lessor of the rent the Rentals contract holds the
/// asset under, read only when the owner is that contract (`None`: unknown,
/// which includes a latest indexed rent older than the asset's arrival).
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

/// The asset's latest indexed rent at a venue: its lessor (`signer`), block
/// and block time. The arguments are the SQL for the venue, the asset's
/// contract and its token id.
macro_rules! latest_rent_sql {
    ($venue:literal, $contract:literal, $token:literal) => {
        concat!(
            "SELECT lower(x.signer) AS signer, x.block_number, x.block_time FROM shop.rental_event x \
             WHERE x.network = 'ethereum' AND x.rentals_address = ",
            $venue,
            " AND x.kind = 'rented' AND x.contract_address = ",
            $contract,
            " AND x.token_id = ",
            $token,
            " ORDER BY x.block_number DESC, x.log_index DESC LIMIT 1"
        )
    };
}

/// The lessor the Rentals contract (`$1`) holds the asset (`$2`, `$3`) for,
/// the legacy `isAssetControlledByLessor` read: the lessor of the latest
/// indexed rent, or no row when that rent is older than the asset row the
/// route read (`$4`, its `updated_at`). A transfer into the contract stamps
/// that row, and a rent lands in the block of its transfer or later, so an
/// older rent means the rentals index has not reached the one the contract
/// holds the asset under.
const CUSTODY_SQL: &str = concat!(
    "SELECT ren.signer FROM (",
    latest_rent_sql!("$1", "$2", "$3::numeric"),
    ") ren WHERE ren.block_time >= $4"
);

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
/// than the listing (`$5`, the head's block time). `custody_lessor` is the
/// lessor of the latest indexed rent, NULL unless that rent's block is at or
/// after the latest folded transfer's: both indexes land whole blocks, and a
/// rent moves the token into the Rentals contract in its own transaction or
/// finds it there, so an older rent is not the one the contract holds the
/// token under.
const OWNERSHIP_SQL: &str = concat!(
    "
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
    SELECT e.block_number, e.block_time FROM land_authz.authz_event e
    WHERE e.token_address = r.contract_address AND e.token_id = r.token_id AND e.kind = 'transfer'
      AND e.block_number <= $3
    ORDER BY e.block_number DESC, e.log_index DESC LIMIT 1
) lt ON true
LEFT JOIN LATERAL (",
    latest_rent_sql!(
        "r.rental_contract_address",
        "r.contract_address",
        "r.token_id"
    ),
    ") ren ON $4 AND ren.block_number >= lt.block_number
WHERE r.status = 'open' AND r.contract_address IN ($1, $2)"
);

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
/// when the reader found no such token, which decides nothing). The asset
/// row counts only when its `updated_at` is newer than the stored
/// `rental_assets.nft_updated_at`, which the route refreshes after this.
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
                r.contract_address, r.token_id::text AS token_id, \
                extract(epoch FROM a.nft_updated_at)::bigint AS stored_at \
         FROM signatures.rentals r JOIN signatures.rental_listings l ON l.id = r.id \
         JOIN signatures.rental_assets a ON a.contract_address = r.contract_address AND a.token_id = r.token_id \
         WHERE r.id = $1",
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
    // The asset row decides only when it is newer than the stored one: a
    // reader answering from an older block must not close the listing.
    let stored_at: i64 = row.get("stored_at");
    let nft = nft.filter(|n| n.updated_at > stored_at);
    let owner = nft.map(|n| n.owner_address.as_str());
    let custody: Option<String> = match nft {
        Some(n) if rentals_ready && n.owner_address.eq_ignore_ascii_case(&venue) => {
            sqlx::query_scalar(CUSTODY_SQL)
                .bind(&venue)
                .bind(row.get::<String, _>("contract_address"))
                .bind(row.get::<String, _>("token_id"))
                .bind(n.updated_at)
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
            // Newer than the seeded `nft_updated_at`.
            updated_at: Utc::now().timestamp() + 60,
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
        let older = LandNft {
            updated_at: 1_600_000_000,
            ..row(OTHER, "parcel", None)
        };
        sync_listing(pool, held, Some(&older), now).await.unwrap();
        assert_eq!(
            state_of(pool, held).await,
            open(),
            "an asset row older than the stored one decides nothing"
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

        // A rent of the expired listing indexed after its cancellation
        // executes it.
        let rent = index::decode_log(
            &index::tests::rented_log(
                LAND_REGISTRY_MAINNET,
                alloy_primitives::U256::from(2u8),
                LESSOR,
                OTHER,
                7,
                alloy_primitives::U256::from(5u8),
                &[3u8; 65],
                50,
                1,
            ),
            1_700_000_000,
        )
        .unwrap()
        .unwrap();
        let mut conn = pool.acquire().await.unwrap();
        index::insert_events(
            &mut conn,
            crate::market::chain_walk::ChainNet::Ethereum,
            &[rent],
        )
        .await
        .unwrap();
        drop(conn);
        sync_listing(pool, stale, None, Utc::now()).await.unwrap();
        assert_eq!(state_of(pool, stale).await, ("executed".into(), None));
        db.drop().await;
    }
    const TENANT: &str = "0x00000000000000000000000000000000000000b2";

    /// Indexes a rent of LAND `token` by `lessor` at `block` (its log index
    /// too), under a signature no seeded listing carries.
    async fn index_rent(pool: &PgPool, token: u8, lessor: &str, block: u64, block_time: i64) {
        let rent = index::decode_log(
            &index::tests::rented_log(
                LAND_REGISTRY_MAINNET,
                alloy_primitives::U256::from(token),
                lessor,
                TENANT,
                7,
                alloy_primitives::U256::from(5u8),
                &[0xee, block as u8, token],
                block,
                block,
            ),
            block_time,
        )
        .unwrap()
        .unwrap();
        let mut conn = pool.acquire().await.unwrap();
        index::insert_events(
            &mut conn,
            crate::market::chain_walk::ChainNet::Ethereum,
            &[rent],
        )
        .await
        .unwrap();
    }

    async fn publish_rentals_index(pool: &PgPool) {
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history) \
             VALUES ('rentals', 'ethereum', 1, 50, 50, timezone('UTC', now()), true)",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    /// Publishes the land-authz index at `block`, or moves it there.
    async fn publish_land_index(pool: &PgPool, block: i64) {
        sqlx::query(
            "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) \
             VALUES ($1, $2, $3, $4, $4, timezone('UTC', now()), true) \
             ON CONFLICT (network) DO UPDATE SET last_block = $4, folded_block = $4, folded_at = EXCLUDED.folded_at",
        )
        .bind(LAND_MAINNET.network)
        .bind(LAND_REGISTRY_MAINNET)
        .bind(ESTATE_REGISTRY_MAINNET)
        .bind(block)
        .execute(pool)
        .await
        .unwrap();
    }

    /// The job's custody leg: a token in the Rentals contract keeps its
    /// listing while the rentals index is not published, and once it is, the
    /// latest indexed rent decides only when its block is at or after the
    /// token's transfer into the contract. An older rent by another lessor
    /// (the token arrived through a rent the index has not reached, such as
    /// an accepted offer) leaves the listing open.
    #[tokio::test]
    async fn the_pass_reads_custody_from_a_rent_no_older_than_the_arrival() {
        let Some(db) = OwnedDatabase::new("rental_custody", Schema::Full, 2).await else {
            return;
        };
        let pool = &db.pool;
        let land = LAND_REGISTRY_MAINNET;
        let held = listing(pool, land, "1", 1).await;
        let rented_by_other = listing(pool, land, "2", 2).await;
        let unindexed = listing(pool, land, "3", 3).await;
        let later = Utc::now().timestamp() + 3_600;
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "INSERT INTO land_authz.token_right (token_address, token_id, owner, updated_block, updated_log) VALUES \
                ('{land}', 1, '{RENTALS}', 40, 0), ('{land}', 2, '{RENTALS}', 40, 1), ('{land}', 3, '{RENTALS}', 40, 2); \
             INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) VALUES \
                (40, 0, {later}, '{land}', 'transfer', 1, '{RENTALS}'), (40, 1, {later}, '{land}', 'transfer', 2, '{RENTALS}'), \
                (40, 2, {later}, '{land}', 'transfer', 3, '{RENTALS}')"
        )))
        .execute(pool)
        .await
        .unwrap();
        publish_land_index(pool, 40).await;
        // The rent of each transfer's block, and for token 3 only a rent
        // from before the previous lessor's claim.
        index_rent(pool, 1, LESSOR, 40, later).await;
        index_rent(pool, 2, OTHER, 41, later).await;
        index_rent(pool, 3, OTHER, 10, 1_600_000_000).await;

        let pass = run_pass(pool, Some(LAND_MAINNET), Utc::now())
            .await
            .unwrap();
        assert_eq!(pass.moved, Some(0), "the rentals index is not published");
        assert_eq!(state_of(pool, rented_by_other).await, open());

        publish_rentals_index(pool).await;
        let pass = run_pass(pool, Some(LAND_MAINNET), Utc::now())
            .await
            .unwrap();
        assert_eq!(pass.moved, Some(1));
        assert_eq!(
            state_of(pool, rented_by_other).await,
            cancelled(REASON_TRANSFERRED),
            "held for another lessor"
        );
        assert_eq!(state_of(pool, held).await, open(), "held for the lessor");
        assert_eq!(
            state_of(pool, unindexed).await,
            open(),
            "a rent older than the arrival does not say who it is held for"
        );

        // The rent the token arrived under, once indexed, keeps the listing.
        index_rent(pool, 3, LESSOR, 42, later).await;
        let pass = run_pass(pool, Some(LAND_MAINNET), Utc::now())
            .await
            .unwrap();
        assert_eq!(pass.moved, Some(0));
        assert_eq!(state_of(pool, unindexed).await, open());
        db.drop().await;
    }

    /// A burned token (an indexed row without an owner) closes its listing
    /// once the folded transfer is newer than the listing. An estate
    /// without parcels closes only once the folded head is newer than the
    /// listing: a head at or before its creation decides nothing.
    #[tokio::test]
    async fn the_pass_cancels_a_burn_and_waits_for_a_head_past_the_listing() {
        let Some(db) = OwnedDatabase::new("rental_burn", Schema::Full, 2).await else {
            return;
        };
        let pool = &db.pool;
        let (land, estate) = (LAND_REGISTRY_MAINNET, ESTATE_REGISTRY_MAINNET);
        let burned = listing(pool, land, "1", 1).await;
        let empty = listing(pool, estate, "10", 2).await;
        let empty_at_head = listing(pool, estate, "11", 3).await;
        let later = Utc::now().timestamp() + 3_600;
        let before = 1_600_000_000_i64;
        // Created in the very second of the head the first pass folds.
        sqlx::query("UPDATE signatures.rentals SET created_at = to_timestamp($2) AT TIME ZONE 'UTC' WHERE id = $1")
            .bind(empty_at_head)
            .bind(before as f64)
            .execute(pool)
            .await
            .unwrap();
        let zero = alloy_primitives::Address::ZERO.to_string();
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "INSERT INTO land_authz.token_right (token_address, token_id, owner, updated_block, updated_log) VALUES \
                ('{land}', 1, NULL, 5, 0), ('{estate}', 10, '{LESSOR}', 1, 0), ('{estate}', 11, '{LESSOR}', 1, 1); \
             INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) VALUES \
                (2, 0, {before}, '{land}', 'transfer', 1, '{LESSOR}'), (5, 0, {later}, '{land}', 'transfer', 1, '{zero}')"
        )))
        .execute(pool)
        .await
        .unwrap();

        publish_land_index(pool, 2).await;
        let pass = run_pass(pool, Some(LAND_MAINNET), Utc::now())
            .await
            .unwrap();
        assert_eq!(pass.moved, Some(0));
        assert_eq!(
            state_of(pool, burned).await,
            open(),
            "no folded transfer after the listing"
        );
        assert_eq!(
            state_of(pool, empty).await,
            open(),
            "the folded head is older than the listing"
        );
        assert_eq!(
            state_of(pool, empty_at_head).await,
            open(),
            "the folded head is as old as the listing"
        );

        publish_land_index(pool, 5).await;
        let pass = run_pass(pool, Some(LAND_MAINNET), Utc::now())
            .await
            .unwrap();
        assert_eq!(pass.moved, Some(3));
        assert_eq!(state_of(pool, burned).await, cancelled(REASON_TRANSFERRED));
        assert_eq!(state_of(pool, empty).await, cancelled(REASON_DISSOLVED));
        assert_eq!(
            state_of(pool, empty_at_head).await,
            cancelled(REASON_DISSOLVED)
        );
        db.drop().await;
    }

    /// The refresh route's custody read: with the asset row naming the
    /// Rentals contract, the latest indexed rent decides only when it is no
    /// older than that row. An older rent by another lessor (the rentals
    /// index has not reached the rent the token arrived under) leaves the
    /// listing open.
    #[tokio::test]
    async fn the_refresh_reads_custody_from_a_rent_no_older_than_the_asset_row() {
        let Some(db) = OwnedDatabase::new("rental_refresh_custody", Schema::Full, 2).await else {
            return;
        };
        let pool = &db.pool;
        let arrived = Utc::now().timestamp() + 60;
        let in_custody = LandNft {
            nft_id: "x".into(),
            category: "parcel".into(),
            owner_address: RENTALS.into(),
            search_text: String::new(),
            distance_to_plaza: None,
            adjacent_to_road: None,
            estate_size: None,
            created_at: 0,
            updated_at: arrived,
        };
        publish_rentals_index(pool).await;

        let unindexed = listing(pool, LAND_REGISTRY_MAINNET, "1", 1).await;
        index_rent(pool, 1, OTHER, 10, arrived - 86_400).await;
        sync_listing(pool, unindexed, Some(&in_custody), Utc::now())
            .await
            .unwrap();
        assert_eq!(
            state_of(pool, unindexed).await,
            open(),
            "a rent older than the asset row does not say who it is held for"
        );
        index_rent(pool, 1, OTHER, 40, arrived).await;
        sync_listing(pool, unindexed, Some(&in_custody), Utc::now())
            .await
            .unwrap();
        assert_eq!(
            state_of(pool, unindexed).await,
            cancelled(REASON_TRANSFERRED),
            "held for another lessor"
        );

        let held = listing(pool, LAND_REGISTRY_MAINNET, "2", 2).await;
        index_rent(pool, 2, LESSOR, 41, arrived).await;
        sync_listing(pool, held, Some(&in_custody), Utc::now())
            .await
            .unwrap();
        assert_eq!(state_of(pool, held).await, open(), "held for the lessor");
        db.drop().await;
    }
}
