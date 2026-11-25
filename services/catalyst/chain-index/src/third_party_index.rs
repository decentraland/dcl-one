//! The published Polygon Third Party Registry index: the provider versions
//! and slot receipts the shop's registry walk keeps in
//! `shop.third_party_registry_event`, read at the block its cursor
//! published. A read answers `None` while there is no current publication:
//! the cursor must cover the whole Polygon history through its published
//! block, be outside divergence and quarantine, and have published within
//! the last 30 minutes.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;
use sqlx::types::Json;

/// The registry walk's name in `shop.chain_cursor`.
pub const INDEXER: &str = "third_party_registry";

/// A contract a provider links its items to, lowercased.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Contract {
    pub network: String,
    pub address: String,
}

/// What a provider's raw metadata parses to.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Metadata {
    pub name: String,
    pub description: String,
    pub contracts: Vec<Contract>,
}

/// A provider after one registry event, as the walk stores it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Provider {
    pub id: String,
    pub approved: bool,
    /// The Merkle root of its last review that set one, `0x` hex.
    pub root: Option<String>,
    pub resolver: Option<String>,
    pub raw_metadata: String,
    /// `None` when the raw metadata doesn't parse.
    pub metadata: Option<Metadata>,
    /// Lowercased addresses.
    pub managers: BTreeSet<String>,
    pub programmatic: bool,
    /// The item slots it bought, a decimal 256-bit count.
    pub max_items: String,
    /// The item slots its cheques consumed, a decimal 256-bit count.
    pub consumed_slots: String,
}

/// A consumed slot cheque: `hash` is the cheque's EIP-712 digest, `0x` hex.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Receipt {
    pub hash: String,
    pub quantity: String,
    pub signer: String,
    pub curator: String,
}

macro_rules! published {
    () => {
        "WITH published AS (
           SELECT folded_block FROM shop.chain_cursor
           WHERE indexer = $1 AND network = 'matic' AND chain_id = 137 AND full_history
             AND quarantined_at IS NULL AND diverged_at IS NULL AND folded_block IS NOT NULL
             AND folded_at BETWEEN timezone('UTC', now()) - interval '30 minutes' AND timezone('UTC', clock_timestamp())
         )"
    };
}

/// The latest version of each provider at the published block, ordered by
/// id in the database's collation: `id` alone when given, those `manager`
/// (a lowercased address) manages, the approved ones when `approved`.
/// `None` without a current publication.
pub async fn providers(
    db: impl PgExecutor<'_>,
    id: Option<&str>,
    manager: Option<&str>,
    approved: bool,
) -> anyhow::Result<Option<Vec<Provider>>> {
    let result = sqlx::query_scalar::<_, Option<Json<Vec<Provider>>>>(concat!(
        published!(),
        ", latest AS (
           SELECT DISTINCT ON (e.provider_id) e.provider_id, e.state
           FROM shop.third_party_registry_event e CROSS JOIN published p
           WHERE e.block_number <= p.folded_block AND ($2::text IS NULL OR e.provider_id = $2)
           ORDER BY e.provider_id, e.block_number DESC, e.log_index DESC
         )
         SELECT CASE WHEN EXISTS (SELECT 1 FROM published)
           THEN coalesce(jsonb_agg(state ORDER BY provider_id), '[]'::jsonb)
         END
         FROM latest
         WHERE ($3::text IS NULL OR state->'managers' ? $3)
           AND (NOT $4 OR state->'approved' = 'true'::jsonb)"
    ))
    .bind(INDEXER)
    .bind(id)
    .bind(manager)
    .bind(approved)
    .fetch_one(db)
    .await?;
    Ok(result.map(|rows| rows.0))
}

/// The receipts of `provider` whose cheque digest is `hash` (lowercase `0x`
/// hex) at the published block, oldest first. `None` without a current
/// publication.
pub async fn receipts(
    db: impl PgExecutor<'_>,
    provider: &str,
    hash: &str,
) -> anyhow::Result<Option<Vec<Receipt>>> {
    let result = sqlx::query_scalar::<_, Option<Json<Vec<Receipt>>>>(concat!(
        published!(),
        " SELECT CASE WHEN EXISTS (SELECT 1 FROM published)
           THEN coalesce(jsonb_agg(e.receipt ORDER BY e.block_number, e.log_index), '[]'::jsonb)
         END
         FROM shop.third_party_registry_event e CROSS JOIN published p
         WHERE e.block_number <= p.folded_block AND e.provider_id = $2
           AND e.receipt->>'hash' = $3"
    ))
    .bind(INDEXER)
    .bind(provider)
    .bind(hash)
    .fetch_one(db)
    .await?;
    Ok(result.map(|rows| rows.0))
}
