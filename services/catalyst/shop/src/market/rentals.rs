//! The market's rentals reader: the LAND and estate rental listings of the
//! rentals listings server (`signatures.*`, this node's own table, over the
//! ONE pool) for `/v1/nfts`. `isOnRent` with a LAND category narrows the
//! page to the assets of the newest 1000 listings in `rentalStatus`
//! (default `open`), and a page's parcels and estates carry their listing
//! (`rental`, `nft.openRentalId`). `tenant` and `rentalDays` were parsed and
//! never applied by the old query builder; they still are not. A listing
//! without periods is not a listing (the old inner join on the aggregate).

#[cfg(test)]
use sqlx::PgPool;

use crate::rentals::db::{STATUS_VALUES, row_to_listing};
use crate::rentals::types::RentalListing;

/// Supplemental owner matches for a LAND page. Only the published Rentals log
/// can identify a lessor. A later transfer into the venue can belong to any
/// wallet, including one with no previous rent. Wait for that arrival before
/// returning an owner page. The caller supplies the catalogue's LAND snapshot.
pub(super) async fn unclaimed_land(
    land: &mut crate::land_authz::indexer::ReadySnapshot,
    owner: &str,
) -> Result<Vec<String>, axum::response::Response> {
    use super::chain_walk::{self, ChainNet};
    use super::{Source, unavailable};
    use crate::rentals::index::RentalsChain;
    let chain = RentalsChain::from_land_network(&land.cursor.network)
        .ok_or_else(super::nft_reads::land_unavailable)?;

    let land_head = land
        .cursor
        .folded_block
        .ok_or_else(super::nft_reads::land_unavailable)?;
    let conn = &mut *land.tx;
    let cursor = chain_walk::cursor(&mut *conn, chain.indexer(), ChainNet::Ethereum)
        .await
        .map_err(super::readers::index_read_error)?;
    let Some(head) =
        cursor.and_then(|c| chain_walk::fresh_fold_on(&c, chain.id(), chrono::Utc::now()))
    else {
        return Err(unavailable(
            Source::RentalsIndex,
            "The rentals index is not ready",
        ));
    };
    let registries = chain.registries();
    let behind: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM land_authz.authz_event e
            JOIN land_authz.token_right t USING (token_address,token_id)
            WHERE e.block_number > $1 AND e.block_number <= $4 AND e.kind='transfer'
              AND e.token_address=ANY($2) AND t.owner=$3
        )",
    )
    .bind(head)
    .bind(registries.as_slice())
    .bind(chain.venue())
    .bind(land_head)
    .fetch_one(&mut *conn)
    .await
    .map_err(super::readers::index_read_error)?;
    if behind {
        tracing::debug!(
            rentals_folded_block = head,
            "owner page waiting for Rentals to publish a transfer into custody"
        );
        return Err(unavailable(
            Source::RentalsIndex,
            "The rentals index has not reached the transfer into custody",
        ));
    }
    sqlx::query_scalar(
        "WITH candidates AS (
            SELECT DISTINCT e.contract_address,e.token_id FROM shop.rental_event e
            WHERE e.network=$6 AND e.rentals_address=$1 AND e.signer=$2
              AND e.kind='rented' AND e.block_number <= $3 AND e.contract_address=ANY($4)
        )
        SELECT c.contract_address || '-' || c.token_id::text FROM candidates c
        JOIN land_authz.token_right t ON (t.token_address,t.token_id)=(c.contract_address,c.token_id)
        CROSS JOIN LATERAL (
            SELECT e.kind,e.signer,e.block_number,e.log_index FROM shop.rental_event e
            WHERE e.network=$6 AND e.rentals_address=$1 AND e.block_number <= $3
              AND (e.contract_address,e.token_id)=(c.contract_address,c.token_id)
              AND e.kind IN ('rented','claimed')
            ORDER BY e.block_number DESC,e.log_index DESC LIMIT 1
        ) latest
        CROSS JOIN LATERAL (
            SELECT e.block_number,e.log_index FROM land_authz.authz_event e
            WHERE (e.token_address,e.token_id)=(c.contract_address,c.token_id) AND e.kind='transfer' AND e.block_number <= $5
            ORDER BY e.block_number DESC,e.log_index DESC LIMIT 1
        ) arrival
        WHERE t.owner=$1 AND latest.kind='rented' AND latest.signer=$2
          AND (latest.block_number,latest.log_index) >= (arrival.block_number,arrival.log_index)",
    )
    .bind(chain.venue()).bind(owner.to_ascii_lowercase()).bind(head).bind(registries.as_slice()).bind(land_head).bind(chain.event_network())
        .fetch_all(conn).await.map_err(super::readers::index_read_error)
}

/// Before the reader cutover, the standalone Sepolia walk published the raw
/// events without deriving signed listing statuses. Check the generation of
/// both publications on the page snapshot before using those statuses.
pub(super) async fn require_sepolia_listings(
    conn: &mut sqlx::PgConnection,
) -> Result<(), axum::response::Response> {
    use crate::rentals::index::RentalsChain;
    let chain = RentalsChain::Sepolia;
    let sql = format!("SELECT EXISTS(SELECT 1 FROM shop.chain_cursor c
        JOIN shop.worker_progress p ON p.worker='rental_listing_generation' AND p.scope=$2 AND p.position=c.owner_generation
        WHERE c.indexer=$3 AND c.network='ethereum' AND {})",
        crate::chain_index::catalogue::published_sql_on("c","$1",chain.id(),super::chain_walk::ChainNet::Matic.testnet_chain_id()));
    let ready: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(chrono::Utc::now().naive_utc())
        .bind(chain.id())
        .bind(chain.indexer())
        .fetch_one(conn)
        .await
        .map_err(super::readers::index_read_error)?;
    if ready {
        Ok(())
    } else {
        Err(super::unavailable(
            super::Source::RentalsIndex,
            "Sepolia rental listings have not been published for the current Rentals history",
        ))
    }
}

/// The old `get_open_rentals` bound.
pub const NEWEST_LISTINGS: i64 = 1000;

const COLUMNS: &str = "SELECT r.id, r.contract_address, r.token_id::text AS token_id, r.network, r.chain_id, r.expiration, \
     r.nonces, r.signature, r.rental_contract_address, r.status::text AS status, r.target, r.rented_days, \
     r.created_at, r.updated_at, r.started_at, l.lessor, l.tenant, a.nft_id, a.category, a.search_text, \
     (SELECT jsonb_agg(jsonb_build_array(p.min_days::text, p.max_days::text, p.price_per_day::text) ORDER BY p.min_days) \
        FROM signatures.rental_periods p WHERE p.rental_id = r.id) AS periods \
     FROM signatures.rentals r \
     JOIN signatures.rental_listings l ON l.id = r.id \
     JOIN signatures.rental_assets a ON a.contract_address = r.contract_address AND a.token_id = r.token_id \
     WHERE EXISTS (SELECT 1 FROM signatures.rental_periods p WHERE p.rental_id = r.id) ";

/// `rentalStatus`, or `open` without one; a value outside the enum matches
/// nothing (the old text comparison), never a 500.
pub fn statuses(requested: &[String]) -> Vec<String> {
    if requested.is_empty() {
        return vec!["open".to_string()];
    }
    requested
        .iter()
        .filter(|s| STATUS_VALUES.contains(&s.as_str()))
        .cloned()
        .collect()
}

/// The listings of these nft ids (a page's parcels and estates) in these
/// statuses, oldest first so a later listing of the same asset wins.
pub async fn listings_of_nfts<'e>(
    conn: impl sqlx::PgExecutor<'e>,
    chain_id: i64,
    nft_ids: &[String],
    statuses: &[String],
) -> Result<Vec<RentalListing>, sqlx::Error> {
    if nft_ids.is_empty() || statuses.is_empty() {
        return Ok(Vec::new());
    }
    let sql = format!(
        "{COLUMNS} AND r.chain_id=$3 AND a.nft_id = ANY($1) AND r.status = ANY($2::signatures.rental_status[]) ORDER BY r.created_at, r.id"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(nft_ids)
        .bind(statuses)
        .bind(chain_id)
        .fetch_all(conn)
        .await?;
    Ok(rows.iter().map(row_to_listing).collect())
}

/// The newest [`NEWEST_LISTINGS`] listings in these statuses.
pub async fn newest_listings<'e>(
    conn: impl sqlx::PgExecutor<'e>,
    chain_id: i64,
    statuses: &[String],
) -> Result<Vec<RentalListing>, sqlx::Error> {
    if statuses.is_empty() {
        return Ok(Vec::new());
    }
    let sql = format!(
        "{COLUMNS} AND r.chain_id=$3 AND r.status = ANY($1::signatures.rental_status[]) ORDER BY r.created_at DESC, r.id LIMIT $2"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(statuses)
        .bind(NEWEST_LISTINGS)
        .bind(chain_id)
        .fetch_all(conn)
        .await?;
    Ok(rows.iter().map(row_to_listing).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_status_default_is_open_and_unknown_values_match_nothing() {
        assert_eq!(statuses(&[]), ["open"]);
        assert_eq!(statuses(&["executed".into(), "bogus".into()]), ["executed"]);
        assert!(statuses(&["bogus".into()]).is_empty());
    }

    async fn listing(
        pool: &PgPool,
        contract: &str,
        status: &str,
        created: &str,
        periods: &[(i32, i32, &str)],
    ) -> String {
        let id: uuid::Uuid = sqlx::query_scalar(
            "INSERT INTO signatures.rentals (contract_address, token_id, network, chain_id, expiration, nonces, signature, rental_contract_address, status, created_at) \
             VALUES ($1, 7, 'ETHEREUM', 1, timezone('UTC', now()) + interval '30 days', ARRAY['0','0','0'], $2, '0x3a1469499d0be105d4f77045ca403a5f6dc2f3f5', $3::signatures.rental_status, timezone('UTC', now()) - $4::interval) RETURNING id",
        )
        .bind(contract)
        .bind(format!("0x{}", uuid::Uuid::new_v4().simple()))
        .bind(status)
        .bind(created)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO signatures.rental_listings (id, lessor) VALUES ($1, '0x000000000000000000000000000000000000a11c')").bind(id).execute(pool).await.unwrap();
        for (min, max, price) in periods {
            sqlx::query("INSERT INTO signatures.rental_periods (rental_id, min_days, max_days, price_per_day) VALUES ($1, $2, $3, $4::numeric)").bind(id).bind(min).bind(max).bind(price).execute(pool).await.unwrap();
        }
        id.to_string()
    }

    /// A parcel with an executed and an open listing (and a period-less
    /// one, which is not a listing): the statuses select, the newest wins
    /// per asset, the periods come ordered.
    #[tokio::test]
    async fn the_reader_selects_by_status_and_carries_the_periods() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let contract =
            format!("0x{}", &uuid::Uuid::new_v4().simple().to_string()[..32]) + "00000000";
        let nft_id = format!("{contract}-7");
        sqlx::query("INSERT INTO signatures.rental_assets (contract_address, token_id, nft_id, network, chain_id, category, search_text, nft_created_at, nft_updated_at, fetched_at) VALUES ($1, 7, $2, 'ETHEREUM', 1, 'parcel', 'Parcel 1,2', timezone('UTC', now()), timezone('UTC', now()), timezone('UTC', now()))")
            .bind(&contract)
            .bind(&nft_id)
            .execute(&pool)
            .await
            .unwrap();
        let executed = listing(&pool, &contract, "executed", "2 days", &[(1, 30, "5")]).await;
        let open = listing(
            &pool,
            &contract,
            "open",
            "1 day",
            &[(7, 90, "3"), (1, 6, "4")],
        )
        .await;
        let bare = listing(&pool, &contract, "cancelled", "3 days", &[]).await;

        let got = listings_of_nfts(&pool, 1, std::slice::from_ref(&nft_id), &statuses(&[]))
            .await
            .unwrap();
        assert_eq!(
            got.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(),
            [open.as_str()]
        );
        assert_eq!(
            got[0]
                .periods
                .iter()
                .map(|p| (p.min_days, p.max_days, p.price_per_day.as_str()))
                .collect::<Vec<_>>(),
            [(1, 6, "4"), (7, 90, "3")]
        );
        assert_eq!(
            (
                got[0].nft_id.as_str(),
                got[0].category.as_str(),
                got[0].status.as_str(),
                got[0].token_id.as_str()
            ),
            (nft_id.as_str(), "parcel", "open", "7")
        );
        let all = listings_of_nfts(
            &pool,
            1,
            std::slice::from_ref(&nft_id),
            &statuses(&["open".into(), "executed".into(), "cancelled".into()]),
        )
        .await
        .unwrap();
        assert_eq!(
            all.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(),
            [executed.as_str(), open.as_str()],
            "oldest first, the period-less one left out"
        );
        let newest = newest_listings(&pool, 1, &statuses(&["executed".into()]))
            .await
            .unwrap();
        assert!(
            newest.iter().any(|l| l.id == executed)
                && !newest.iter().any(|l| l.id == open || l.id == bare)
        );
        assert!(
            newest_listings(&pool, 1, &statuses(&["bogus".into()]))
                .await
                .unwrap()
                .is_empty()
        );

        sqlx::query("DELETE FROM signatures.rental_assets WHERE contract_address = $1")
            .bind(&contract)
            .execute(&pool)
            .await
            .unwrap();
    }
}
