//! `collections-mirror`: a whole walk of `collections(orderBy: id)` on each
//! configured collections subgraph, upserted into `shop.collections`
//! (PK `(network, contract_address)`). A collection is never deleted on
//! chain, so a row the upstream no longer returns is kept; `mirrored_at` is
//! the pass that last wrote it.

use crate::shared::now_utc;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::jobs::JobContext;

use super::super::MarketplaceState;
use super::super::graph::Network;
use super::super::readers::{bool_of, int_of, opt_int_of, str_of, strings_of};
use super::super::wire;
use super::{Mirror, PAGE_SIZE, Page, walk};

const QUERY: &str = r#"
query collections($idFrom: ID!, $first: Int!) {
  collections(first: $first, orderBy: id, orderDirection: asc, where: { id_gt: $idFrom }) {
    id
    name
    symbol
    owner
    creator
    isCompleted
    isApproved
    isEditable
    minters
    managers
    urn
    itemsCount
    createdAt
    updatedAt
    reviewedAt
    firstListedAt
    searchIsStoreMinter
    searchText
  }
}"#;

pub(super) async fn run_pass(
    state: &MarketplaceState,
    ctx: &JobContext,
    network: Network,
    url: &str,
) -> anyhow::Result<()> {
    let pool = state.pool.clone();
    let chain_id: i32 = u64::from(wire::Network::from(network).chain_id()) as i32;
    walk(
        state,
        ctx,
        Mirror::Collections,
        network,
        url,
        false,
        |_, id| async move {
            let data = super::super::readers::query(
                state,
                url,
                QUERY,
                json!({ "idFrom": id.unwrap_or_default(), "first": PAGE_SIZE }),
            )
            .await?;
            let rows = super::super::readers::rows(&data, "collections");
            let last_id = rows
                .last()
                .and_then(|r| r.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string);
            Ok(Page {
                rows,
                last_updated_at: None,
                last_id,
            })
        },
        |rows| {
            let pool = pool.clone();
            async move { upsert(&pool, network, chain_id, &rows).await }
        },
    )
    .await
}

/// The rows as `jsonb_to_recordset` reads them; malformed rows are skipped
/// and counted in the log, a duplicate id in one page keeps the last copy.
fn recordset(rows: &[Value]) -> Vec<Value> {
    let mut by_id: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();
    for r in rows {
        let id = str_of(r, "id").to_lowercase();
        if !super::super::is_valid_eth_address(&id) {
            tracing::warn!(id, "collections mirror: row without a contract id skipped");
            continue;
        }
        by_id.insert(
            id.clone(),
            json!({
                "contract_address": id,
                "name": str_of(r, "name"),
                "symbol": r.get("symbol").and_then(Value::as_str),
                "creator": str_of(r, "creator").to_lowercase(),
                "owner": str_of(r, "owner").to_lowercase(),
                "urn": str_of(r, "urn"),
                "is_completed": bool_of(r, "isCompleted"),
                "is_approved": bool_of(r, "isApproved"),
                "is_editable": bool_of(r, "isEditable"),
                "items_count": int_of(r, "itemsCount").max(0),
                "minters": strings_of(r, "minters"),
                "managers": strings_of(r, "managers"),
                "search_is_store_minter": bool_of(r, "searchIsStoreMinter"),
                "search_text": r.get("searchText").and_then(Value::as_str),
                "created_at": int_of(r, "createdAt"),
                "updated_at": int_of(r, "updatedAt"),
                "reviewed_at": int_of(r, "reviewedAt"),
                "first_listed_at": opt_int_of(r, "firstListedAt"),
            }),
        );
    }
    by_id.into_values().collect()
}

pub async fn upsert(
    pool: &PgPool,
    network: Network,
    chain_id: i32,
    rows: &[Value],
) -> Result<i64, sqlx::Error> {
    let records = recordset(rows);
    if records.is_empty() {
        return Ok(0);
    }
    let done = sqlx::query(
        "INSERT INTO shop.collections \
             (network, contract_address, chain_id, name, symbol, creator, owner, urn, is_completed, is_approved, \
              is_editable, items_count, minters, managers, search_is_store_minter, search_text, created_at, \
              updated_at, reviewed_at, first_listed_at, mirrored_at) \
         SELECT $1, r.contract_address, $2, r.name, r.symbol, r.creator, r.owner, r.urn, r.is_completed, \
                r.is_approved, r.is_editable, r.items_count, r.minters, r.managers, r.search_is_store_minter, \
                r.search_text, r.created_at, r.updated_at, r.reviewed_at, r.first_listed_at, $3 \
         FROM jsonb_to_recordset($4) AS r(contract_address text, name text, symbol text, creator text, owner text, \
              urn text, is_completed boolean, is_approved boolean, is_editable boolean, items_count integer, \
              minters text[], managers text[], search_is_store_minter boolean, search_text text, \
              created_at bigint, updated_at bigint, reviewed_at bigint, first_listed_at bigint) \
         ON CONFLICT (network, contract_address) DO UPDATE SET \
             chain_id = EXCLUDED.chain_id, name = EXCLUDED.name, symbol = EXCLUDED.symbol, \
             creator = EXCLUDED.creator, owner = EXCLUDED.owner, urn = EXCLUDED.urn, \
             is_completed = EXCLUDED.is_completed, is_approved = EXCLUDED.is_approved, \
             is_editable = EXCLUDED.is_editable, items_count = EXCLUDED.items_count, \
             minters = EXCLUDED.minters, managers = EXCLUDED.managers, \
             search_is_store_minter = EXCLUDED.search_is_store_minter, search_text = EXCLUDED.search_text, \
             created_at = EXCLUDED.created_at, updated_at = EXCLUDED.updated_at, \
             reviewed_at = EXCLUDED.reviewed_at, first_listed_at = EXCLUDED.first_listed_at, \
             mirrored_at = EXCLUDED.mirrored_at",
    )
    .bind(network.db_name())
    .bind(chain_id)
    .bind(now_utc())
    .bind(Value::Array(records))
    .execute(pool)
    .await?;
    Ok(done.rows_affected() as i64)
}

#[cfg(test)]
pub(crate) fn fixture(id: &str, name: &str, creator: &str, approved: bool) -> Value {
    json!({
        "id": id, "name": name, "symbol": "DCL", "owner": creator, "creator": creator,
        "isCompleted": true, "isApproved": approved, "isEditable": false,
        "minters": ["0x214ffc0f0103735728dc66b61a22e4f163e275ae"], "managers": [], "urn": format!("urn:decentraland:matic:collections-v2:{id}"),
        "itemsCount": "3", "createdAt": "1700000000", "updatedAt": "1700000100", "reviewedAt": "1700000200",
        "firstListedAt": null, "searchIsStoreMinter": true, "searchText": name.to_lowercase()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_map_to_the_collections_columns_and_dedupe_by_contract() {
        let a = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let rows = vec![
            fixture(&a.to_uppercase(), "First", "0xCREATOR", true),
            fixture(a, "Second", "0xcreator", false),
            json!({ "id": "nope" }),
        ];
        let out = recordset(&rows);
        assert_eq!(
            out.len(),
            1,
            "the same contract twice keeps one row, a malformed id is skipped"
        );
        assert_eq!(out[0]["contract_address"], a);
        assert_eq!(out[0]["name"], "Second");
        assert_eq!(out[0]["creator"], "0xcreator");
        assert_eq!(out[0]["items_count"], 3);
        assert_eq!(out[0]["created_at"], 1_700_000_000_i64);
        assert_eq!(out[0]["first_listed_at"], Value::Null);
        assert_eq!(
            out[0]["minters"],
            json!(["0x214ffc0f0103735728dc66b61a22e4f163e275ae"])
        );
    }
}
