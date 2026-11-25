//! One listing asset as of a published land-authz snapshot.

use crate::land_authz::{LAND_MAINNET, LAND_SEPOLIA, LandNetwork};
use crate::market::chain_walk;
use sqlx::PgPool;

/// The indexed LAND or estate row used to create or refresh a listing.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct LandNft {
    /// `<contract>-<tokenId>`, the wire's `nftId`.
    pub nft_id: String,
    /// `parcel` or `estate`.
    pub category: String,
    pub owner_address: String,
    pub search_text: String,
    pub distance_to_plaza: Option<i32>,
    pub adjacent_to_road: Option<bool>,
    pub estate_size: Option<i32>,
    /// Seconds since the epoch.
    pub created_at: i64,
    pub updated_at: i64,
    pub transferred_at: i64,
    /// The marketplace subgraph exposes the transfer time only.
    pub transfer_block: Option<i64>,
    pub membership_block: i64,
    pub metadata_block: i64,
    pub membership_checked_at: Option<i64>,
    /// Update timestamp of the LAND publication snapshot, absent on graph rows.
    pub land_cursor_updated_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, thiserror::Error)]
pub enum NftReadError {
    #[error("land-authz index unavailable")]
    Unavailable,
    #[error("NFT metadata index unavailable")]
    MetadataUnavailable,
    #[error("marketplace subgraph unavailable: {0}")]
    Subgraph(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub(super) fn listing_network(
    network: Option<LandNetwork>,
    chain_id: i64,
) -> Result<LandNetwork, NftReadError> {
    let expected = match chain_id {
        1 => LAND_MAINNET,
        11155111 => LAND_SEPOLIA,
        _ => return Err(NftReadError::Unavailable),
    };
    network
        .filter(|net| *net == expected)
        .ok_or(NftReadError::Unavailable)
}

pub(super) async fn nft_by_contract_token(
    pool: &PgPool,
    net: LandNetwork,
    chain_id: i64,
    contract_address: &str,
    token_id: &str,
    custody_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<Option<(LandNft, Option<super::index::RentEvidence>)>, NftReadError> {
    let mut snapshot = crate::land_authz::indexer::ready_snapshot(
        pool,
        net.network,
        net.land_registry,
        net.estate_registry,
    )
    .await?
    .ok_or(NftReadError::Unavailable)?;

    let chain = super::index::RentalsChain::from_id(chain_id)
        .filter(|chain| chain.land() == net)
        .ok_or(NftReadError::Unavailable)?;
    let ready = chain_walk::published_sql_on("c", "timezone('UTC', now())", chain.id(), 137);
    let metadata_head: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT folded_block FROM shop.chain_cursor c WHERE indexer='nft' AND network='ethereum'
         AND chain_id=$1 AND {ready}"
    )))
    .bind(chain_id)
    .fetch_optional(&mut *snapshot.tx)
    .await?
    .ok_or(NftReadError::MetadataUnavailable)?;

    #[derive(sqlx::FromRow)]
    struct IndexedAsset {
        #[sqlx(flatten)]
        asset: LandNft,
        data_block: Option<i64>,
    }

    let land_head = snapshot
        .cursor
        .folded_block
        .ok_or(NftReadError::Unavailable)?;
    let row = sqlx::query_as::<_, IndexedAsset>(
        "SELECT tr.token_address || '-' || tr.token_id AS nft_id,
                CASE WHEN tr.token_address = $3 THEN 'parcel' ELSE 'estate' END AS category,
                tr.owner AS owner_address,
                CASE WHEN tr.token_address = $3 THEN concat_ws(',', tr.x, tr.y, nullif(lower(d.name), ''))
                     ELSE coalesce(lower(d.name), '') END AS search_text,
                coalesce(p.distance_to_plaza, members.distance, -1)::int AS distance_to_plaza,
                coalesce(p.adjacent_to_road, members.adjacent, false) AS adjacent_to_road,
                members.size AS estate_size,
                coalesce(mint.block_time, 0) AS created_at,
                greatest(transfer.block_time, doc.block_time, 0) AS updated_at,
                coalesce(transfer.block_time, 0) AS transferred_at,
                CASE WHEN tr.token_address = $4 THEN (
                    SELECT e.block_time FROM land_authz.authz_event e WHERE e.block_number <= $5
                    ORDER BY e.block_number DESC, e.log_index DESC LIMIT 1
                ) END AS membership_checked_at,
                CASE WHEN tr.token_address = $4 THEN $5::bigint ELSE 0 END AS membership_block,
                LEAST($5::bigint, $6::bigint) AS metadata_block,
                $7::timestamp AS land_cursor_updated_at,
                coalesce(transfer.block_number, 0) AS transfer_block, d.updated_block AS data_block
         FROM land_authz.token_right tr
         LEFT JOIN shop.land_data d ON d.token_address = tr.token_address AND d.token_id = tr.token_id
         LEFT JOIN shop.nft_event doc ON doc.network = 'ethereum' AND doc.block_number = d.updated_block
                                      AND doc.log_index = d.updated_log
         LEFT JOIN shop.land_proximity p ON tr.token_address = $3 AND (p.x, p.y) = (tr.x, tr.y)
         LEFT JOIN LATERAL (
             SELECT count(*)::int AS size, min(lp.distance_to_plaza) FILTER (WHERE lp.distance_to_plaza >= 0) AS distance,
                    bool_or(lp.adjacent_to_road) AS adjacent
             FROM land_authz.estate_land m
             LEFT JOIN land_authz.token_right parcel ON parcel.token_address = $3 AND parcel.token_id = m.land_token_id
             LEFT JOIN shop.land_proximity lp ON (lp.x, lp.y) = (parcel.x, parcel.y)
             WHERE m.estate_token_id = tr.token_id
         ) members ON tr.token_address = $4
         LEFT JOIN LATERAL (
             SELECT block_time FROM land_authz.authz_event WHERE token_address = tr.token_address
                 AND token_id = tr.token_id AND kind = 'transfer' AND block_number <= $5
             ORDER BY block_number, log_index LIMIT 1
         ) mint ON true
         LEFT JOIN LATERAL (
             SELECT block_number, block_time FROM land_authz.authz_event WHERE token_address = tr.token_address
                 AND token_id = tr.token_id AND kind = 'transfer' AND block_number <= $5
             ORDER BY block_number DESC, log_index DESC LIMIT 1
         ) transfer ON true
         WHERE tr.token_address = $1 AND tr.token_id = $2::numeric AND tr.token_address IN ($3, $4)
           AND tr.owner IS NOT NULL",
    )
    .bind(contract_address.to_ascii_lowercase())
    .bind(token_id)
    .bind(net.land_registry)
    .bind(net.estate_registry)
    .bind(land_head)
    .bind(metadata_head)
    .bind(snapshot.cursor.updated_at)
    .fetch_optional(&mut *snapshot.tx)
    .await?;
    let Some(row) = row else { return Ok(None) };
    // A newer document cannot date the owner or estate membership beyond the rights fold.
    if row.data_block.is_some_and(|block| block > land_head) {
        return Err(NftReadError::Unavailable);
    }
    if row.asset.created_at <= 0 || row.asset.transferred_at <= 0 {
        return Err(NftReadError::Unavailable);
    }
    if row
        .asset
        .transfer_block
        .is_some_and(|block| block > metadata_head)
        || row.data_block.is_some_and(|block| block > metadata_head)
    {
        return Err(NftReadError::MetadataUnavailable);
    }
    let at_venue = row.asset.owner_address.eq_ignore_ascii_case(chain.venue());
    let rent = match custody_at {
        Some(now) if at_venue => Some(
            super::index::published_rent(
                &mut snapshot.tx,
                chain,
                chain.venue(),
                contract_address,
                token_id,
                now,
            )
            .await?,
        ),
        _ => None,
    };
    Ok(Some((row.asset, rent)))
}
