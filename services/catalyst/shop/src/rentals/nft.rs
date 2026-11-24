//! One listing asset as of a published land-authz snapshot.

use crate::land_authz::LandNetwork;
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
}

#[derive(Debug, thiserror::Error)]
pub enum NftReadError {
    #[error("land-authz index unavailable")]
    Unavailable,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub async fn nft_by_contract_token(
    pool: &PgPool,
    network: Option<LandNetwork>,
    contract_address: &str,
    token_id: &str,
) -> Result<Option<LandNft>, NftReadError> {
    let net = network.ok_or(NftReadError::Unavailable)?;
    let mut snapshot = crate::land_authz::indexer::ready_snapshot(
        pool,
        net.network,
        net.land_registry,
        net.estate_registry,
    )
    .await?
    .ok_or(NftReadError::Unavailable)?;

    let row = sqlx::query_as(
        "SELECT tr.token_address || '-' || tr.token_id AS nft_id,
                CASE WHEN tr.token_address = $3 THEN 'parcel' ELSE 'estate' END AS category,
                tr.owner AS owner_address,
                CASE WHEN tr.token_address = $3 THEN concat_ws(',', tr.x, tr.y, nullif(lower(d.name), ''))
                     ELSE coalesce(lower(d.name), '') END AS search_text,
                coalesce(p.distance_to_plaza, members.distance, -1)::int AS distance_to_plaza,
                coalesce(p.adjacent_to_road, members.adjacent, false) AS adjacent_to_road,
                members.size AS estate_size,
                coalesce(mint.block_time, 0) AS created_at,
                greatest(transfer.block_time, doc.block_time, 0) AS updated_at
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
                 AND token_id = tr.token_id AND kind = 'transfer' ORDER BY block_number, log_index LIMIT 1
         ) mint ON true
         LEFT JOIN LATERAL (
             SELECT block_time FROM land_authz.authz_event WHERE token_address = tr.token_address
                 AND token_id = tr.token_id AND kind = 'transfer' ORDER BY block_number DESC, log_index DESC LIMIT 1
         ) transfer ON true
         WHERE tr.token_address = $1 AND tr.token_id = $2::numeric AND tr.token_address IN ($3, $4)
           AND tr.owner IS NOT NULL",
    )
    .bind(contract_address.to_ascii_lowercase())
    .bind(token_id)
    .bind(net.land_registry)
    .bind(net.estate_registry)
    .fetch_optional(&mut *snapshot.tx)
    .await?;
    Ok(row)
}
