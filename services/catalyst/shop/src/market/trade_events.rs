//! The `trade_events` index: the off-chain marketplace contracts' logs on
//! both chains (`shop.trade_event`), walked by `chain_walk` from the
//! first contract's deployment, and folded into one chain verdict per stored
//! trade (`shop.trade_chain_state`), upstream's `mv_trades` order
//! (marketplace-server ae872a4, `logic/trades/materialized-view.ts`):
//! `cancelled` when its signer cancelled the signature or a signer or
//! contract signature index moved away from the one it was signed with,
//! else `sold` once the Traded executions of its signature reach
//! `checks.uses`, else `open`; expiry stays a read-time rule. The contract's
//! own rules (`Signatures.sol`, `Verifications.sol`, `Marketplace.sol` at
//! offchain-marketplace-contract 87350c6): uses are counted per (signer,
//! keccak256(signature)); a cancellation is keyed by (caller, hash), so only
//! the signer's cancels a trade; V3 keys cancellations on the trade's
//! EIP-712 digest: the one the trade stored when it was created or imported
//! (`shop.trades.trade_digest`), else the one the fold rebuilds from the
//! stored trade, accepted only when the signature recovers the signer over
//! it. A V3 trade with no such digest is marked (`digest_failed_at`) and
//! retried after [`DIGEST_RETRY`]; its cancellation would go unseen, so the
//! open-listing reads leave it out.
//!
//! The verdicts close a trade only while the network's cursor is published
//! (`chain_walk::published_sql_on`: a fold published within 30 minutes; a
//! pass in progress keeps the previous publish): an index still catching up
//! could show a stale signature index as a cancellation, and would miss the
//! latest executions.
//! So the open-listing reads fail closed: every route that lists, prices or
//! reports the status of trades answers the unavailable envelope naming
//! this index until it is current on every network ([`require_current`],
//! called by `trades::ready`; the stored records of `/v1/trades` carry no
//! status and wait for the trades mirror only), and
//! `unified_trades` lists a trade only while its network's cursor is ready
//! and no verdict closed it ([`listable_on_chain_sql`]), in the same
//! statement, so a cursor that falls behind after the gate drops the trade
//! instead of listing it again. `/v1/bids` reports `sold` and `cancelled`
//! from the same verdicts, and `GET /v1/trades/{hashed_signature}/accept`
//! answers the notification event of an executed trade.

use crate::market::chain_walk::{topic_address, topic_u256};
use std::collections::HashMap;
use std::str::FromStr;

use alloy_primitives::Address;
use alloy_sol_types::{SolType, sol};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool, Row};

use super::chain_walk::{self, ChainIndex, ChainNet, INDEXER_TRADE_EVENTS, LogGroup};
use super::listings::{
    self, OffChainMarketplace, TradeAssetInput, TradeChecksInput, TradeCreation,
};
use super::wire::format_ether;
use super::{MarketplaceState, SharedState, Source, unavailable};
use crate::chain_index::rpc::RpcLog;

pub const TOPIC_TRADED: &str = "0xaaecdfa7e74e704650fcb273f630f42f68974eff42bfffc1732cf30db9e4685b";
/// V3 adds `bytes32 indexed _tradeDigest`; the data is the same Trade tuple.
pub const TOPIC_TRADED_V3: &str =
    "0x71dc7036c75ab7570a8b79d4a452c5a4d3ac4fdf0b2cc58d518d979f0ec557ff";
pub const TOPIC_SIGNATURE_CANCELLED: &str =
    "0x8c4e82d0c2e3a462d0450cd1797aef55b85987a833dd3d93b61d96824a01410e";
pub const TOPIC_SIGNER_INDEX_INCREASED: &str =
    "0x492ddec6f62ab9180c9800a9a60ec15bb0dbae20d2a19cdc824448872f796667";
pub const TOPIC_CONTRACT_INDEX_INCREASED: &str =
    "0x09cc1ddfeaa3955efcd7527dadf166c8dd69e1e1ac87e52919eb59da0b42ee19";

pub const ALL_TOPICS: [&str; 5] = [
    TOPIC_TRADED,
    TOPIC_TRADED_V3,
    TOPIC_SIGNATURE_CANCELLED,
    TOPIC_SIGNER_INDEX_INCREASED,
    TOPIC_CONTRACT_INDEX_INCREASED,
];

pub const KIND_TRADED: &str = "traded";
pub const KIND_SIGNATURE_CANCELLED: &str = "signature_cancelled";
pub const KIND_SIGNER_INDEX: &str = "signer_index_increased";
pub const KIND_CONTRACT_INDEX: &str = "contract_index_increased";

/// The off-chain marketplace deployments of a network's mainnet chain
/// ([`deployments`] of `net.chain_id()`).
pub fn marketplaces(net: ChainNet) -> &'static [(&'static str, u64, bool)] {
    deployments(net.chain_id())
}

/// A marketplace contract, its deployment block and whether it cancels by digest (V3).
type Deployment = (&'static str, u64, bool);

/// The off-chain marketplace deployments of a chain, oldest first, with the
/// block each was deployed in and whether it cancels by digest (V3).
/// Mainnet blocks: `eth_getCode` bisection on rpc.decentraland.org,
/// 2026-09-25; V1 is the squid's `OffChainMarketplace`, still holding the
/// most mirrored trades. Testnet blocks (Amoy, Sepolia, the addresses of
/// `listings::offchain_marketplaces` and the dApps' `OffChainMarketplace`):
/// the same bisection, 2026-11-02. Sepolia V1 is pinned in
/// decentraland/marketplace-squid-core@85c6832e96e5459b37f0f315b8e384693684e12d,
/// src/eth/addresses/sepolia.ts. Its creation transaction is
/// 0xd813aee88f1e1a21783ca8fa44868e926b9e2ddee3a73cc69919fcebd0edd337.
const DEPLOYMENTS: [(i64, &[Deployment]); 4] = [
    (
        listings::MATIC_MAINNET,
        &[
            (
                "0x540fb08edb56aae562864b390542c97f562825ba",
                62_349_044,
                false,
            ),
            (
                "0xa40b1d129b8906888720686f3a01921ddf37716f",
                77_807_803,
                false,
            ),
            (
                "0xe38ef22abe871513555cba89adfe45ab4f548ada",
                93_588_249,
                true,
            ),
        ],
    ),
    (
        listings::ETHEREUM_MAINNET,
        &[
            (
                "0x2d6b3508f9aca32d2550f92b2addba932e73c1ff",
                20_843_699,
                false,
            ),
            (
                "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7",
                23_598_480,
                false,
            ),
            (
                "0x0f11d0d1671519683bd48abf3dbe779e300941cd",
                25_950_329,
                true,
            ),
        ],
    ),
    (
        listings::MATIC_AMOY,
        &[
            (
                "0x6ab20ae56673ed65f520b7be332aeb61b3ed727d",
                12_490_079,
                false,
            ),
            (
                "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7",
                27_973_439,
                false,
            ),
            (
                "0x36fd1434a6c4b8ade80c9847c1d15033ce34488c",
                44_057_476,
                true,
            ),
        ],
    ),
    (
        listings::ETHEREUM_SEPOLIA,
        &[
            (
                "0x54bdd3ffc1448bdab0251f05dd554eec803adb2f",
                6_770_056,
                false,
            ),
            (
                "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7",
                9_455_328,
                false,
            ),
            (
                "0x257db44ac97789c16ab277eae87dcde0c246cc9f",
                11_419_771,
                true,
            ),
        ],
    ),
];

/// Every chain [`deployments`] lists.
pub const OFFCHAIN_MARKETPLACE_CHAINS: [i64; DEPLOYMENTS.len()] = {
    let mut chains = [0; DEPLOYMENTS.len()];
    let mut i = 0;
    while i < chains.len() {
        chains[i] = DEPLOYMENTS[i].0;
        i += 1;
    }
    chains
};

/// The [`DEPLOYMENTS`] of a chain; empty for any other chain.
pub fn deployments(chain_id: i64) -> &'static [(&'static str, u64, bool)] {
    DEPLOYMENTS
        .iter()
        .find(|(chain, _)| *chain == chain_id)
        .map_or(&[], |(_, d)| *d)
}

/// The deployment `address` names on `net`, as the EIP-712 domain the
/// signature was made against (`listings::offchain_marketplaces`), for the
/// V3 digest.
fn domain_of(chain_id: i64, address: &str) -> Option<OffChainMarketplace> {
    listings::offchain_marketplaces(chain_id)
        .into_iter()
        .find(|m| m.address.eq_ignore_ascii_case(address))
}

sol! {
    struct ExternalCheckTuple {
        address contractAddress;
        bytes4 selector;
        bytes value;
        bool required;
    }

    struct ChecksTuple {
        uint256 uses;
        uint256 expiration;
        uint256 effective;
        bytes32 salt;
        uint256 contractSignatureIndex;
        uint256 signerSignatureIndex;
        bytes32 allowedRoot;
        bytes32[] allowedProof;
        ExternalCheckTuple[] externalChecks;
    }

    struct AssetTuple {
        uint256 assetType;
        address contractAddress;
        uint256 value;
        address beneficiary;
        bytes extra;
    }

    struct TradeTuple {
        address signer;
        bytes signature;
        ChecksTuple checks;
        AssetTuple[] sent;
        AssetTuple[] received;
    }
}

/// One row of `shop.trade_event`.
#[derive(Clone, Debug, PartialEq)]
pub struct TradeEvent {
    pub block_number: i64,
    pub log_index: i32,
    pub block_time: i64,
    pub marketplace_address: String,
    pub kind: &'static str,
    pub caller: String,
    pub hashed_signature: Option<String>,
    pub new_index: Option<String>,
    pub signer: Option<String>,
    pub assets: Option<Value>,
    pub tx_hash: String,
}

fn assets_json(assets: &[AssetTuple]) -> Value {
    Value::Array(
        assets
            .iter()
            .map(|a| {
                json!({
                    "assetType": a.assetType.to_string(),
                    "contractAddress": format!("{:#x}", a.contractAddress),
                    "value": a.value.to_string(),
                    "beneficiary": format!("{:#x}", a.beneficiary),
                })
            })
            .collect(),
    )
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("log at block {block} index {index} has too few topics for {kind}")]
    Topics {
        block: i64,
        index: i32,
        kind: &'static str,
    },
    #[error("Traded at block {block} index {index}: {why}")]
    Trade { block: i64, index: i32, why: String },
}

/// One log as a row; `Ok(None)` for a topic this index does not model.
pub fn decode_log(log: &RpcLog, block_time: i64) -> Result<Option<TradeEvent>, DecodeError> {
    let (block, index) = chain_walk::log_position(log);
    let (block, index) = (block, index as i32);
    let Some(topic0) = log.topics.first().map(|t| t.to_ascii_lowercase()) else {
        return Ok(None);
    };
    let need = |n: usize, kind: &'static str| {
        if log.topics.len() < n {
            Err(DecodeError::Topics { block, index, kind })
        } else {
            Ok(())
        }
    };
    let base = |kind: &'static str| TradeEvent {
        block_number: block,
        log_index: index,
        block_time,
        marketplace_address: log.address.to_ascii_lowercase(),
        kind,
        caller: log
            .topics
            .get(1)
            .and_then(|t| topic_address(t))
            .unwrap_or_default(),
        hashed_signature: None,
        new_index: None,
        signer: None,
        assets: None,
        tx_hash: log
            .transaction_hash
            .clone()
            .unwrap_or_default()
            .to_ascii_lowercase(),
    };
    Ok(Some(match topic0.as_str() {
        TOPIC_TRADED | TOPIC_TRADED_V3 => {
            need(3, KIND_TRADED)?;
            let data =
                hex::decode(log.data.trim_start_matches("0x")).map_err(|e| DecodeError::Trade {
                    block,
                    index,
                    why: e.to_string(),
                })?;
            let (trade,) =
                <(TradeTuple,)>::abi_decode_params(&data).map_err(|e| DecodeError::Trade {
                    block,
                    index,
                    why: e.to_string(),
                })?;
            TradeEvent {
                hashed_signature: Some(log.topics[2].to_ascii_lowercase()),
                signer: Some(format!("{:#x}", trade.signer)),
                assets: Some(
                    json!({ "sent": assets_json(&trade.sent), "received": assets_json(&trade.received), "uses": trade.checks.uses.to_string() }),
                ),
                ..base(KIND_TRADED)
            }
        }
        TOPIC_SIGNATURE_CANCELLED => {
            need(3, KIND_SIGNATURE_CANCELLED)?;
            TradeEvent {
                hashed_signature: Some(log.topics[2].to_ascii_lowercase()),
                ..base(KIND_SIGNATURE_CANCELLED)
            }
        }
        TOPIC_SIGNER_INDEX_INCREASED | TOPIC_CONTRACT_INDEX_INCREASED => {
            let kind = if topic0 == TOPIC_SIGNER_INDEX_INCREASED {
                KIND_SIGNER_INDEX
            } else {
                KIND_CONTRACT_INDEX
            };
            need(3, kind)?;
            TradeEvent {
                new_index: topic_u256(&log.topics[2]),
                ..base(kind)
            }
        }
        _ => return Ok(None),
    }))
}

/// Appends a window's rows (`ON CONFLICT (network, block_number, log_index) DO NOTHING`).
pub async fn insert_events(
    conn: &mut PgConnection,
    net: ChainNet,
    rows: &[TradeEvent],
) -> Result<usize, sqlx::Error> {
    let mut written = 0;
    for chunk in rows.chunks(1_000) {
        let r = sqlx::query(
            "INSERT INTO shop.trade_event \
                 (network, block_number, log_index, block_time, marketplace_address, kind, caller, hashed_signature, new_index, signer, assets, tx_hash) \
             SELECT $1, b, i, t, m, k, c, h, n::numeric, s, a, x \
             FROM UNNEST($2::bigint[], $3::int[], $4::bigint[], $5::text[], $6::text[], $7::text[], $8::text[], $9::text[], $10::text[], $11::jsonb[], $12::text[]) \
                  AS u(b, i, t, m, k, c, h, n, s, a, x) \
             ON CONFLICT (network, block_number, log_index) DO NOTHING",
        )
        .bind(net.db_name())
        .bind(chunk.iter().map(|r| r.block_number).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.log_index).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.block_time).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.marketplace_address.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.kind.to_string()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.caller.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.hashed_signature.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.new_index.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.signer.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.assets.clone()).collect::<Vec<_>>())
        .bind(chunk.iter().map(|r| r.tx_hash.clone()).collect::<Vec<_>>())
        .execute(&mut *conn)
        .await?;
        written += r.rows_affected() as usize;
    }
    Ok(written)
}

/// The stored trade as the signed struct, to rebuild its EIP-712 digest;
/// `Ok(Err(why))` when the row cannot be read as one (its checks do not
/// parse, or it is gone). A database error is the outer `Err`: the fold
/// stops and retries, it never records a digest failure it did not prove.
async fn trade_creation(
    pool: &PgPool,
    trade_id: uuid::Uuid,
) -> Result<Result<TradeCreation, String>, sqlx::Error> {
    let Some(head) = sqlx::query("SELECT signer, signature, type::text AS type, network, chain_id, checks FROM shop.trades WHERE id = $1")
        .bind(trade_id)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(Err("the trade is no longer stored".to_string()));
    };
    let checks: TradeChecksInput = match serde_json::from_value(head.get::<Value, _>("checks")) {
        Ok(checks) => checks,
        Err(e) => return Ok(Err(format!("the stored checks: {e}"))),
    };
    let assets = sqlx::query(
        "SELECT ta.direction::text AS direction, ta.asset_type::int4 AS asset_type, ta.contract_address, ta.extra, ta.beneficiary, \
                erc20.amount::text AS amount, erc721.token_id, item.item_id \
         FROM shop.trade_assets ta \
         LEFT JOIN shop.trade_assets_erc20 erc20 ON erc20.asset_id = ta.id \
         LEFT JOIN shop.trade_assets_erc721 erc721 ON erc721.asset_id = ta.id \
         LEFT JOIN shop.trade_assets_item item ON item.asset_id = ta.id \
         WHERE ta.trade_id = $1 ORDER BY ta.created_at, ta.id",
    )
    .bind(trade_id)
    .fetch_all(pool)
    .await?;
    let (mut sent, mut received) = (Vec::new(), Vec::new());
    for a in assets {
        let input = TradeAssetInput {
            asset_type: a.get("asset_type"),
            contract_address: a.get("contract_address"),
            extra: a.get("extra"),
            beneficiary: a.get("beneficiary"),
            amount: a.get("amount"),
            token_id: a.get("token_id"),
            item_id: a.get("item_id"),
        };
        if a.get::<String, _>("direction") == "sent" {
            sent.push(input)
        } else {
            received.push(input)
        }
    }
    Ok(Ok(TradeCreation {
        signer: head.get("signer"),
        signature: head.get("signature"),
        trade_type: head.get("type"),
        network: head.get("network"),
        chain_id: head.get::<i32, _>("chain_id") as i64,
        checks,
        sent,
        received,
    }))
}

/// Rows the digest step settles per statement, the bound of one batch's work.
const DIGEST_BATCH: i64 = 2_000;

/// How long a V3 trade whose digest could not be established waits before
/// the digest step tries it again: permanent failures stay cheap, and a
/// digest that arrives later (an import, a repaired row, a fixed rebuild) is
/// picked up within minutes. SQL interval text.
const DIGEST_RETRY: &str = "10 minutes";

/// Whether `signature` recovers `signer` over `digest`: the digest is the
/// one the trade was signed over. Every trade source verifies an ECDSA
/// signature of an EOA (this node's `POST /v1/trades`, marketplace-server's
/// `resolveTradeSignature`), so a digest that fails here keys no
/// cancellation of that trade.
fn signed_over(digest: &str, signature: &str, signer: &str) -> bool {
    let Ok(bytes) = hex::decode(digest.trim_start_matches("0x")) else {
        return false;
    };
    let Ok(digest) = <[u8; 32]>::try_from(bytes.as_slice()) else {
        return false;
    };
    crate::auth::recover::recover_address_from_digest(&digest, signature)
        .is_ok_and(|recovered| recovered.eq_ignore_ascii_case(signer))
}

/// A V3 trade the digest step settles.
struct DigestCandidate {
    id: uuid::Uuid,
    contract: String,
    signer: String,
    signature: String,
    stored: Option<String>,
}

/// The digest of `c` rebuilt from its stored row against its marketplace's
/// domain on `chain_id`, when the signature recovers the signer over it;
/// `Ok(Err(why))` when it cannot be established.
async fn rebuilt_digest(
    pool: &PgPool,
    chain_id: i64,
    c: &DigestCandidate,
) -> Result<Result<String, String>, sqlx::Error> {
    let Some(domain) = domain_of(chain_id, &c.contract) else {
        return Ok(Err(format!("no EIP-712 domain for {}", c.contract)));
    };
    let trade = match trade_creation(pool, c.id).await? {
        Ok(trade) => trade,
        Err(why) => return Ok(Err(why)),
    };
    let digest = match listings::signing_hash(&trade, &domain) {
        Ok(h) => format!("{h:#x}"),
        Err(e) => return Ok(Err(e.to_string())),
    };
    if !signed_over(&digest, &c.signature, &c.signer) {
        return Ok(Err(
            "the signature does not recover the signer over the digest rebuilt from the row"
                .to_string(),
        ));
    }
    Ok(Ok(digest))
}

/// The digest step: every V3 trade of `net` without a verdict row, and every
/// one whose row has no digest and whose last failure is older than
/// [`DIGEST_RETRY`] (or that never had one: a row the fold created first, or
/// one from before failures were marked), gets its digest: the stored one
/// when the signature recovers the signer over it, else the one rebuilt from
/// the row on the same condition. A row that gains a digest is refolded
/// (`folded_block = -1`), since its cancellation may already be in the log;
/// one that cannot get one is marked `digest_failed_at` and logged. A
/// verified digest is never overwritten. Returns the trades settled: each
/// leaves the candidate set (a digest, or a failure newer than the
/// backoff), so the caller's loop ends.
async fn fill_v3_digests(
    pool: &PgPool,
    net: ChainNet,
    chain_id: i64,
    owner: &chain_walk::Owner,
) -> anyhow::Result<usize> {
    let v3: Vec<String> = deployments(chain_id)
        .iter()
        .filter(|(_, _, v3)| *v3)
        .map(|(a, _, _)| a.to_string())
        .collect();
    let candidates: Vec<DigestCandidate> = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT t.id, lower(t.contract) AS contract, t.signer, t.signature, t.trade_digest FROM shop.trades t \
         LEFT JOIN shop.trade_chain_state s ON s.trade_id = t.id \
         WHERE lower(t.network) = $1 AND lower(t.contract) = ANY($2) \
           AND (s.trade_id IS NULL \
                OR (s.digest IS NULL AND (s.digest_failed_at IS NULL \
                    OR s.digest_failed_at <= timezone('UTC', now()) - interval '{DIGEST_RETRY}'))) \
         LIMIT $3"
    )))
    .bind(net.db_name())
    .bind(&v3)
    .bind(DIGEST_BATCH)
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| DigestCandidate {
        id: r.get("id"),
        contract: r.get("contract"),
        signer: r.get("signer"),
        signature: r.get("signature"),
        stored: r.get("trade_digest"),
    })
    .collect();
    let mut rows: (Vec<uuid::Uuid>, Vec<Option<String>>) = (Vec::new(), Vec::new());
    for c in &candidates {
        let digest = match c
            .stored
            .as_deref()
            .filter(|d| signed_over(d, &c.signature, &c.signer))
        {
            Some(stored) => Ok(stored.to_string()),
            None => rebuilt_digest(pool, chain_id, c).await?,
        };
        rows.0.push(c.id);
        rows.1.push(match digest {
            Ok(d) => Some(d),
            Err(why) => {
                tracing::warn!(trade = %c.id, stored = c.stored.is_some(), error = %why, "trade-events: no verified V3 digest for a stored trade; it is not listed until one is established");
                None
            }
        });
    }
    if !rows.0.is_empty() {
        // Each batch commits on its own, after the owner check.
        let mut tx = pool.begin().await?;
        sqlx::query(
            "INSERT INTO shop.trade_chain_state (trade_id, network, digest, digest_failed_at, status, folded_block) \
             SELECT id, $1, d, CASE WHEN d IS NULL THEN timezone('UTC', now()) END, 'open', -1 \
             FROM UNNEST($2::uuid[], $3::text[]) AS u(id, d) \
             ON CONFLICT (trade_id) DO UPDATE SET \
                 digest = EXCLUDED.digest, \
                 digest_failed_at = EXCLUDED.digest_failed_at, \
                 folded_block = CASE WHEN EXCLUDED.digest IS NULL THEN shop.trade_chain_state.folded_block ELSE -1 END \
             WHERE shop.trade_chain_state.digest IS NULL",
        )
        .bind(net.db_name())
        .bind(&rows.0)
        .bind(&rows.1)
        .execute(&mut *tx)
        .await?;
        owner.check(&mut tx).await?;
        tx.commit().await?;
    }
    Ok(candidates.len())
}

/// The verdict of every touched trade of `net`: trades without a folded row
/// (new mirrored or local trades, the V3 rows the digest step created), the
/// trades a traded or cancelled signature past `since` names (by hash or
/// digest), every trade of a signer whose index moved past `since`, every
/// trade of a contract whose index moved past `since`. `since = -1`
/// refolds every trade of the network (the rebuild). The index legs repeat
/// `new_index IS NOT NULL` (a CHECK makes it true of both index kinds) so the
/// planner can prove `trade_event_index_idx`'s partial predicate: without it
/// the per-trade signer lateral was a sequential scan of `trade_event` per
/// touched trade (a 74,460-trade rebuild: 69 s, 53M buffer hits; with it
/// 0.9 s, `METRICS.md` "Trade-events fold").
const FOLD_SQL: &str = "
WITH touched AS (
    SELECT t.id FROM shop.trades t
    WHERE lower(t.network) = $1
      AND NOT EXISTS (SELECT 1 FROM shop.trade_chain_state s WHERE s.trade_id = t.id AND s.folded_block >= 0)
  UNION
    SELECT t.id FROM shop.trades t WHERE lower(t.network) = $1 AND $2 < 0
  UNION
    SELECT t.id FROM shop.trade_event e JOIN shop.trades t ON t.hashed_signature = e.hashed_signature
    WHERE e.network = $1 AND e.block_number > $2 AND e.hashed_signature IS NOT NULL
  UNION
    SELECT s.trade_id FROM shop.trade_event e JOIN shop.trade_chain_state s ON s.digest = e.hashed_signature
    WHERE e.network = $1 AND e.block_number > $2 AND e.hashed_signature IS NOT NULL
  UNION
    SELECT t.id FROM shop.trade_event e
    JOIN shop.trades t ON t.signer = e.caller AND lower(t.contract) = e.marketplace_address AND lower(t.network) = e.network
    WHERE e.network = $1 AND e.block_number > $2 AND e.kind = 'signer_index_increased' AND e.new_index IS NOT NULL
  UNION
    SELECT t.id FROM shop.trade_event e
    JOIN shop.trades t ON lower(t.contract) = e.marketplace_address AND lower(t.network) = e.network
    WHERE e.network = $1 AND e.block_number > $2 AND e.kind = 'contract_index_increased' AND e.new_index IS NOT NULL
),
verdict AS (
    SELECT t.id, lower(t.network) AS network, d.digest, x.executions, x.executed_at,
           COALESCE((t.checks->>'uses')::numeric, 1) AS uses,
           (c.cancelled OR si.moved OR ci.moved) AS cancelled
    FROM touched tt
    JOIN shop.trades t ON t.id = tt.id
    LEFT JOIN shop.trade_chain_state d ON d.trade_id = t.id
    CROSS JOIN LATERAL (
        SELECT count(*)::int AS executions, max(e.block_time) AS executed_at FROM shop.trade_event e
        WHERE e.hashed_signature IN (t.hashed_signature, d.digest) AND e.kind = 'traded' AND e.network = lower(t.network)
          AND e.marketplace_address = lower(t.contract) AND e.signer = t.signer
    ) x
    CROSS JOIN LATERAL (
        SELECT EXISTS (SELECT 1 FROM shop.trade_event e
            WHERE e.kind = 'signature_cancelled' AND e.hashed_signature IN (t.hashed_signature, d.digest)
              AND e.caller = t.signer AND e.network = lower(t.network) AND e.marketplace_address = lower(t.contract)) AS cancelled
    ) c
    CROSS JOIN LATERAL (
        SELECT COALESCE((SELECT e.new_index FROM shop.trade_event e
            WHERE e.network = lower(t.network) AND e.marketplace_address = lower(t.contract) AND e.kind = 'signer_index_increased' AND e.caller = t.signer
              AND e.new_index IS NOT NULL
            ORDER BY e.block_number DESC, e.log_index DESC LIMIT 1), 0)
          <> COALESCE((t.checks->>'signerSignatureIndex')::numeric, 0) AS moved
    ) si
    CROSS JOIN LATERAL (
        SELECT COALESCE((SELECT e.new_index FROM shop.trade_event e
            WHERE e.network = lower(t.network) AND e.marketplace_address = lower(t.contract) AND e.kind = 'contract_index_increased'
              AND e.new_index IS NOT NULL
            ORDER BY e.block_number DESC, e.log_index DESC LIMIT 1), 0)
          <> COALESCE((t.checks->>'contractSignatureIndex')::numeric, 0) AS moved
    ) ci
)
INSERT INTO shop.trade_chain_state (trade_id, network, digest, executions, status, executed_at, folded_block)
SELECT id, network, digest, executions,
       CASE WHEN cancelled THEN 'cancelled' WHEN executions >= uses THEN 'sold' ELSE 'open' END,
       executed_at, $3
FROM verdict
ON CONFLICT (trade_id) DO UPDATE SET
    executions = EXCLUDED.executions,
    status = EXCLUDED.status,
    executed_at = EXCLUDED.executed_at,
    folded_block = EXCLUDED.folded_block";

/// The digest step until no V3 trade is left to settle, then the verdict
/// fold, in one transaction for the fold. Returns the verdicts written.
pub async fn fold(
    pool: &PgPool,
    net: ChainNet,
    chain_id: i64,
    since: Option<i64>,
    upto: i64,
    owner: &chain_walk::Owner,
) -> anyhow::Result<u64> {
    while fill_v3_digests(pool, net, chain_id, owner).await? as i64 >= DIGEST_BATCH {}
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL statement_timeout = '1800s'")
        .execute(&mut *tx)
        .await?;
    super::chain_walk::analyze_if_stale(&mut tx, "shop", "trade_event", since.is_none()).await?;
    let written = sqlx::query(FOLD_SQL)
        .bind(net.db_name())
        .bind(since.unwrap_or(-1))
        .bind(upto)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    owner.check(&mut tx).await?;
    tx.commit().await?;
    Ok(written)
}

/// One network's side of the walk, on the chain its RPC serves: the
/// network's mainnet, or its testnet (`MARKETPLACE_INDEX_CHAIN_ID_*`).
pub struct TradeEventsIndex {
    pub net: ChainNet,
    pub chain_id: i64,
}

impl TradeEventsIndex {
    /// The walk of `net` on its mainnet.
    pub fn mainnet(net: ChainNet) -> Self {
        Self {
            net,
            chain_id: net.chain_id(),
        }
    }
}

#[async_trait::async_trait]
impl ChainIndex for TradeEventsIndex {
    fn indexer(&self) -> &'static str {
        INDEXER_TRADE_EVENTS
    }
    fn chain_id(&self) -> i64 {
        self.chain_id
    }

    fn net(&self) -> ChainNet {
        self.net
    }

    fn start_block(&self) -> u64 {
        deployments(self.chain_id)
            .iter()
            .map(|(_, b, _)| *b)
            .min()
            .unwrap_or(0)
    }

    async fn groups(
        &self,
        _pool: &PgPool,
        _from: u64,
        _to: u64,
        _anchor: (u64, i64),
    ) -> anyhow::Result<Vec<LogGroup>> {
        Ok(vec![LogGroup {
            addresses: deployments(self.chain_id)
                .iter()
                .map(|(a, _, _)| a.to_string())
                .collect(),
            topics: json!([ALL_TOPICS]),
        }])
    }

    async fn write(
        &self,
        conn: &mut PgConnection,
        logs: &[RpcLog],
        times: &HashMap<i64, i64>,
    ) -> anyhow::Result<usize> {
        let mut rows = Vec::with_capacity(logs.len());
        for log in logs {
            let block = chain_walk::log_position(log).0;
            if let Some(row) = decode_log(log, times.get(&block).copied().unwrap_or(0))? {
                rows.push(row);
            }
        }
        Ok(insert_events(conn, self.net, &rows).await?)
    }

    async fn clear(&self, conn: &mut PgConnection, to: i64) -> anyhow::Result<()> {
        self.rewind(conn, to).await?;
        // A rebuild trusts nothing derived: the digests are recomputed from
        // the stored trades and every verdict refolded.
        sqlx::query("DELETE FROM shop.trade_chain_state WHERE network = $1")
            .bind(self.net.db_name())
            .execute(&mut *conn)
            .await?;
        Ok(())
    }
    async fn rewind(&self, conn: &mut PgConnection, to: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM shop.trade_event WHERE network = $1 AND block_number > $2")
            .bind(self.net.db_name())
            .bind(to)
            .execute(&mut *conn)
            .await?;
        // Every verdict is refolded from the remaining log at the next
        // publish (the cursor's folded_block is cleared: since = rebuild).
        sqlx::query("UPDATE shop.trade_chain_state SET folded_block = -1 WHERE network = $1")
            .bind(self.net.db_name())
            .execute(&mut *conn)
            .await?;
        Ok(())
    }

    async fn fold(
        &self,
        pool: &PgPool,
        since: Option<i64>,
        upto: i64,
        owner: &chain_walk::Owner,
    ) -> anyhow::Result<bool> {
        fold(pool, self.net, self.chain_id, since, upto, owner).await?;
        Ok(true)
    }
}

/// The networks whose trade verdicts readers may use at `now`, each read on
/// the chain `indexes` configures for it.
pub async fn ready_networks(
    pool: &PgPool,
    indexes: &chain_walk::IndexSettings,
    now: chrono::DateTime<Utc>,
) -> Vec<ChainNet> {
    let mut out = Vec::new();
    for net in ChainNet::ALL {
        if chain_walk::cursor(pool, INDEXER_TRADE_EVENTS, net)
            .await
            .ok()
            .flatten()
            .and_then(|c| chain_walk::fresh_fold_on(&c, indexes.chain_id(net), now))
            .is_some()
        {
            out.push(net);
        }
    }
    out
}

/// The SQL predicate "the chain closed trade `t`", with the readiness rule
/// inline (a bound `now_p`): a verdict of a network whose index is not
/// published, or is bound to another chain than this process serves, is
/// ignored ([`verdicts_ready_sql`]). For the duplicate-listing refusal,
/// where an unpublished verdict must keep counting the trade as open; the
/// open-listing feeds read [`listable_on_chain_sql`].
pub fn closed_on_chain_sql(t: &str, now_p: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM shop.trade_chain_state tcs \
                 JOIN shop.chain_cursor tcc ON tcc.indexer = 'trade_events' AND tcc.network = tcs.network \
                 WHERE tcs.trade_id = {t}.id AND tcs.status <> 'open' AND {ready})",
        ready = verdicts_ready_sql("tcc", now_p)
    )
}

/// The SQL predicate "the trade-events verdicts of trade `t`'s network are
/// current" (a bound `now_p`): its cursor is published, recent and bound to
/// the chain this process serves ([`verdicts_ready_sql`]).
pub fn verdicts_current_sql(t: &str, now_p: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM shop.chain_cursor tcr \
                 WHERE tcr.indexer = 'trade_events' AND tcr.network = lower({t}.network) AND {ready})",
        ready = verdicts_ready_sql("tcr", now_p)
    )
}

/// The SQL predicate "trade `t` is a V3 trade whose cancellation digest
/// could not be established" (`trade_chain_state.digest_failed_at`): its
/// cancellation would go unseen.
pub fn unverifiable_sql(t: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM shop.trade_chain_state tcu \
                 WHERE tcu.trade_id = {t}.id AND tcu.digest_failed_at IS NOT NULL)"
    )
}

/// The SQL predicate "trade `t` may be served as an open listing" for the
/// open-listing feeds (a bound `now_p`): its network's verdicts are current
/// ([`verdicts_current_sql`]) and no verdict closed it (sold, cancelled) or
/// left it unverifiable ([`unverifiable_sql`]). Fails closed: a trade of a
/// network whose index is behind is not listed, whatever its row says.
pub fn listable_on_chain_sql(t: &str, now_p: &str) -> String {
    format!(
        "({current} AND NOT EXISTS (SELECT 1 FROM shop.trade_chain_state tcs \
                 WHERE tcs.trade_id = {t}.id AND (tcs.status <> 'open' OR tcs.digest_failed_at IS NOT NULL)))",
        current = verdicts_current_sql(t, now_p)
    )
}

/// The networks of `nets` whose trade-events verdicts are not current at
/// `now` on `conn` ([`verdicts_ready_sql`]), in the order asked.
pub async fn stale_networks_on(
    conn: &mut PgConnection,
    nets: &[ChainNet],
    now: chrono::DateTime<Utc>,
) -> Result<Vec<ChainNet>, sqlx::Error> {
    let names: Vec<&str> = nets.iter().map(|n| n.db_name()).collect();
    let stale: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT n.network FROM unnest($2::text[]) WITH ORDINALITY AS n(network, i) \
         WHERE NOT EXISTS (SELECT 1 FROM shop.chain_cursor c \
                           WHERE c.indexer = 'trade_events' AND c.network = n.network AND {ready}) \
         ORDER BY n.i",
        ready = verdicts_ready_sql("c", "$1")
    )))
    .bind(now.naive_utc())
    .bind(&names)
    .fetch_all(conn)
    .await?;
    Ok(stale.iter().filter_map(|n| ChainNet::parse(n)).collect())
}

/// What the envelope says of the networks whose verdicts are not current.
pub fn stale_message(stale: &[ChainNet]) -> String {
    let names: Vec<&str> = stale.iter().map(|n| n.db_name()).collect();
    format!(
        "the trade-events index has not published a fold on {} within the last 30 minutes: sold and cancelled off-chain listings cannot be told from open ones until it does",
        names.join(" and ")
    )
}

/// `Err(envelope)` unless the trade-events verdicts are current on every
/// network ([`stale_networks_on`]): the gate of every route that lists,
/// prices or reports the status of trades (`trades::ready`). A database
/// error reads as not current: the envelope, never a guess.
pub async fn require_current(state: &MarketplaceState) -> Result<(), Response> {
    let mut conn = match state.pool.acquire().await {
        Ok(conn) => conn,
        Err(e) => return Err(unreadable(e)),
    };
    require_current_on(&mut conn, Utc::now()).await
}

/// [`require_current`] read on `conn` at `now`: a reader that runs its
/// reads in one snapshot decides on the cursors that snapshot sees.
pub async fn require_current_on(
    conn: &mut PgConnection,
    now: chrono::DateTime<Utc>,
) -> Result<(), Response> {
    match stale_networks_on(conn, &ChainNet::ALL, now).await {
        Ok(stale) if stale.is_empty() => Ok(()),
        Ok(stale) => Err(unavailable(
            Source::TradeEventsIndex,
            &stale_message(&stale),
        )),
        Err(e) => Err(unreadable(e)),
    }
}

/// The envelope of a cursor read that failed: not current, never a guess.
fn unreadable(e: sqlx::Error) -> Response {
    tracing::warn!(error = %e, "cannot read shop.chain_cursor; treating the trade-events index as not current");
    unavailable(
        Source::TradeEventsIndex,
        "the trade-events index state cannot be read",
    )
}

/// The readiness rule of the trade-events cursor as the verdict SQL reads
/// it: the publish marker ([`chain_walk::published_sql_on`]: a fold
/// published within 30 minutes, not `folded_block = last_block`, which every
/// pass breaks while it walks and folds), with the chains this process serves
/// ([`super::catalog::served_chains`], the trade-events walk's
/// `MARKETPLACE_INDEX_CHAIN_ID_*`), so a cursor bound to another chain never
/// lends its verdicts.
fn verdicts_ready_sql(alias: &str, now_p: &str) -> String {
    let (ethereum, matic) = super::catalog::served_chains();
    chain_walk::published_sql_on(alias, now_p, ethereum, matic)
}

/// The chain verdict of trade `t` (`sold` | `cancelled`), NULL when open or
/// when its network's index is not published.
pub fn chain_status_sql(t: &str, now_p: &str) -> String {
    format!(
        "(SELECT tcs.status FROM shop.trade_chain_state tcs \
          JOIN shop.chain_cursor tcc ON tcc.indexer = 'trade_events' AND tcc.network = tcs.network \
          WHERE tcs.trade_id = {t}.id AND tcs.status <> 'open' AND {ready})",
        ready = verdicts_ready_sql("tcc", now_p)
    )
}

/// What the notification needs of one asset.
#[derive(Clone, Debug, Default)]
pub(crate) struct AssetMeta {
    pub(crate) image: String,
    pub(crate) seller: String,
    pub(crate) category: String,
    pub(crate) rarity: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) contract_address: String,
    pub(crate) token_id: Option<String>,
    pub(crate) item_id: Option<String>,
}

impl AssetMeta {
    fn link(&self, base: &str) -> String {
        match &self.token_id {
            Some(token) => format!("{base}/contracts/{}/tokens/{token}", self.contract_address),
            None => format!(
                "{base}/contracts/{}/items/{}",
                self.contract_address,
                self.item_id.as_deref().unwrap_or_default()
            ),
        }
    }

    fn token_or_item_id(&self) -> String {
        self.token_id
            .clone()
            .or_else(|| self.item_id.clone())
            .unwrap_or_default()
    }
}

fn insert_opt(map: &mut serde_json::Map<String, Value>, key: &str, v: &Option<String>) {
    if let Some(val) = v {
        map.insert(key.into(), json!(val));
    }
}

pub(crate) fn bid_accepted_event(trade: &StoredTrade, asset: &AssetMeta, base: &str) -> Value {
    let price = trade.sent_amount.clone().unwrap_or_default();
    let mut m = serde_json::Map::new();
    m.insert("address".into(), json!(trade.signer));
    m.insert("image".into(), json!(asset.image));
    m.insert("seller".into(), json!(asset.seller));
    m.insert("category".into(), json!(asset.category));
    insert_opt(&mut m, "rarity", &asset.rarity);
    m.insert("link".into(), json!(asset.link(base)));
    insert_opt(&mut m, "nftName", &asset.name);
    m.insert("price".into(), json!(price));
    m.insert("title".into(), json!("Bid Accepted"));
    m.insert(
        "description".into(),
        json!(format!(
            "Your bid for {} MANA for this {} was accepted.",
            format_ether(&price),
            asset.name.as_deref().unwrap_or_default()
        )),
    );
    m.insert("network".into(), json!(trade.network));
    json!({ "type": "blockchain", "subType": "bid-accepted", "key": format!("bid-accepted-{}", trade.id), "timestamp": 0, "metadata": Value::Object(m) })
}

const BID_NOTIFICATION_NS: uuid::Uuid =
    uuid::Uuid::from_u128(0x61a4_0d93_c2e7_4b18_9f35_7e0b_a8c6_14d2);

/// The asset a bid is on, as the indexes describe it; `None` when none does.
pub(super) async fn bid_asset(
    pool: &PgPool,
    land_image_base: &str,
    network: &str,
    bid: &listings::TradeCreation,
) -> Result<Option<AssetMeta>, sqlx::Error> {
    let Some(asset) = bid.received.first() else {
        return Ok(None);
    };
    let net = network.to_ascii_lowercase();
    let contract = asset.contract_address.to_ascii_lowercase();
    Ok(match (asset.asset_type, &asset.token_id, &asset.item_id) {
        (crate::types::ASSET_TYPE_ERC721, Some(token), _) => {
            nft_meta(pool, land_image_base, &net, &contract, token).await?
        }
        (crate::types::ASSET_TYPE_COLLECTION_ITEM, _, Some(item)) => {
            item_meta(pool, &net, &contract, item).await?
        }
        _ => None,
    })
}

/// The stored metadata of a `bid_received` notification, local or on-chain:
/// the asset's seller is the recipient, `amount` the price as the
/// description shows it.
pub(crate) fn bid_received_metadata(
    meta: &AssetMeta,
    link: String,
    price: &str,
    amount: &str,
    network: &str,
) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("image".into(), json!(meta.image));
    m.insert("seller".into(), json!(meta.seller));
    m.insert("category".into(), json!(meta.category));
    insert_opt(&mut m, "rarity", &meta.rarity);
    m.insert("link".into(), json!(link));
    insert_opt(&mut m, "nftName", &meta.name);
    m.insert("price".into(), json!(price));
    m.insert("title".into(), json!("Bid Received"));
    m.insert(
        "description".into(),
        json!(format!(
            "You received a bid of {amount} MANA for this {}.",
            meta.name.as_deref().unwrap_or_default()
        )),
    );
    m.insert("network".into(), json!(network));
    Value::Object(m)
}

/// marketplace-server's BID_RECEIVED event of a local bid as the old parser
/// stored it, for the asset's seller (item creator or nft owner); `None`
/// without a price or when the bidder is the seller.
pub(super) fn bid_received_notification(
    meta: &AssetMeta,
    marketplace_base: &str,
    trade_id: &str,
    network: &str,
    bid: &listings::TradeCreation,
) -> Option<crate::shared::notifications::NewNotification> {
    let price = bid.sent.first().and_then(|a| a.amount.as_ref())?;
    if meta.seller.eq_ignore_ascii_case(&bid.signer) {
        return None;
    }
    let recipient = crate::shared::identity::Address::parse(&meta.seller).ok()?;
    let metadata = bid_received_metadata(
        meta,
        format!("{marketplace_base}/account?section=bids"),
        price,
        &format_ether(price),
        network,
    );
    let mut n =
        crate::shared::notifications::NewNotification::now(recipient, "bid_received", metadata);
    n.id = uuid::Uuid::new_v5(
        &BID_NOTIFICATION_NS,
        format!("bid-created:{trade_id}").as_bytes(),
    );
    Some(n)
}

pub(crate) fn item_sold_event(
    trade: &StoredTrade,
    asset: &AssetMeta,
    caller: &str,
    base: &str,
) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("address".into(), json!(trade.signer));
    m.insert("image".into(), json!(asset.image));
    m.insert("seller".into(), json!(asset.seller));
    m.insert("buyer".into(), json!(caller));
    m.insert("category".into(), json!(asset.category));
    insert_opt(&mut m, "rarity", &asset.rarity);
    m.insert("link".into(), json!(asset.link(base)));
    insert_opt(&mut m, "nftName", &asset.name);
    m.insert("title".into(), json!("Item Sold"));
    m.insert(
        "description".into(),
        json!(format!(
            "Someone just bought your {}",
            asset.name.as_deref().unwrap_or_default()
        )),
    );
    m.insert("network".into(), json!(trade.network));
    m.insert("tokenId".into(), json!(asset.token_or_item_id()));
    json!({ "type": "blockchain", "subType": "item-sold", "key": format!("item-sold-{}", trade.id), "timestamp": 0, "metadata": Value::Object(m) })
}

pub(crate) struct StoredTrade {
    pub(crate) id: String,
    pub(crate) network: String,
    pub(crate) signer: String,
    pub(crate) trade_type: String,
    pub(crate) contract: String,
    /// The first sent erc20 amount (a bid's price).
    pub(crate) sent_amount: Option<String>,
    /// (asset_type, contract, token or item id) of every sent then received asset.
    pub(crate) assets: Vec<(i32, String, Option<String>, Option<String>)>,
}

async fn stored_trade(
    conn: &mut PgConnection,
    hashed_signature: &str,
) -> Result<Option<StoredTrade>, sqlx::Error> {
    let Some(head) = sqlx::query("SELECT id::text AS id, network, signer, type::text AS type, contract FROM shop.trades WHERE hashed_signature = $1")
        .bind(hashed_signature)
        .fetch_optional(&mut *conn)
        .await?
    else {
        return Ok(None);
    };
    let assets = sqlx::query(
        "SELECT ta.direction::text AS direction, ta.asset_type::int4 AS asset_type, ta.contract_address, erc20.amount::text AS amount, erc721.token_id, item.item_id \
         FROM shop.trade_assets ta \
         LEFT JOIN shop.trade_assets_erc20 erc20 ON erc20.asset_id = ta.id \
         LEFT JOIN shop.trade_assets_erc721 erc721 ON erc721.asset_id = ta.id \
         LEFT JOIN shop.trade_assets_item item ON item.asset_id = ta.id \
         WHERE ta.trade_id = $1::uuid ORDER BY ta.direction ASC, ta.created_at, ta.id",
    )
    .bind(head.get::<String, _>("id"))
    .fetch_all(&mut *conn)
    .await?;
    let sent_amount = assets
        .iter()
        .find(|a| a.get::<String, _>("direction") == "sent")
        .and_then(|a| a.get::<Option<String>, _>("amount"));
    Ok(Some(StoredTrade {
        id: head.get("id"),
        network: head.get("network"),
        signer: head.get("signer"),
        trade_type: head.get("type"),
        contract: head.get::<String, _>("contract").to_ascii_lowercase(),
        sent_amount,
        assets: assets
            .iter()
            .map(|a| {
                (
                    a.get("asset_type"),
                    a.get::<String, _>("contract_address").to_ascii_lowercase(),
                    a.get("token_id"),
                    a.get("item_id"),
                )
            })
            .collect(),
    }))
}

fn category_of_item_type(item_type: &str) -> &'static str {
    if item_type == "emote_v1" {
        "emote"
    } else {
        "wearable"
    }
}

/// An item asset from the items mirror: the creator is the seller.
pub(crate) async fn item_meta<'e>(
    conn: impl sqlx::PgExecutor<'e>,
    network: &str,
    contract: &str,
    item_id: &str,
) -> Result<Option<AssetMeta>, sqlx::Error> {
    let row = sqlx::query("SELECT image, creator, rarity, name, item_type FROM shop.items WHERE network = $1 AND contract_address = $2 AND item_id = $3::numeric")
        .bind(network)
        .bind(contract)
        .bind(item_id)
        .fetch_optional(conn)
        .await?;
    Ok(row.map(|r| AssetMeta {
        image: r.get::<Option<String>, _>("image").unwrap_or_default(),
        seller: r.get("creator"),
        category: category_of_item_type(&r.get::<String, _>("item_type")).to_string(),
        rarity: r.get("rarity"),
        name: r.get("name"),
        contract_address: contract.to_string(),
        token_id: None,
        item_id: Some(item_id.to_string()),
    }))
}

/// [`item_meta`] of many `(contract, item_id)` at once, keyed as asked.
pub(crate) async fn item_metas(
    pool: &PgPool,
    network: &str,
    keys: &[(String, String)],
) -> Result<HashMap<(String, String), AssetMeta>, sqlx::Error> {
    let (contracts, items): (Vec<String>, Vec<String>) = keys
        .iter()
        .filter(|(_, i)| !i.is_empty() && i.bytes().all(|b| b.is_ascii_digit()))
        .cloned()
        .unzip();
    let rows = sqlx::query(
        "SELECT u.contract, u.item, i.image, i.creator, i.rarity, i.name, i.item_type \
         FROM UNNEST($2::text[], $3::text[]) AS u(contract, item) \
         JOIN shop.items i ON i.network = $1 AND i.contract_address = u.contract AND i.item_id = u.item::numeric",
    )
    .bind(network)
    .bind(&contracts)
    .bind(&items)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let (contract, item): (String, String) = (r.get("contract"), r.get("item"));
            let meta = AssetMeta {
                image: r.get::<Option<String>, _>("image").unwrap_or_default(),
                seller: r.get("creator"),
                category: category_of_item_type(&r.get::<String, _>("item_type")).to_string(),
                rarity: r.get("rarity"),
                name: r.get("name"),
                contract_address: contract.clone(),
                token_id: None,
                item_id: Some(item.clone()),
            };
            ((contract, item), meta)
        })
        .collect())
}

/// [`nft_meta`] of many `(contract, token_id)` at once, keyed as asked.
pub(crate) async fn nft_metas(
    pool: &PgPool,
    land_image_base: &str,
    network: &str,
    keys: &[(String, String)],
) -> Result<HashMap<(String, String), AssetMeta>, sqlx::Error> {
    let found = super::nft_reads::asset_metas(pool, land_image_base, network, keys).await?;
    Ok(found
        .into_iter()
        .map(|((contract, token), n)| {
            let meta = AssetMeta {
                image: n.image,
                seller: n.owner,
                category: n.category,
                rarity: n.rarity,
                name: n.name,
                contract_address: contract.clone(),
                token_id: Some(token.clone()),
                item_id: None,
            };
            ((contract, token), meta)
        })
        .collect())
}

/// An nft asset from the nft index (collections and names) or the LAND
/// index: its current owner is the seller, as the old squid read had it.
pub(crate) async fn nft_meta<'e>(
    conn: impl sqlx::PgExecutor<'e>,
    land_image_base: &str,
    network: &str,
    contract: &str,
    token_id: &str,
) -> Result<Option<AssetMeta>, sqlx::Error> {
    if let Some(n) =
        super::nft_reads::asset_meta(conn, land_image_base, network, contract, token_id).await?
    {
        return Ok(Some(AssetMeta {
            image: n.image,
            seller: n.owner,
            category: n.category,
            rarity: n.rarity,
            name: n.name,
            contract_address: contract.to_string(),
            token_id: Some(token_id.to_string()),
            item_id: None,
        }));
    }
    Ok(None)
}

/// The event of an executed trade, or why there is none. `Ok(None)`: the
/// trade is known and the chain has no execution of it.
async fn accepted_event(
    state: &MarketplaceState,
    conn: &mut PgConnection,
    published_block: i64,
    trade: &StoredTrade,
    caller: &str,
) -> Result<Option<Value>, Response> {
    let network = trade.network.to_ascii_lowercase();
    let executed: Option<i64> = sqlx::query_scalar(
        "SELECT e.block_time FROM shop.trade_event e \
         WHERE e.hashed_signature = (SELECT hashed_signature FROM shop.trades WHERE id = $1::uuid) \
           AND e.kind = 'traded' AND e.network = $2 AND e.marketplace_address = $3 AND e.block_number <= $4 \
         ORDER BY e.block_number DESC, e.log_index DESC LIMIT 1",
    )
    .bind(&trade.id)
    .bind(&network)
    .bind(&trade.contract)
    .bind(published_block)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| super::wire::ApiError::internal(e.to_string()).into_response())?;
    if executed.is_none() {
        return Ok(None);
    }
    let mut metas = Vec::new();
    for (asset_type, contract, token_id, item_id) in &trade.assets {
        let meta = match (*asset_type, token_id, item_id) {
            (crate::types::ASSET_TYPE_ERC721, Some(token), _) => {
                nft_meta(
                    &mut *conn,
                    &state.settings.land_image_base_url,
                    &network,
                    contract,
                    token,
                )
                .await
            }
            (crate::types::ASSET_TYPE_COLLECTION_ITEM, _, Some(item)) => {
                item_meta(&mut *conn, &network, contract, item).await
            }
            _ => Ok(None),
        }
        .map_err(|e| super::wire::ApiError::internal(e.to_string()).into_response())?;
        metas.extend(meta);
    }
    let base = state.settings.market.marketplace_base_url.as_str();
    let event = match (trade.trade_type.as_str(), metas.as_slice()) {
        ("bid", [asset]) => Some(bid_accepted_event(trade, asset, base)),
        ("public_item_order" | "public_nft_order", [asset]) => {
            Some(item_sold_event(trade, asset, caller, base))
        }
        _ => None,
    };
    Ok(Some(event.unwrap_or(Value::Null)))
}

fn envelope_error(status: StatusCode, message: String) -> Response {
    (status, Json(json!({ "ok": false, "message": message }))).into_response()
}

/// `GET /v1/trades/{hashed_signature}/accept?timestamp=&caller=`: the
/// notification event of an executed trade, `{ok: true, data}` (the event's
/// `timestamp` is the query's, else now: the upstream rule). A signature
/// this node does not store is the upstream 404; before the trade's network
/// is published the envelope; a trade the chain never executed is a 404
/// naming that (the upstream built the event unconditionally, trusting its
/// caller; here the chain decides); an executed trade whose assets the
/// indexes cannot describe is the upstream 500 "Notification event was not
/// generated".
pub async fn get_trade_accepted_event(
    State(state): State<SharedState>,
    Path(hashed_signature): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let hashed_signature = hashed_signature.to_ascii_lowercase();
    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => return unreadable(e),
    };
    if let Err(e) = sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await
    {
        return unreadable(e);
    }
    let trade = match stored_trade(&mut tx, &hashed_signature).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return envelope_error(
                StatusCode::NOT_FOUND,
                format!("Trade with hashed signature {hashed_signature} not found"),
            );
        }
        Err(e) => return super::wire::ApiError::internal(e.to_string()).into_response(),
    };
    let Some(net) = ChainNet::parse(&trade.network) else {
        return envelope_error(
            StatusCode::NOT_FOUND,
            format!("Trade with hashed signature {hashed_signature} not found"),
        );
    };
    let chain = state.settings.market.indexes.chain_id(net);
    let cursor = match chain_walk::cursor(&mut *tx, INDEXER_TRADE_EVENTS, net).await {
        Ok(cursor) => cursor,
        Err(e) => return unreadable(e),
    };
    let Some(published_block) =
        cursor.and_then(|c| chain_walk::fresh_fold_on(&c, chain, Utc::now()))
    else {
        return unavailable(
            Source::TradeEventsIndex,
            &format!(
                "the {} off-chain marketplace's Traded events are not indexed to the chain head yet",
                net.db_name()
            ),
        );
    };
    let timestamp = pairs
        .iter()
        .find(|(k, _)| k == "timestamp")
        .and_then(|(_, v)| v.parse::<f64>().ok())
        .map(|v| v as i64)
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    let caller = pairs
        .iter()
        .find(|(k, _)| k == "caller")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    match accepted_event(&state, &mut tx, published_block, &trade, &caller).await {
        Err(r) => r,
        Ok(None) => envelope_error(
            StatusCode::NOT_FOUND,
            format!(
                "Trade with hashed signature {hashed_signature} has not been executed on chain"
            ),
        ),
        Ok(Some(Value::Null)) => envelope_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Notification event was not generated".to_string(),
        ),
        Ok(Some(mut event)) => {
            event["timestamp"] = json!(timestamp);
            Json(json!({ "ok": true, "data": event })).into_response()
        }
    }
}

/// The index's state for the operator status route and the tests, each
/// network read on the chain `indexes` configures for it.
pub async fn status(pool: &PgPool, indexes: &chain_walk::IndexSettings) -> Value {
    let mut out = serde_json::Map::new();
    for net in ChainNet::ALL {
        let c = chain_walk::cursor(pool, INDEXER_TRADE_EVENTS, net)
            .await
            .ok()
            .flatten();
        out.insert(
            net.db_name().to_string(),
            json!(c.map(|c| json!({
            "lastBlock": c.last_block, "foldedBlock": c.folded_block, "fullHistory": c.full_history,
            "ready": chain_walk::fresh_fold_on(&c, indexes.chain_id(net), Utc::now()).is_some(), "reorgRewinds": c.reorg_rewinds, "divergedAt": c.diverged_at,
            "chainId": c.chain_id, "quarantine": c.quarantine_reason,
        }))),
        );
    }
    Value::Object(out)
}

pub fn parse_address(s: &str) -> Option<Address> {
    Address::from_str(s).ok()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloy_primitives::{B256, FixedBytes, U256, keccak256};
    use alloy_sol_types::SolValue;

    #[test]
    fn the_topics_are_the_keccak_of_their_signatures() {
        let tuple = "(address,bytes,(uint256,uint256,uint256,bytes32,uint256,uint256,bytes32,bytes32[],(address,bytes4,bytes,bool)[]),(uint256,address,uint256,address,bytes)[],(uint256,address,uint256,address,bytes)[])";
        assert_eq!(
            format!(
                "{:#x}",
                keccak256(format!("Traded(address,bytes32,{tuple})"))
            ),
            TOPIC_TRADED
        );
        assert_eq!(
            format!(
                "{:#x}",
                keccak256(format!("Traded(address,bytes32,bytes32,{tuple})"))
            ),
            TOPIC_TRADED_V3
        );
        assert_eq!(
            format!("{:#x}", keccak256("SignatureCancelled(address,bytes32)")),
            TOPIC_SIGNATURE_CANCELLED
        );
        assert_eq!(
            format!(
                "{:#x}",
                keccak256("SignerSignatureIndexIncreased(address,uint256)")
            ),
            TOPIC_SIGNER_INDEX_INCREASED
        );
        assert_eq!(
            format!(
                "{:#x}",
                keccak256("ContractSignatureIndexIncreased(address,uint256)")
            ),
            TOPIC_CONTRACT_INDEX_INCREASED
        );
    }

    #[test]
    fn every_marketplace_the_listings_verify_against_is_walked() {
        for chain_id in OFFCHAIN_MARKETPLACE_CHAINS {
            let walked = deployments(chain_id);
            for m in listings::offchain_marketplaces(chain_id) {
                assert!(
                    walked.iter().any(|(a, _, _)| *a == m.address),
                    "{} missing on {chain_id}",
                    m.address,
                );
                assert!(domain_of(chain_id, m.address).is_some());
                // V3 is the deployment that cancels by digest, on every chain.
                let (_, _, v3) = walked.iter().find(|(a, _, _)| *a == m.address).unwrap();
                assert_eq!(*v3, m.cancels_by_digest, "{} on {chain_id}", m.address);
            }
        }
    }

    #[test]
    fn a_network_walks_its_mainnet_unless_told_its_testnet() {
        assert_eq!(marketplaces(ChainNet::Matic), deployments(137));
        assert_eq!(marketplaces(ChainNet::Ethereum), deployments(1));
        let mainnet = TradeEventsIndex::mainnet(ChainNet::Matic);
        assert_eq!(mainnet.chain_id, 137);
        assert_eq!(mainnet.start_block(), 62_349_044);
        let amoy = TradeEventsIndex {
            net: ChainNet::Matic,
            chain_id: 80_002,
        };
        assert_eq!(amoy.start_block(), 12_490_079);
        let sepolia = TradeEventsIndex {
            net: ChainNet::Ethereum,
            chain_id: 11_155_111,
        };
        assert_eq!(sepolia.start_block(), 6_770_056);
        // No mainnet address is walked on a testnet, and an unknown chain walks nothing.
        for (a, _, _) in deployments(80_002) {
            assert!(!deployments(137).iter().any(|(m, _, _)| m == a), "{a}");
        }
        assert!(deployments(10).is_empty());
    }

    /// Publishes the trade-events index on both networks, each on its
    /// mainnet chain, current now: the state the routes that list, price or
    /// report the status of trades require ([`require_current`]), for the
    /// tests that read trades without walking the index. A cursor already
    /// there is made current.
    pub(crate) async fn publish_cursors(pool: &PgPool) {
        publish_cursors_at(pool, None).await
    }

    /// [`publish_cursors`] folded at `at` (a read's fixed clock), else now.
    pub(crate) async fn publish_cursors_at(pool: &PgPool, at: Option<chrono::NaiveDateTime>) {
        for net in ChainNet::ALL {
            sqlx::query(
                "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, fold_through, folded_block, folded_at, full_history, chain_id) \
                 VALUES ('trade_events', $1, 0, 10, 10, 10, COALESCE($3, timezone('UTC', now())), true, $2) \
                 ON CONFLICT (indexer, network) DO UPDATE SET folded_block = shop.chain_cursor.last_block, \
                     fold_through = shop.chain_cursor.last_block, folded_at = EXCLUDED.folded_at, \
                     full_history = true, chain_id = EXCLUDED.chain_id, quarantined_at = NULL, quarantine_reason = NULL",
            )
            .bind(net.db_name())
            .bind(net.chain_id())
            .bind(at)
            .execute(pool)
            .await
            .expect("the trade-events cursors are published");
        }
    }

    pub(crate) fn topic_of(addr: &str) -> String {
        format!("0x{:0>64}", addr.trim_start_matches("0x"))
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "fixture: one argument per Traded log field"
    )]
    pub(crate) fn traded_log(
        block: u64,
        index: u64,
        market: &str,
        caller: &str,
        signer: &str,
        signature: &str,
        uses: u64,
        v3: bool,
    ) -> RpcLog {
        let trade = TradeTuple {
            signer: Address::from_str(signer).unwrap(),
            signature: hex::decode(signature.trim_start_matches("0x"))
                .unwrap()
                .into(),
            checks: ChecksTuple {
                uses: U256::from(uses),
                expiration: U256::from(1u64),
                effective: U256::ZERO,
                salt: FixedBytes::ZERO,
                contractSignatureIndex: U256::ZERO,
                signerSignatureIndex: U256::ZERO,
                allowedRoot: FixedBytes::ZERO,
                allowedProof: vec![],
                externalChecks: vec![],
            },
            sent: vec![AssetTuple {
                assetType: U256::from(3u8),
                contractAddress: Address::from_str("0x00000000000000000000000000000000000000cc")
                    .unwrap(),
                value: U256::from(7u8),
                beneficiary: Address::ZERO,
                extra: vec![].into(),
            }],
            received: vec![AssetTuple {
                assetType: U256::from(1u8),
                contractAddress: Address::from_str("0x00000000000000000000000000000000000000dd")
                    .unwrap(),
                value: U256::from(10u8).pow(U256::from(18u8)),
                beneficiary: Address::from_str(signer).unwrap(),
                extra: vec![].into(),
            }],
        };
        let data = (trade,).abi_encode_params();
        let hashed = format!(
            "{:#x}",
            keccak256(hex::decode(signature.trim_start_matches("0x")).unwrap())
        );
        let mut topics = vec![
            (if v3 { TOPIC_TRADED_V3 } else { TOPIC_TRADED }).to_string(),
            topic_of(caller),
            hashed,
        ];
        if v3 {
            topics.push(format!("{:#x}", B256::repeat_byte(0x11)));
        }
        RpcLog {
            address: market.to_string(),
            topics,
            data: format!("0x{}", hex::encode(data)),
            block_number: format!("0x{block:x}"),
            log_index: format!("0x{index:x}"),
            block_timestamp: Some(format!("0x{:x}", 1_700_000_000 + block)),
            block_hash: None,
            transaction_hash: Some(format!("0x{:064x}", block * 1000 + index)),
        }
    }

    #[test]
    fn a_traded_log_decodes_its_signature_signer_and_assets_in_both_versions() {
        let market = marketplaces(ChainNet::Matic)[1].0;
        let signer = "0x00000000000000000000000000000000000000a1";
        let sig = format!("0x{}1b", "ab".repeat(64));
        for v3 in [false, true] {
            let log = traded_log(
                100,
                2,
                market,
                "0x00000000000000000000000000000000000000b2",
                signer,
                &sig,
                3,
                v3,
            );
            let ev = decode_log(&log, 55).unwrap().unwrap();
            assert_eq!(ev.kind, KIND_TRADED);
            assert_eq!(
                ev.hashed_signature,
                Some(listings::hashed_signature(&sig)),
                "the topic is the stored hashed_signature"
            );
            assert_eq!(ev.signer.as_deref(), Some(signer));
            assert_eq!(ev.caller, "0x00000000000000000000000000000000000000b2");
            let assets = ev.assets.unwrap();
            assert_eq!(assets["sent"][0]["assetType"], "3");
            assert_eq!(assets["sent"][0]["value"], "7");
            assert_eq!(assets["received"][0]["value"], "1000000000000000000");
            assert_eq!(assets["uses"], "3");
            assert_eq!((ev.block_number, ev.log_index, ev.block_time), (100, 2, 55));
        }
    }

    #[test]
    fn index_and_cancel_logs_decode_and_an_unknown_topic_is_skipped() {
        let market = marketplaces(ChainNet::Ethereum)[0].0;
        let mk = |topics: Vec<String>| RpcLog {
            address: market.to_string(),
            topics,
            data: "0x".into(),
            block_number: "0x10".into(),
            log_index: "0x1".into(),
            block_timestamp: None,
            block_hash: None,
            transaction_hash: None,
        };
        let signer = topic_of("0x00000000000000000000000000000000000000a1");
        let ev = decode_log(
            &mk(vec![
                TOPIC_SIGNER_INDEX_INCREASED.into(),
                signer.clone(),
                format!("0x{:064x}", 5),
            ]),
            1,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (ev.kind, ev.new_index.as_deref(), ev.caller.as_str()),
            (
                KIND_SIGNER_INDEX,
                Some("5"),
                "0x00000000000000000000000000000000000000a1"
            )
        );
        let ev = decode_log(
            &mk(vec![
                TOPIC_SIGNATURE_CANCELLED.into(),
                signer.clone(),
                format!("0x{}", "cd".repeat(32)),
            ]),
            1,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            ev.hashed_signature.as_deref(),
            Some(format!("0x{}", "cd".repeat(32)).as_str())
        );
        assert!(
            decode_log(&mk(vec![format!("0x{}", "00".repeat(32))]), 1)
                .unwrap()
                .is_none()
        );
        assert!(
            decode_log(&mk(vec![TOPIC_SIGNATURE_CANCELLED.into(), signer]), 1).is_err(),
            "a truncated log is an error, not a silent skip"
        );
    }

    #[test]
    fn ether_amounts_format_like_ethers() {
        assert_eq!(format_ether("1000000000000000000"), "1.0");
        assert_eq!(format_ether("1500000000000000000"), "1.5");
        assert_eq!(format_ether("200000000000000000000"), "200.0");
        assert_eq!(format_ether("1"), "0.000000000000000001");
        assert_eq!(format_ether(""), "0.0");
        for input in ["", "  ", "000000", "bad", "-1", "1.0", "é"] {
            assert_eq!(format_ether(input), "0.0", "{input:?}");
        }
        assert_eq!(format_ether("  001000000000000000000 "), "1.0");
        assert_eq!(format_ether("1000000000000000001"), "1.000000000000000001");
        let oversized = "9".repeat(100);
        assert_eq!(
            format_ether(&oversized),
            format!("{}.{}", "9".repeat(82), "9".repeat(18))
        );
    }

    #[test]
    fn the_readiness_sql_guards_every_verdict() {
        let sql = closed_on_chain_sql("t", "$1");
        assert!(
            sql.contains("tcs.trade_id = t.id")
                && sql.contains("tcc.folded_block IS NOT NULL")
                && !sql.contains("tcc.folded_block = tcc.last_block")
                && sql.contains("$1 - interval '1800 seconds'")
                && sql.contains("$1 + interval '600 seconds'"),
            "{sql}"
        );
        assert!(chain_status_sql("t", "$2").contains("tcs.status <> 'open'"));
    }
}
