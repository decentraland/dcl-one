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
//! the listing's creation leave it open. So does a token in the Rentals
//! contract whose rent the rentals index has not reached: the latest indexed rent names who the
//! contract holds the token for only when it is no older than the token's
//! arrival there (the job compares blocks, the refresh route the rent's
//! block time with the latest transfer time). Every write is guarded by
//! `status = 'open'`, so a rent the fold recorded meanwhile wins.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{PgConnection, PgPool, Row};
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
/// an empty string means the published arrival has no covering rent).
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
    ($venue:literal, $contract:literal, $token:literal, $head:literal, $network:literal) => {
        concat!(
            "SELECT lower(x.signer) AS signer, x.block_number, x.block_time FROM shop.rental_event x \
             WHERE x.network = ",
            $network,
            " AND x.rentals_address = ",
            $venue,
            " AND x.kind = 'rented' AND x.contract_address = ",
            $contract,
            " AND x.token_id = ",
            $token,
            " AND x.block_number <= ",
            $head,
            " ORDER BY x.block_number DESC, x.log_index DESC LIMIT 1"
        )
    };
}

async fn cancel_on(
    conn: &mut PgConnection,
    cancels: &[(Uuid, &'static str)],
    membership: Option<(i64, i64)>,
    now: NaiveDateTime,
) -> Result<u64, sqlx::Error> {
    if cancels.is_empty() {
        return Ok(0);
    }
    let ids: Vec<Uuid> = cancels.iter().map(|c| c.0).collect();
    let reasons: Vec<&str> = cancels.iter().map(|c| c.1).collect();
    Ok(sqlx::query(
        // PostgreSQL rechecks the locked asset after a concurrent fill commits.
        "WITH locked AS MATERIALIZED (
             SELECT r.id, u.reason FROM UNNEST($1::uuid[], $2::text[]) AS u(id, reason)
             JOIN signatures.rentals r ON r.id=u.id
             JOIN signatures.rental_assets a USING (contract_address, token_id)
             WHERE r.status='open' AND (u.reason <> 'dissolved' OR
                 (a.membership_block <= $4 AND r.created_at < to_timestamp($5) AT TIME ZONE 'UTC'))
             ORDER BY a.contract_address, a.token_id FOR UPDATE OF a
         )
         UPDATE signatures.rentals r SET status='cancelled', cancel_reason=u.reason, updated_at=$3
         FROM locked u WHERE r.id=u.id AND r.status='open'",
    )
    .bind(&ids)
    .bind(&reasons)
    .bind(now)
    .bind(membership.map(|(block, _)| block))
    .bind(membership.map(|(_, time)| time as f64))
    .execute(&mut *conn)
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
/// `$3` the folded block, `$4` the published Rentals height from the same snapshot.
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
       COALESCE(
           $4::bigint >= lt.block_number
           AND NOT EXISTS (
               SELECT 1 FROM shop.rental_event pending
               WHERE pending.network=$6 AND pending.rentals_address=r.rental_contract_address
                 AND pending.contract_address=r.contract_address AND pending.token_id=r.token_id
                 AND pending.kind='rented'
                 AND pending.block_number > $4::bigint AND pending.block_number >= lt.block_number
           ), false
       ) AS custody_known,
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
        "r.token_id",
        "$4::bigint",
        "$6"
    ),
    ") ren ON ren.block_number BETWEEN lt.block_number AND $4::bigint AND r.chain_id=$7
WHERE r.status = 'open' AND r.contract_address IN ($1, $2) AND r.chain_id=$7"
);

/// Closes the open listings whose asset changed owner or whose estate was
/// dissolved, from the land-authz index of `land`; `None` while that index
/// is not published.
pub async fn cancel_moved(
    pool: &PgPool,
    land: LandNetwork,
    now: DateTime<Utc>,
) -> Result<Option<u64>, sqlx::Error> {
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
    let Some(chain) = index::RentalsChain::from_land_network(land.network) else {
        return Ok(None);
    };
    let rentals_cursor = crate::market::chain_walk::cursor(
        &mut *snapshot.tx,
        chain.indexer(),
        crate::market::chain_walk::ChainNet::Ethereum,
    )
    .await?;
    let Some(rentals_cursor) = rentals_cursor
        .filter(|cursor| crate::market::chain_walk::is_ready_on(cursor, chain.id(), now))
    else {
        return Ok(None);
    };
    let Some(rentals_head) = rentals_cursor.folded_block else {
        return Ok(None);
    };
    let rows = sqlx::query(OWNERSHIP_SQL)
        .bind(land.land_registry.to_ascii_lowercase())
        .bind(land.estate_registry.to_ascii_lowercase())
        .bind(folded)
        .bind(rentals_head)
        .bind(head_time.unwrap_or(0) as f64)
        .bind(chain.event_network())
        .bind(chain.id())
        .fetch_all(&mut *snapshot.tx)
        .await?;
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
            let custody = if owner
                .as_deref()
                .is_some_and(|owner| owner.eq_ignore_ascii_case(&venue))
                && r.get::<bool, _>("custody_known")
                && custody.is_none()
            {
                Some(String::new())
            } else {
                custody
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
    // Reacquire both publication rows in the write transaction. If either
    // cursor changed since the evidence snapshot (including a rewind), discard
    // the decision and let the next pass read its new publication.
    snapshot.tx.rollback().await?;
    let cursors = PublishedCursorSnapshot {
        land,
        land_cursor: &snapshot.cursor,
        rentals_chain: chain,
        rentals_cursor: &rentals_cursor,
    };
    cancel_if_published_snapshot(
        pool,
        &cursors,
        &cancels,
        head_time.map(|time| (folded, time)),
        now,
        now.naive_utc(),
    )
    .await
}

struct PublishedCursorSnapshot<'a> {
    land: LandNetwork,
    land_cursor: &'a crate::land_authz::indexer::Cursor,
    rentals_chain: index::RentalsChain,
    rentals_cursor: &'a crate::market::chain_walk::Cursor,
}

async fn cancel_if_published_snapshot(
    pool: &PgPool,
    snapshot: &PublishedCursorSnapshot<'_>,
    cancels: &[(Uuid, &'static str)],
    membership: Option<(i64, i64)>,
    now: DateTime<Utc>,
    updated_at: NaiveDateTime,
) -> Result<Option<u64>, sqlx::Error> {
    if cancels.is_empty() {
        return Ok(Some(0));
    }
    let mut tx = pool.begin().await?;
    let current_land: Option<crate::land_authz::indexer::Cursor> = sqlx::query_as(
        "SELECT network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started, updated_at \
         FROM land_authz.index_cursor WHERE network=$1 FOR SHARE",
    )
    .bind(snapshot.land.network)
    .fetch_optional(&mut *tx)
    .await?;
    let current_rentals = index::lock_cursor(&mut tx, snapshot.rentals_chain).await?;
    let land_matches = current_land.as_ref().is_some_and(|cursor| {
        cursor == snapshot.land_cursor
            && cursor
                .land_registry
                .eq_ignore_ascii_case(snapshot.land.land_registry)
            && cursor
                .estate_registry
                .eq_ignore_ascii_case(snapshot.land.estate_registry)
            && crate::land_authz::indexer::index_ready(cursor, now)
    });
    let rentals_matches = current_rentals.as_ref().is_some_and(|cursor| {
        cursor == snapshot.rentals_cursor
            && crate::market::chain_walk::is_ready_on(cursor, snapshot.rentals_chain.id(), now)
    });
    if !land_matches || !rentals_matches {
        tx.rollback().await?;
        return Ok(None);
    }
    let written = cancel_on(&mut tx, cancels, membership, updated_at).await?;
    tx.commit().await?;
    Ok(Some(written))
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
/// must have newer LAND evidence than the stored asset time to establish
/// a transfer, custody change or dissolved estate.
pub async fn sync_listing(
    pool: &PgPool,
    id: Uuid,
    nft: Option<&LandNft>,
    now: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let venue: Option<String> =
        sqlx::query_scalar("SELECT rental_contract_address FROM signatures.rentals WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    let Some(venue) = venue else {
        return Ok(true);
    };
    let row = sqlx::query(
        "SELECT r.status::text AS status, r.expiration < $2 AS expired, l.lessor, \
                r.created_at, r.contract_address, r.token_id::text AS token_id, r.chain_id::bigint AS chain_id, a.membership_block, a.metadata_block \
         FROM signatures.rentals r JOIN signatures.rental_listings l ON l.id = r.id \
         JOIN signatures.rental_assets a ON a.contract_address = r.contract_address AND a.token_id = r.token_id \
         WHERE r.id = $1",
    )
    .bind(id)
    .bind(now.naive_utc())
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Ok(true);
    };
    let chain = (row.get::<i64, _>("chain_id") == 1 && index::walks(&venue))
        .then_some(index::RentalsChain::Mainnet);
    if let Some(chain) = chain
        && index::fold_listing(pool, chain, &venue, id, now)
            .await?
            .is_none()
    {
        return Ok(false);
    }
    let mut tx = pool.begin().await?;
    let rentals_cursor = if let Some(chain) = chain {
        let Some(cursor) = index::lock_cursor(&mut tx, chain).await? else {
            tx.rollback().await?;
            return Ok(false);
        };
        if !crate::market::chain_walk::is_ready_on(&cursor, chain.id(), now) {
            tx.rollback().await?;
            return Ok(false);
        }
        Some(cursor)
    } else {
        None
    };
    if let Some(expected_updated_at) = nft.and_then(|n| n.land_cursor_updated_at) {
        let land_network = match chain {
            Some(index::RentalsChain::Mainnet) => crate::land_authz::LAND_MAINNET.network,
            Some(index::RentalsChain::Sepolia) | None => {
                tx.rollback().await?;
                return Ok(false);
            }
        };
        let current_land: Option<crate::land_authz::indexer::Cursor> = sqlx::query_as(
            "SELECT network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started, updated_at \
             FROM land_authz.index_cursor WHERE network=$1 FOR SHARE",
        )
        .bind(land_network)
        .fetch_optional(&mut *tx)
        .await?;
        let current_matches = current_land.is_some_and(|cursor| {
            cursor.updated_at == expected_updated_at
                && cursor
                    .land_registry
                    .eq_ignore_ascii_case(crate::land_authz::LAND_MAINNET.land_registry)
                && cursor
                    .estate_registry
                    .eq_ignore_ascii_case(crate::land_authz::LAND_MAINNET.estate_registry)
                && crate::land_authz::indexer::index_ready(&cursor, now)
        });
        if !current_matches {
            tx.rollback().await?;
            return Ok(false);
        }
    }
    let still_open: bool =
        sqlx::query_scalar("SELECT status = 'open' FROM signatures.rentals WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(false);
    if !still_open {
        tx.rollback().await?;
        return Ok(true);
    }
    let lessor: String = row.get("lessor");
    // Compare published source versions independently from legacy metadata timestamps.
    let stored_metadata_block: i64 = row.get("metadata_block");
    let created_at = row
        .get::<NaiveDateTime, _>("created_at")
        .and_utc()
        .timestamp();
    // Metadata can be filled while custody evidence is still unpublished.
    // Recheck the covering rent for a current venue observation on each refresh.
    let current = nft.filter(|n| n.metadata_block >= stored_metadata_block);
    let transfer = current
        .filter(|n| n.transferred_at > created_at || n.owner_address.eq_ignore_ascii_case(&venue));
    let owner = transfer.map(|n| n.owner_address.as_str());
    let custody: Option<String> = match transfer {
        Some(n) if n.owner_address.eq_ignore_ascii_case(&venue) => {
            if let (Some(chain), Some(cursor)) = (chain, rentals_cursor.as_ref()) {
                let evidence = index::rent_evidence_at(
                    &mut tx,
                    chain,
                    &venue,
                    row.get("contract_address"),
                    row.get("token_id"),
                    cursor.folded_block,
                )
                .await?;
                match evidence.coverage(n.transfer_block, Some(n.metadata_block), n.transferred_at)
                {
                    index::RentCoverage::Pending => None,
                    index::RentCoverage::NoCoveringRent => Some(String::new()),
                    index::RentCoverage::Covered => evidence.signer,
                }
            } else {
                // Non-walking chains have no local custody evidence. A token
                // at the Rentals venue stays unknown and cannot be cancelled.
                None
            }
        }
        _ => None,
    };

    let dissolved = nft.is_some_and(|n| {
        n.category == "estate"
            && n.estate_size == Some(0)
            && n.metadata_block >= stored_metadata_block
            && n.membership_block >= row.get::<i64, _>("membership_block")
            && n.membership_checked_at.is_some_and(|at| at > created_at)
    });
    if let Some(reason) = cancel_reason(
        row.get("expired"),
        dissolved,
        owner,
        &lessor,
        &venue,
        custody.as_deref(),
    ) {
        cancel_on(
            &mut tx,
            &[(id, reason)],
            nft.and_then(|n| Some((n.membership_block, n.membership_checked_at?))),
            now.naive_utc(),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(true)
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
    use crate::land_authz::{
        ESTATE_REGISTRY_MAINNET, LAND_MAINNET, LAND_REGISTRY_MAINNET, LAND_SEPOLIA,
    };
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

    /// A transfer after listing or a dissolved estate cancels the listing.
    /// An earlier transfer, Rentals custody, an unknown token or an
    /// unpublished index leaves it open.
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
        publish_rentals_index(pool).await;
        let pass = run_pass(pool, Some(LAND_MAINNET), Utc::now())
            .await
            .unwrap();
        assert_eq!(pass.moved, Some(3));
        assert_eq!(
            state_of(pool, transferred).await,
            cancelled(REASON_TRANSFERRED)
        );
        assert_eq!(state_of(pool, custody).await, cancelled(REASON_TRANSFERRED));
        assert_eq!(state_of(pool, dissolved).await, cancelled(REASON_DISSOLVED));
        for id in [trailing, kept, unindexed, whole] {
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
        publish_rentals_index(pool).await;
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
            transferred_at: Utc::now().timestamp() + 60,
            transfer_block: Some(1),
            membership_block: 1,
            metadata_block: 1,
            membership_checked_at: Some(Utc::now().timestamp() + 60),
            land_cursor_updated_at: None,
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
            transferred_at: 1_600_000_000,
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
        sqlx::query(
            "UPDATE signatures.rental_assets SET nft_updated_at=to_timestamp($1) AT TIME ZONE 'UTC'
            WHERE contract_address=$2 AND token_id=10",
        )
        .bind(now.timestamp() - 120)
        .bind(ESTATE_REGISTRY_MAINNET)
        .execute(pool)
        .await
        .unwrap();
        let trailing = LandNft {
            membership_checked_at: Some(now.timestamp() - 60),
            ..row(LESSOR, "estate", Some(0))
        };
        sync_listing(pool, estate, Some(&trailing), now)
            .await
            .unwrap();
        assert_eq!(
            state_of(pool, estate).await,
            open(),
            "LAND's empty observation predates the listing"
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
        sqlx::query("DELETE FROM shop.chain_cursor WHERE indexer='rentals'")
            .execute(pool)
            .await
            .unwrap();
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
        index::insert_events(&mut conn, index::RentalsChain::Mainnet, &[bump])
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
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history, chain_id) \
             VALUES ('rentals', 'ethereum', 1, 49, 49, timezone('UTC', now()), true, 1)",
        )
        .execute(pool)
        .await
        .unwrap();
        sync_listing(pool, bumped, Some(&row(LESSOR, "parcel", None)), Utc::now())
            .await
            .unwrap();
        assert_eq!(
            state_of(pool, bumped).await,
            open(),
            "the nonce bump is above the published height"
        );
        sqlx::query(
            "UPDATE shop.chain_cursor SET last_block=50, folded_block=50 WHERE indexer='rentals'",
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
            Utc::now().timestamp() + 120,
        )
        .unwrap()
        .unwrap();
        let mut conn = pool.acquire().await.unwrap();
        index::insert_events(&mut conn, index::RentalsChain::Mainnet, &[rent])
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
        index::insert_events(&mut conn, index::RentalsChain::Mainnet, &[rent])
            .await
            .unwrap();
    }

    async fn publish_rentals_index(pool: &PgPool) {
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history, chain_id) \
             VALUES ('rentals', 'ethereum', 1, 50, 50, timezone('UTC', now()), true, 1)",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    async fn wait_for_cursor_lock(pool: &PgPool) {
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let waiting: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM pg_stat_activity \
                     WHERE datname=current_database() AND wait_event_type='Lock' \
                       AND query ILIKE '%shop.chain_cursor%' AND query ILIKE '%FOR SHARE%')",
                )
                .fetch_one(pool)
                .await
                .unwrap();
                if waiting {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("decision waits for the published cursor lock");
    }

    async fn wait_for_land_cursor_lock(pool: &PgPool) {
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let waiting: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM pg_stat_activity \
                     WHERE datname=current_database() AND wait_event_type='Lock' \
                       AND query ILIKE '%land_authz.index_cursor%' AND query ILIKE '%FOR SHARE%')",
                )
                .fetch_one(pool)
                .await
                .unwrap();
                if waiting {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("PATCH waits for the observed LAND cursor lock");
    }

    #[tokio::test]
    async fn refresh_revalidates_the_rentals_cursor_before_folding_events() {
        let Some(db) = OwnedDatabase::new("rental_refresh_cursor_rewind", Schema::Full, 4).await
        else {
            return;
        };
        let pool = &db.pool;
        let id = listing(pool, LAND_REGISTRY_MAINNET, "33", 33).await;
        publish_rentals_index(pool).await;
        let bump = index::decode_log(
            &index::tests::asset_index_log(
                LESSOR,
                LAND_REGISTRY_MAINNET,
                alloy_primitives::U256::from(33u8),
                1,
                50,
                0,
            ),
            1_700_000_000,
        )
        .unwrap()
        .unwrap();
        let mut conn = pool.acquire().await.unwrap();
        index::insert_events(&mut conn, index::RentalsChain::Mainnet, &[bump])
            .await
            .unwrap();
        drop(conn);

        let mut lock = pool.begin().await.unwrap();
        sqlx::query(
            "SELECT 1 FROM shop.chain_cursor WHERE indexer='rentals' AND network='ethereum' FOR UPDATE",
        )
        .execute(&mut *lock)
        .await
        .unwrap();
        let route_pool = pool.clone();
        let refresh =
            tokio::spawn(async move { sync_listing(&route_pool, id, None, Utc::now()).await });
        wait_for_cursor_lock(pool).await;
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_block=NULL WHERE indexer='rentals' AND network='ethereum'",
        )
        .execute(&mut *lock)
        .await
        .unwrap();
        lock.commit().await.unwrap();

        assert!(!refresh.await.unwrap().unwrap());
        assert_eq!(state_of(pool, id).await, open());
        db.drop().await;
    }

    #[tokio::test]
    async fn status_pass_discards_cancellation_after_a_rentals_rewind() {
        let Some(db) = OwnedDatabase::new("rental_status_cursor_rewind", Schema::Full, 4).await
        else {
            return;
        };
        let pool = &db.pool;
        let id = listing(pool, LAND_REGISTRY_MAINNET, "34", 34).await;
        publish_rentals_index(pool).await;
        sqlx::query(
            "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) \
             VALUES ($1, $2, $3, 5, 5, timezone('UTC', now()), true)",
        )
        .bind(LAND_MAINNET.network)
        .bind(LAND_MAINNET.land_registry)
        .bind(LAND_MAINNET.estate_registry)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO land_authz.token_right (token_address, token_id, owner, updated_block, updated_log) VALUES ($1, 34, $2, 5, 0)",
        )
        .bind(LAND_MAINNET.land_registry)
        .bind(OTHER)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) VALUES (5, 0, $1, $2, 'transfer', 34, $3)",
        )
        .bind(Utc::now().timestamp() + 3600)
        .bind(LAND_MAINNET.land_registry)
        .bind(OTHER)
        .execute(pool)
        .await
        .unwrap();

        let mut lock = pool.begin().await.unwrap();
        sqlx::query(
            "SELECT 1 FROM shop.chain_cursor WHERE indexer='rentals' AND network='ethereum' FOR UPDATE",
        )
        .execute(&mut *lock)
        .await
        .unwrap();
        let status_pool = pool.clone();
        let pass =
            tokio::spawn(async move { cancel_moved(&status_pool, LAND_MAINNET, Utc::now()).await });
        wait_for_cursor_lock(pool).await;
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_block=NULL WHERE indexer='rentals' AND network='ethereum'",
        )
        .execute(&mut *lock)
        .await
        .unwrap();
        lock.commit().await.unwrap();

        assert_eq!(pass.await.unwrap().unwrap(), None);
        assert_eq!(state_of(pool, id).await, open());
        db.drop().await;
    }

    #[tokio::test]
    async fn refresh_discards_land_cancellation_after_a_rewind() {
        let Some(db) = OwnedDatabase::new("rental_patch_land_rewind", Schema::Full, 4).await else {
            return;
        };
        let pool = &db.pool;
        let id = listing(pool, LAND_REGISTRY_MAINNET, "35", 35).await;
        publish_rentals_index(pool).await;
        sqlx::query(
            "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) \
             VALUES ($1, $2, $3, 5, 5, timezone('UTC', now()), true)",
        )
        .bind(LAND_MAINNET.network)
        .bind(LAND_MAINNET.land_registry)
        .bind(LAND_MAINNET.estate_registry)
        .execute(pool)
        .await
        .unwrap();
        let land_cursor_updated_at: chrono::NaiveDateTime =
            sqlx::query_scalar("SELECT updated_at FROM land_authz.index_cursor WHERE network=$1")
                .bind(LAND_MAINNET.network)
                .fetch_one(pool)
                .await
                .unwrap();
        let nft = LandNft {
            nft_id: "test".into(),
            category: "parcel".into(),
            owner_address: OTHER.into(),
            search_text: String::new(),
            distance_to_plaza: None,
            adjacent_to_road: None,
            estate_size: None,
            created_at: 1,
            updated_at: Utc::now().timestamp() + 3600,
            transferred_at: Utc::now().timestamp() + 3600,
            transfer_block: Some(5),
            membership_block: 0,
            metadata_block: 5,
            membership_checked_at: None,
            land_cursor_updated_at: Some(land_cursor_updated_at),
        };

        let mut lock = pool.begin().await.unwrap();
        sqlx::query("SELECT 1 FROM land_authz.index_cursor WHERE network=$1 FOR UPDATE")
            .bind(LAND_MAINNET.network)
            .execute(&mut *lock)
            .await
            .unwrap();
        let route_pool = pool.clone();
        let refresh =
            tokio::spawn(
                async move { sync_listing(&route_pool, id, Some(&nft), Utc::now()).await },
            );
        wait_for_land_cursor_lock(pool).await;
        sqlx::query("UPDATE land_authz.index_cursor SET folded_block=NULL WHERE network=$1")
            .bind(LAND_MAINNET.network)
            .execute(&mut *lock)
            .await
            .unwrap();
        lock.commit().await.unwrap();

        assert!(!refresh.await.unwrap().unwrap());
        assert_eq!(state_of(pool, id).await, open());
        db.drop().await;
    }

    #[tokio::test]
    async fn sepolia_status_keeps_unknown_venue_custody_open() {
        let Some(db) = OwnedDatabase::new("rental_sepolia_status", Schema::Full, 2).await else {
            return;
        };
        let pool = &db.pool;
        let venue = crate::rentals::signature::rentals_contract(11_155_111).unwrap();
        let id = seed_listing(
            pool,
            LAND_SEPOLIA.land_registry,
            "55",
            ["0", "0", "0"],
            &format!("0x{}", hex::encode([55u8; 65])),
            LESSOR,
            venue,
        )
        .await;
        // The shared fixture defaults to mainnet. Exercise the signed Sepolia chain.
        sqlx::query("UPDATE signatures.rentals SET chain_id=11155111 WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE signatures.rental_assets SET chain_id=11155111 WHERE contract_address=$1",
        )
        .bind(LAND_SEPOLIA.land_registry)
        .execute(pool)
        .await
        .unwrap();
        let ix = index::RentalsIndex::sepolia();
        use crate::market::chain_walk::{ChainIndex, IndexSettings, test_owner};
        let owner = test_owner(pool, &ix, &IndexSettings::default()).await;
        sqlx::query("INSERT INTO shop.rental_event (network, block_number, log_index, block_time, rentals_address, kind, contract_address, token_id, signer, tenant, rental_days, price_per_day, signature, tx_hash)
                SELECT 'sepolia', 40, 0, 1800000000, rental_contract_address, 'rented', contract_address, token_id, $2, $3, 1, 1, signature, '0xtx' FROM signatures.rentals WHERE id=$1")
            .bind(id).bind(LESSOR).bind(OTHER).execute(pool).await.unwrap();
        ix.fold(pool, None, 40, &owner).await.unwrap();
        assert_eq!(
            state_of(pool, id).await,
            open(),
            "the Sepolia log walk only supplies wallet history"
        );
        let now = Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) \
             VALUES ($1, $2, $3, 40, 40, timezone('UTC', now()), true)",
        )
        .bind(LAND_SEPOLIA.network)
        .bind(LAND_SEPOLIA.land_registry)
        .bind(LAND_SEPOLIA.estate_registry)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO land_authz.token_right (token_address, token_id, owner, updated_block, updated_log) VALUES ($1, 55, $2, 40, 0)",
        )
        .bind(LAND_SEPOLIA.land_registry)
        .bind(venue)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, token_id, account, kind) VALUES (40, 0, $1, $2, 55, $3, 'transfer')",
        )
        .bind(now + 60)
        .bind(LAND_SEPOLIA.land_registry)
        .bind(venue)
        .execute(pool)
        .await
        .unwrap();
        assert_eq!(
            cancel_moved(pool, LAND_SEPOLIA, Utc::now()).await.unwrap(),
            None,
            "the status job has no Sepolia Rentals source yet"
        );
        assert_eq!(state_of(pool, id).await, open());
        db.drop().await;
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

    /// The job's custody leg: a token in the Rentals contract stays unknown
    /// until the Rentals cursor reaches its arrival. Once published, a
    /// covering rent keeps the listing only when it names this lessor; no
    /// covering rent means nobody holds custody and cancels it.
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
        let no_rent = listing(pool, land, "4", 4).await;
        let cursor_behind_arrival = listing(pool, land, "5", 5).await;
        let later = Utc::now().timestamp() + 3_600;
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "INSERT INTO land_authz.token_right (token_address, token_id, owner, updated_block, updated_log) VALUES \
                ('{land}', 1, '{RENTALS}', 40, 0), ('{land}', 2, '{RENTALS}', 40, 1), ('{land}', 3, '{RENTALS}', 40, 2), \
                ('{land}', 4, '{RENTALS}', 40, 3), ('{land}', 5, '{RENTALS}', 45, 0); \
             INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) VALUES \
                (40, 0, {later}, '{land}', 'transfer', 1, '{RENTALS}'), (40, 1, {later}, '{land}', 'transfer', 2, '{RENTALS}'), \
                (40, 2, {later}, '{land}', 'transfer', 3, '{RENTALS}'), (40, 3, {later}, '{land}', 'transfer', 4, '{RENTALS}'), \
                (45, 0, {later}, '{land}', 'transfer', 5, '{RENTALS}')"
        )))
        .execute(pool)
        .await
        .unwrap();
        publish_land_index(pool, 45).await;
        // The rent of each transfer's block, and for token 3 only a rent
        // from before the previous lessor's claim.
        index_rent(pool, 1, LESSOR, 40, later).await;
        index_rent(pool, 2, OTHER, 41, later).await;
        index_rent(pool, 3, OTHER, 10, 1_600_000_000).await;

        let pass = run_pass(pool, Some(LAND_MAINNET), Utc::now())
            .await
            .unwrap();
        assert_eq!(pass.moved, None, "the rentals index is not published");
        assert_eq!(state_of(pool, rented_by_other).await, open());

        publish_rentals_index(pool).await;
        sqlx::query(
            "UPDATE shop.chain_cursor SET last_block=40,folded_block=40 WHERE indexer='rentals'",
        )
        .execute(pool)
        .await
        .unwrap();
        let withheld = run_pass(pool, Some(LAND_MAINNET), Utc::now())
            .await
            .unwrap();
        assert_eq!(withheld.moved, Some(2));
        assert_eq!(
            state_of(pool, rented_by_other).await,
            open(),
            "rent 41 is above the published Rentals head"
        );
        assert_eq!(
            state_of(pool, unindexed).await,
            cancelled(REASON_TRANSFERRED)
        );
        assert_eq!(state_of(pool, no_rent).await, cancelled(REASON_TRANSFERRED));
        assert_eq!(
            state_of(pool, cursor_behind_arrival).await,
            open(),
            "the Rentals cursor has not reached the transfer into custody"
        );
        sqlx::query(
            "UPDATE shop.chain_cursor SET last_block=50,folded_block=50 WHERE indexer='rentals'",
        )
        .execute(pool)
        .await
        .unwrap();
        let pass = run_pass(pool, Some(LAND_MAINNET), Utc::now())
            .await
            .unwrap();
        assert_eq!(pass.moved, Some(2));
        assert_eq!(
            state_of(pool, rented_by_other).await,
            cancelled(REASON_TRANSFERRED),
            "held for another lessor"
        );
        assert_eq!(state_of(pool, held).await, open(), "held for the lessor");
        assert_eq!(
            state_of(pool, unindexed).await,
            cancelled(REASON_TRANSFERRED)
        );
        assert_eq!(state_of(pool, no_rent).await, cancelled(REASON_TRANSFERRED));
        assert_eq!(
            state_of(pool, cursor_behind_arrival).await,
            cancelled(REASON_TRANSFERRED)
        );
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
        publish_rentals_index(pool).await;
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

    /// The refresh route treats an older rent as no custody record for the
    /// token's current arrival, so the listing is transferred away.
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
            transferred_at: arrived,
            transfer_block: Some(40),
            membership_block: 0,
            metadata_block: 1,
            membership_checked_at: None,
            land_cursor_updated_at: None,
        };
        publish_rentals_index(pool).await;

        let unindexed = listing(pool, LAND_REGISTRY_MAINNET, "1", 1).await;
        index_rent(pool, 1, OTHER, 10, arrived - 86_400).await;
        sync_listing(pool, unindexed, Some(&in_custody), Utc::now())
            .await
            .unwrap();
        assert_eq!(
            state_of(pool, unindexed).await,
            cancelled(REASON_TRANSFERRED),
            "a rent older than the arrival does not cover custody"
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
    /// The fold reads its verdicts before it writes them. A listing the job
    /// cancels in between keeps the job's reason against the fold's nonce
    /// cancellation, which a rebuild could reopen; a rent of it still
    /// executes it.
    #[tokio::test]
    async fn the_fold_keeps_the_reason_of_a_listing_the_job_cancelled() {
        let Some(db) = OwnedDatabase::new("rental_fold_guard", Schema::Full, 2).await else {
            return;
        };
        let pool = &db.pool;
        let id = listing(pool, LAND_REGISTRY_MAINNET, "1", 1).await;
        let decoded = |log| index::decode_log(&log, 1_700_000_000).unwrap().unwrap();
        let token = alloy_primitives::U256::from(1u8);
        let mut conn = pool.acquire().await.unwrap();
        let bump = decoded(index::tests::asset_index_log(
            LESSOR,
            LAND_REGISTRY_MAINNET,
            token,
            1,
            50,
            0,
        ));
        index::insert_events(&mut conn, index::RentalsChain::Mainnet, &[bump])
            .await
            .unwrap();
        let verdicts =
            index::changed_verdicts(&mut conn, index::RentalsChain::Mainnet, RENTALS, -1, None)
                .await
                .unwrap();
        assert_eq!(
            verdicts
                .iter()
                .map(|v| (v.id, v.status))
                .collect::<Vec<_>>(),
            [(id, "cancelled")]
        );

        expire(pool, id).await;
        run_pass(pool, None, Utc::now()).await.unwrap();
        assert_eq!(state_of(pool, id).await, cancelled(REASON_EXPIRED));
        let written = index::apply_verdicts(&mut conn, &verdicts, Utc::now().naive_utc())
            .await
            .unwrap();
        assert_eq!(written, 0);
        assert_eq!(state_of(pool, id).await, cancelled(REASON_EXPIRED));

        let rent = decoded(index::tests::rented_log(
            LAND_REGISTRY_MAINNET,
            token,
            LESSOR,
            OTHER,
            7,
            alloy_primitives::U256::from(5u8),
            &[1u8; 65],
            51,
            0,
        ));
        index::insert_events(&mut conn, index::RentalsChain::Mainnet, &[rent])
            .await
            .unwrap();
        let verdicts =
            index::changed_verdicts(&mut conn, index::RentalsChain::Mainnet, RENTALS, -1, None)
                .await
                .unwrap();
        let written = index::apply_verdicts(&mut conn, &verdicts, Utc::now().naive_utc())
            .await
            .unwrap();
        assert_eq!(written, 1);
        assert_eq!(state_of(pool, id).await, ("executed".into(), None));
        drop(conn);
        db.drop().await;
    }
}
