//! Current rentals from one published chain snapshot. The lessor retains
//! custody information until a claim, even after the tenant's rental expires.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::types::Json;

use super::SharedState;
use super::http::{ApiError, Ok2};
use super::index::RentalsChain;
use crate::market::chain_walk::published_sql_on;
use crate::market::{Source, is_valid_eth_address, unavailable};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RentalQuery {
    chain_id: Option<i64>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Rental {
    id: String,
    contract_address: String,
    token_id: String,
    lessor: String,
    tenant: String,
    operator: Option<String>,
    started_at: i64,
    ends_at: i64,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct WalletRentals {
    lessor_rentals: Vec<Rental>,
    tenant_rentals: Vec<Rental>,
}

/// Cursor and history are evaluated in the same statement snapshot. Find
/// candidate assets by wallet, then check their latest rent or claim across
/// all participants before doing any work on the current rental period.
async fn wallet_rentals(
    pool: &PgPool,
    chain: RentalsChain,
    wallet: &str,
    now: DateTime<Utc>,
) -> sqlx::Result<Option<WalletRentals>> {
    let query = format!(
        "
WITH published AS (
    SELECT c.folded_block
    FROM shop.chain_cursor c
    WHERE c.indexer = $4 AND c.network = 'ethereum' AND {ready}
)
SELECT COALESCE((
    SELECT jsonb_agg(jsonb_build_object(
        'id', latest.tx_hash || ':' || latest.log_index,
        'contractAddress', candidate.contract_address,
        'tokenId', candidate.token_id::text,
        'lessor', latest.signer,
        'tenant', latest.tenant,
        'operator', latest.operator,
        'startedAt', period.block_time,
        'endsAt', period.block_time + duration.days * 86400
    ) ORDER BY candidate.contract_address, candidate.token_id)
    FROM (
        SELECT DISTINCT e.contract_address, e.token_id
        FROM shop.rental_event e
        WHERE e.network = $2 AND e.rentals_address = $3 AND e.block_number <= published.folded_block
          AND e.kind = 'rented' AND e.contract_address = ANY($6)
          AND (e.signer = $1 OR e.tenant = $1)
    ) candidate
    CROSS JOIN LATERAL (
        SELECT e.kind, e.signer, e.tenant, e.operator, e.tx_hash, e.block_number, e.log_index
        FROM shop.rental_event e
        WHERE e.network = $2 AND e.rentals_address = $3 AND e.block_number <= published.folded_block
          AND e.contract_address = candidate.contract_address AND e.token_id = candidate.token_id
          AND e.kind IN ('rented', 'claimed')
        ORDER BY e.block_number DESC, e.log_index DESC
        LIMIT 1
    ) latest
    CROSS JOIN LATERAL (
        SELECT e.block_time, e.block_number, e.log_index
        FROM shop.rental_event e
        WHERE e.network = $2 AND e.rentals_address = $3
          AND e.contract_address = candidate.contract_address AND e.token_id = candidate.token_id
          AND (e.block_number, e.log_index) <= (latest.block_number, latest.log_index)
          AND e.kind = 'rented' AND e.is_extension IS NOT TRUE
        ORDER BY e.block_number DESC, e.log_index DESC
        LIMIT 1
    ) period
    CROSS JOIN LATERAL (
        SELECT sum(e.rental_days) AS days
        FROM shop.rental_event e
        WHERE e.network = $2 AND e.rentals_address = $3
          AND e.contract_address = candidate.contract_address AND e.token_id = candidate.token_id
          AND (e.block_number, e.log_index) >= (period.block_number, period.log_index)
          AND (e.block_number, e.log_index) <= (latest.block_number, latest.log_index)
          AND e.kind = 'rented'
    ) duration
    WHERE latest.kind = 'rented' AND (latest.signer = $1 OR latest.tenant = $1)
), '[]'::jsonb)
FROM published",
        ready = published_sql_on("c", "$5", chain.id(), 137)
    );
    let rows: Option<Json<Vec<Rental>>> = sqlx::query_scalar(sqlx::AssertSqlSafe(query))
        .bind(wallet)
        .bind(chain.event_network())
        .bind(chain.venue())
        .bind(chain.indexer())
        .bind(now.naive_utc())
        .bind(chain.registries().as_slice())
        .fetch_optional(pool)
        .await?;

    Ok(rows.map(|Json(rows)| {
        let mut rentals = WalletRentals::default();
        for rental in rows {
            if rental.tenant == wallet && rental.ends_at > now.timestamp() {
                rentals.tenant_rentals.push(rental.clone());
            }
            if rental.lessor == wallet {
                rentals.lessor_rentals.push(rental);
            }
        }
        rentals
    }))
}

/// `GET /v1/rentals/{address}?chainId=1`, with mainnet as the default.
pub(super) async fn get_wallet_rentals(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    Query(query): Query<RentalQuery>,
) -> Result<Response, ApiError> {
    if !is_valid_eth_address(&address) {
        return Err(ApiError::bad_request("Invalid Ethereum address"));
    }
    let chain = match query.chain_id.unwrap_or(1) {
        1 => RentalsChain::Mainnet,
        11_155_111 => RentalsChain::Sepolia,
        _ => return Err(ApiError::bad_request("Unsupported rentals chain")),
    };

    Ok(
        match wallet_rentals(
            state.db.pool(),
            chain,
            &address.to_ascii_lowercase(),
            Utc::now(),
        )
        .await?
        {
            Some(rentals) => Ok2(StatusCode::OK, rentals).into_response(),
            None => unavailable(Source::RentalsIndex, "The rentals index is not ready"),
        },
    )
}
