// from: catalyrst/crates/catalyrst-economy/src/ports/reconcile.rs (the due-set pass over unsettled rows), marketplace@f3956826:webapp/src/modules/transaction (a hash polled to its receipt), SHOP.md "Preserve settlement and recovery" (indexed chain progress before a settlement is final; unknown transactions shown as pending)
//! The `executions-track` job: every open execution row is taken from its
//! receipt to a verdict, and nothing is held in memory between steps, so a
//! restart, a reconnect or a second node resumes exactly where the rows
//! are. One step per due row:
//!
//! 1. no receipt yet: `submitted` stays, rescheduled with backoff; after
//!    [`DROP_AFTER`] without a receipt the row is `dropped` (the wallet may
//!    resend; a later receipt for the same hash reopens nothing);
//! 2. a receipt with status 0: `reverted`;
//! 3. a receipt with status 1: what its logs prove is recorded
//!    (`settlement`: the venue's own event, the ERC-721 that moved and to
//!    whom, every MANA transfer out of the buyer) and the row is `mined`;
//! 4. a mined row is final once the chain index of its kind has folded the
//!    settlement past the receipt's block while published (the
//!    trade-events index for a trade, the nft index's order fold for a
//!    legacy order, the rentals index for a rent or a claim, the nft index
//!    for a NAME or a transfer), or, on a chain without that index, once
//!    the confirmation depth has passed; the ownership and the proceeds are
//!    then read again from the chain and compared with the expectation:
//!    `settled` when they agree, `mismatch` when they do not.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Address, B256, U256};
use chrono::{Duration as ChronoDuration, NaiveDateTime, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::chain::{Reader, Receipt};
use super::executions::{Advance, ExecutionRow, advance, due};
use super::venues::Chain;
use super::{SharedState, State};
use crate::evm::signer::ReceiptLog;
use crate::jobs::{JobContext, Jobs, PeriodicCfg};
use crate::market::chain_walk::{self, ChainNet, INDEXER_NFT, INDEXER_TRADE_EVENTS};
use crate::market::nft_index::TOPIC_TRANSFER;
use crate::market::trade_events::{TOPIC_SIGNATURE_CANCELLED, TOPIC_TRADED, TOPIC_TRADED_V3};
use crate::rentals::index::{
    INDEXER_RENTALS, TOPIC_ASSET_CLAIMED, TOPIC_ASSET_INDEX_UPDATED, TOPIC_ASSET_RENTED,
};

/// Rows the pass takes at most.
pub const DUE_BATCH: i64 = 50;
/// A submitted hash without a receipt for this long is dropped.
pub const DROP_AFTER: ChronoDuration = ChronoDuration::hours(24);
/// `OrderSuccessful` and `OrderCancelled` of the legacy order book.
pub const TOPIC_ORDER_SUCCESSFUL: &str = crate::market::nft_index::TOPIC_ORDER_SUCCESSFUL;
pub const TOPIC_ORDER_CANCELLED: &str = crate::market::nft_index::TOPIC_ORDER_CANCELLED;
/// `NameRegistered` of the DCLRegistrar.
pub const TOPIC_NAME_REGISTERED: &str = crate::market::nft_index::TOPIC_NAME_REGISTERED;

/// The next check: 5 s, 10 s, 20 s ... capped at a minute; the same
/// schedule for an unmined hash and a settlement waiting for its index.
pub fn backoff(attempts: i32) -> Duration {
    Duration::from_secs((5u64 << attempts.clamp(0, 4) as u64).min(60))
}

/// The address an indexed `address` topic carries (its low 20 bytes).
fn topic_address(topic: &B256) -> Address {
    Address::from_slice(&topic[12..])
}

/// A topic as `0x`-prefixed lowercase hex, the form the indexes' constants take.
fn topic_hex(topic: &B256) -> String {
    format!("{topic:#x}")
}

/// One 32-byte ABI word as an integer; `None` when the data is not one word.
fn word_u256(word: &[u8]) -> Option<U256> {
    (word.len() == 32).then(|| U256::from_be_slice(word))
}

/// What a receipt's logs prove about a settlement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Proof {
    /// The venue emitted its own settlement event for the reference.
    pub venue_event: Option<String>,
    /// The ERC-721 transfers of the expected token: `(from, to)`.
    pub token_moves: Vec<(Address, Address)>,
    /// MANA moved out of `payer`, by recipient.
    pub proceeds: BTreeMap<Address, U256>,
    pub mana_out: U256,
}

/// Reads the receipt's logs for `row`'s expectation.
pub fn proof_of(row: &ExecutionRow, receipt: &Receipt, payer: Address) -> Proof {
    let mut proof = Proof::default();
    let venue: Option<Address> = row.venue.parse().ok();
    let contract: Option<Address> = row.contract_address.as_deref().and_then(|c| c.parse().ok());
    let token_id: Option<U256> = row
        .token_id
        .as_deref()
        .and_then(|t| U256::from_str_radix(t, 10).ok());
    let mana: Option<Address> = row.mana_token.as_deref().and_then(|m| m.parse().ok());
    let reference = row.reference.as_deref().map(|r| r.to_ascii_lowercase());
    for log in &receipt.logs {
        let emitter = Some(log.address);
        let topic0 = log.topics.first().map(topic_hex).unwrap_or_default();
        let names_reference = |t: &B256| reference.as_deref() == Some(topic_hex(t).as_str());
        if emitter == venue {
            let event = match topic0.as_str() {
                TOPIC_TRADED | TOPIC_TRADED_V3 => log
                    .topics
                    .get(2)
                    .filter(|t| names_reference(t))
                    .map(|_| "Traded"),
                TOPIC_SIGNATURE_CANCELLED => log
                    .topics
                    .get(2)
                    .filter(|t| names_reference(t))
                    .map(|_| "SignatureCancelled"),
                TOPIC_ORDER_SUCCESSFUL => Some("OrderSuccessful"),
                TOPIC_ORDER_CANCELLED => Some("OrderCancelled"),
                TOPIC_ASSET_RENTED => Some("AssetRented"),
                TOPIC_ASSET_CLAIMED => Some("AssetClaimed"),
                TOPIC_ASSET_INDEX_UPDATED => Some("AssetIndexUpdated"),
                TOPIC_NAME_REGISTERED => Some("NameRegistered"),
                _ => None,
            };
            if let Some(event) = event {
                proof.venue_event.get_or_insert_with(|| event.to_string());
            }
        }
        if emitter == contract
            && topic0 == TOPIC_TRANSFER
            && log.topics.len() == 4
            && token_id == Some(U256::from_be_bytes(log.topics[3].0))
        {
            proof
                .token_moves
                .push((topic_address(&log.topics[1]), topic_address(&log.topics[2])));
        }
        if emitter == mana
            && topic0 == TOPIC_TRANSFER
            && log.topics.len() == 3
            && topic_address(&log.topics[1]) == payer
        {
            let amount = word_u256(&log.data).unwrap_or(U256::ZERO);
            *proof
                .proceeds
                .entry(topic_address(&log.topics[2]))
                .or_insert(U256::ZERO) += amount;
            proof.mana_out += amount;
        }
    }
    proof
}

fn proof_json(proof: &Proof) -> Value {
    json!({
        "venueEvent": proof.venue_event,
        "tokenMoves": proof.token_moves.iter().map(|(f, t)| json!({ "from": format!("{f:#x}"), "to": format!("{t:#x}") })).collect::<Vec<_>>(),
        "proceeds": proof.proceeds.iter().map(|(to, amount)| json!({ "to": format!("{to:#x}"), "amount": amount.to_string() })).collect::<Vec<_>>(),
        "manaOut": proof.mana_out.to_string(),
    })
}

/// Whether the chain index of `row`'s kind has folded the settlement past
/// `block` while published; `None` when the chain has no such index.
pub async fn index_confirms(
    pool: &PgPool,
    chain: &Chain,
    row: &ExecutionRow,
    block: i64,
) -> Result<Option<bool>, sqlx::Error> {
    let Some(net) = chain.net else {
        return Ok(None);
    };
    let now = Utc::now();
    let ready = |indexer: &'static str| async move {
        chain_walk::ready_cursor(pool, indexer, net, now)
            .await
            .filter(|c| c.folded_block.is_some_and(|f| f >= block))
    };
    let confirmed = match row.kind.as_str() {
        "accept_trade" | "cancel_trade" => {
            let Some(_) = ready(INDEXER_TRADE_EVENTS).await else {
                return Ok(Some(false));
            };
            let kind = if row.kind == "accept_trade" {
                "traded"
            } else {
                "signature_cancelled"
            };
            sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM shop.trade_event WHERE network = $1 AND tx_hash = $2 AND kind = $3 AND hashed_signature = $4)")
                .bind(net.db_name())
                .bind(&row.tx_hash)
                .bind(kind)
                .bind(row.reference.as_deref().unwrap_or_default())
                .fetch_one(pool)
                .await?
        }
        "execute_order" | "cancel_order" => {
            let Some(_) = ready(INDEXER_NFT).await else {
                return Ok(Some(false));
            };
            let wanted = if row.kind == "execute_order" {
                "sold"
            } else {
                "cancelled"
            };
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM shop.nft_order WHERE network = $1 AND contract_address = $2 AND token_id = $3::numeric AND status = $4 AND updated_block >= $5)",
            )
            .bind(net.db_name())
            .bind(row.contract_address.as_deref().unwrap_or_default())
            .bind(row.token_id.as_deref().unwrap_or("0"))
            .bind(wanted)
            .bind(block)
            .fetch_one(pool)
            .await?
        }
        "accept_rental" | "claim_rental" | "cancel_rental" => {
            let Some(_) = ready(INDEXER_RENTALS).await else {
                return Ok(Some(false));
            };
            let kind = match row.kind.as_str() {
                "accept_rental" => "rented",
                "claim_rental" => "claimed",
                _ => "asset_index_updated",
            };
            sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM shop.rental_event WHERE network = $1 AND tx_hash = $2 AND kind = $3)")
                .bind(net.db_name())
                .bind(&row.tx_hash)
                .bind(kind)
                .fetch_one(pool)
                .await?
        }
        "register_name" | "transfer" => {
            let Some(_) = ready(INDEXER_NFT).await else {
                return Ok(Some(false));
            };
            let land = chain.is_land_registry(
                row.contract_address
                    .as_deref()
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(Address::ZERO),
            ) || chain.is_estate_registry(
                row.contract_address
                    .as_deref()
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(Address::ZERO),
            );
            if land {
                if !crate::market::nft_reads::land_ready(pool).await {
                    return Ok(Some(false));
                }
                sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM land_authz.token_right WHERE token_address = $1 AND token_id = $2::numeric AND owner = $3 AND updated_block >= $4)")
                    .bind(row.contract_address.as_deref().unwrap_or_default())
                    .bind(row.token_id.as_deref().unwrap_or("0"))
                    .bind(row.expected_owner.as_deref().unwrap_or_default())
                    .bind(block)
                    .fetch_one(pool)
                    .await?
            } else {
                sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM shop.nft WHERE network = $1 AND contract_address = $2 AND token_id = $3::numeric AND owner = $4 AND transferred_block >= $5)")
                    .bind(net.db_name())
                    .bind(row.contract_address.as_deref().unwrap_or_default())
                    .bind(row.token_id.as_deref().unwrap_or("0"))
                    .bind(row.expected_owner.as_deref().unwrap_or_default())
                    .bind(block)
                    .fetch_one(pool)
                    .await?
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(confirmed))
}

/// The final comparison: the token's owner now and the MANA the payer
/// spent against the expectation; the mismatches, empty when it settled.
pub async fn verdict(reader: &Reader, row: &ExecutionRow, proof: &Proof) -> Vec<String> {
    let mut mismatches = Vec::new();
    let contract: Option<Address> = row.contract_address.as_deref().and_then(|c| c.parse().ok());
    let token_id: Option<U256> = row
        .token_id
        .as_deref()
        .and_then(|t| U256::from_str_radix(t, 10).ok());
    let expected_owner: Option<Address> =
        row.expected_owner.as_deref().and_then(|o| o.parse().ok());
    if let (Some(contract), Some(token_id), Some(expected)) = (contract, token_id, expected_owner)
        && row.kind != "cancel_rental"
    {
        match reader.owner_of(contract, token_id).await {
            Ok(Some(owner)) if owner == expected => {}
            Ok(Some(owner)) => mismatches.push(format!(
                "the token is held by {owner:#x}, not {expected:#x}"
            )),
            Ok(None) => mismatches.push("the token does not exist".to_string()),
            Err(e) => mismatches.push(format!("ownership could not be read: {e}")),
        }
    }
    if let Some(price) = row
        .expected_price
        .as_deref()
        .and_then(|p| U256::from_str_radix(p, 10).ok())
    {
        if proof.mana_out != price {
            mismatches.push(format!(
                "{} wei left the payer, the price was {price}",
                proof.mana_out
            ));
        }
        if let Some(seller) = row
            .expected_seller
            .as_deref()
            .and_then(|s| s.parse::<Address>().ok())
        {
            match proof.proceeds.get(&seller) {
                Some(got) if !got.is_zero() => {}
                _ => mismatches.push(format!("the seller {seller:#x} received nothing")),
            }
        }
    }
    if matches!(
        row.kind.as_str(),
        "accept_trade"
            | "execute_order"
            | "accept_rental"
            | "register_name"
            | "cancel_trade"
            | "cancel_order"
            | "claim_rental"
            | "cancel_rental"
    ) && proof.venue_event.is_none()
    {
        mismatches.push("the venue emitted no settlement event for this reference".to_string());
    }
    if row.kind == "transfer"
        && proof.token_moves.last().map(|(_, to)| Some(*to)) != Some(expected_owner)
    {
        mismatches.push("the token did not move to the recipient in this transaction".to_string());
    }
    mismatches
}

/// One row's step; returns the status it moved to (or stayed at).
pub async fn step(
    state: &State,
    row: &ExecutionRow,
    now: NaiveDateTime,
) -> anyhow::Result<&'static str> {
    let chain_id = crate::types::ChainId(row.chain_id.max(0) as u64);
    let stay = |error: Option<String>, status: &'static str| Advance {
        status,
        block_number: None,
        confirmed_by: None,
        settlement: None,
        error,
        next_check_at: now
            + ChronoDuration::from_std(backoff(row.attempts)).unwrap_or(ChronoDuration::minutes(1)),
    };
    let Some(chain) = state.venues.chain(chain_id).cloned() else {
        advance(
            &state.pool,
            row.id,
            &row.status,
            &stay(
                Some("no venue book for the chain".into()),
                if row.status == "mined" {
                    "mined"
                } else {
                    "submitted"
                },
            ),
            now,
        )
        .await?;
        return Ok("unknown-chain");
    };
    let Some(reader) = state.reader(chain_id) else {
        advance(
            &state.pool,
            row.id,
            &row.status,
            &stay(
                Some("no JSON-RPC endpoint for the chain".into()),
                if row.status == "mined" {
                    "mined"
                } else {
                    "submitted"
                },
            ),
            now,
        )
        .await?;
        return Ok("no-rpc");
    };
    let receipt = match reader.receipt(&row.tx_hash).await {
        Ok(r) => r,
        Err(e) => {
            advance(
                &state.pool,
                row.id,
                &row.status,
                &stay(
                    Some(e.to_string()),
                    if row.status == "mined" {
                        "mined"
                    } else {
                        "submitted"
                    },
                ),
                now,
            )
            .await?;
            return Ok("rpc-error");
        }
    };
    let Some(receipt) = receipt else {
        let dropped = now.signed_duration_since(row.created_at) >= DROP_AFTER;
        let status = if dropped { "dropped" } else { "submitted" };
        advance(
            &state.pool,
            row.id,
            &row.status,
            &Advance {
                error: dropped.then(|| "no receipt within 24 h".to_string()),
                ..stay(None, status)
            },
            now,
        )
        .await?;
        return Ok(status);
    };
    if receipt.status == Some(false) {
        advance(
            &state.pool,
            row.id,
            &row.status,
            &Advance {
                status: "reverted",
                block_number: Some(receipt.block_number as i64),
                confirmed_by: Some("receipt"),
                settlement: Some(json!({ "reverted": true })),
                error: None,
                next_check_at: now,
            },
            now,
        )
        .await?;
        return Ok("reverted");
    }
    let payer: Address = row.actor.parse().unwrap_or(receipt.from);
    let proof = proof_of(row, &receipt, payer);
    let block = receipt.block_number as i64;
    let (confirmed, by) = match index_confirms(&state.pool, &chain, row, block).await? {
        Some(true) => (true, "index"),
        Some(false) => (false, "index"),
        None => match reader.head_block().await {
            Ok(head) => (
                head.saturating_sub(receipt.block_number) >= chain.confirmations,
                "finality",
            ),
            Err(_) => (false, "finality"),
        },
    };
    if !confirmed {
        // On a chain with the index, the confirmation depth alone also
        // finalizes: the index may lag its own publish window while the
        // chain is long past the block.
        let finality = match reader.head_block().await {
            Ok(head) if by == "index" => {
                head.saturating_sub(receipt.block_number)
                    >= chain.confirmations + ChainNet::Ethereum.publish_lag() * 4
            }
            _ => false,
        };
        if !finality {
            advance(
                &state.pool,
                row.id,
                &row.status,
                &Advance {
                    status: "mined",
                    block_number: Some(block),
                    confirmed_by: None,
                    settlement: Some(proof_json(&proof)),
                    error: None,
                    next_check_at: now
                        + ChronoDuration::from_std(backoff(row.attempts))
                            .unwrap_or(ChronoDuration::minutes(1)),
                },
                now,
            )
            .await?;
            return Ok("mined");
        }
    }
    let mismatches = verdict(&reader, row, &proof).await;
    let status = if mismatches.is_empty() {
        "settled"
    } else {
        "mismatch"
    };
    let mut settlement = proof_json(&proof);
    settlement["mismatches"] = json!(mismatches);
    settlement["confirmedBy"] = json!(by);
    advance(
        &state.pool,
        row.id,
        &row.status,
        &Advance {
            status,
            block_number: Some(block),
            confirmed_by: Some(if confirmed { by } else { "finality" }),
            settlement: Some(settlement),
            error: None,
            next_check_at: now,
        },
        now,
    )
    .await?;
    Ok(status)
}

/// One pass over the due rows.
pub async fn run_pass(state: &State, ctx: &JobContext) -> anyhow::Result<()> {
    let now = Utc::now().naive_utc();
    let rows = due(&state.pool, now, DUE_BATCH).await?;
    let mut moved = BTreeMap::new();
    for row in &rows {
        if ctx.is_cancelled() {
            break;
        }
        let outcome = step(state, row, now).await?;
        *moved.entry(outcome).or_insert(0usize) += 1;
    }
    if !rows.is_empty() {
        ctx.progress(format!("executions: {} due, {:?}", rows.len(), moved));
    }
    Ok(())
}

/// Attaches `executions-track` when at least one chain of the book has a
/// JSON-RPC endpoint to read receipts from.
pub fn spawn(state: &Arc<State>, jobs: &Jobs) {
    if !state.venues.chains().any(|c| c.rpc_url.is_some()) {
        tracing::info!(
            "executions tracker not started: no chain has a JSON-RPC endpoint (TRADE_RPC_URLS / ETH_RPC_URL)"
        );
        return;
    }
    let state = state.clone();
    jobs.spawn_periodic_leased(
        "executions-track",
        PeriodicCfg::every(state.settings.track_interval)
            .with_jitter(Duration::from_secs(2))
            .with_error_backoff(Duration::from_secs(300)),
        move |ctx| {
            let state = state.clone();
            async move { run_pass(&state, &ctx).await }
        },
    );
}

/// The topic constants this module matches receipts with (the tests pin them).
pub fn topics() -> [(&'static str, &'static str); 8] {
    [
        ("Traded", TOPIC_TRADED),
        ("TradedV3", TOPIC_TRADED_V3),
        ("SignatureCancelled", TOPIC_SIGNATURE_CANCELLED),
        ("OrderSuccessful", TOPIC_ORDER_SUCCESSFUL),
        ("OrderCancelled", TOPIC_ORDER_CANCELLED),
        ("AssetRented", TOPIC_ASSET_RENTED),
        ("AssetClaimed", TOPIC_ASSET_CLAIMED),
        ("Transfer", TOPIC_TRANSFER),
    ]
}

/// A receipt log as the tests and the anvil harness build it.
pub fn log(address: Address, topics: Vec<String>, data: Vec<u8>) -> ReceiptLog {
    ReceiptLog {
        address,
        topics: topics
            .iter()
            .map(|t| t.parse().expect("a 32-byte topic"))
            .collect(),
        data: data.into(),
    }
}

pub fn topic_of(a: Address) -> String {
    format!("0x{:0>64}", hex::encode(a.as_slice()))
}

pub fn topic_word(v: U256) -> String {
    format!("{:#x}", B256::from(v.to_be_bytes::<32>()))
}

pub type Shared = SharedState;

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: &str, price: Option<&str>) -> ExecutionRow {
        let now = Utc::now().naive_utc();
        ExecutionRow {
            id: uuid::Uuid::new_v4(),
            chain_id: 137,
            tx_hash: format!("0x{}", "11".repeat(32)),
            kind: kind.into(),
            actor: format!("{:#x}", Address::repeat_byte(0xb0)),
            venue: format!("{:#x}", Address::repeat_byte(0xee)),
            contract_address: Some(format!("{:#x}", Address::repeat_byte(0xcc))),
            token_id: Some("7".into()),
            reference: Some(format!("0x{}", "ab".repeat(32))),
            expected_price: price.map(String::from),
            mana_token: Some(format!("{:#x}", Address::repeat_byte(0xaa))),
            expected_seller: Some(format!("{:#x}", Address::repeat_byte(0x5e))),
            expected_owner: Some(format!("{:#x}", Address::repeat_byte(0xb0))),
            status: "submitted".into(),
            block_number: None,
            confirmed_by: None,
            settlement: None,
            attempts: 0,
            last_error: None,
            next_check_at: now,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn the_backoff_climbs_to_a_minute() {
        assert_eq!(backoff(0), Duration::from_secs(5));
        assert_eq!(backoff(1), Duration::from_secs(10));
        assert_eq!(backoff(3), Duration::from_secs(40));
        assert_eq!(backoff(9), Duration::from_secs(60));
    }

    #[test]
    fn a_receipt_proves_the_event_the_token_move_and_the_proceeds() {
        let r = row("accept_trade", Some("1000"));
        let buyer = Address::repeat_byte(0xb0);
        let seller = Address::repeat_byte(0x5e);
        let fee = Address::repeat_byte(0xfe);
        let venue = Address::repeat_byte(0xee);
        let receipt = Receipt {
            tx_hash: r.tx_hash.clone(),
            status: Some(true),
            block_number: 100,
            from: buyer,
            to: Some(venue),
            gas_used: None,
            logs: vec![
                log(
                    Address::repeat_byte(0xaa),
                    vec![TOPIC_TRANSFER.into(), topic_of(buyer), topic_of(seller)],
                    U256::from(975u32).to_be_bytes_vec(),
                ),
                log(
                    Address::repeat_byte(0xaa),
                    vec![TOPIC_TRANSFER.into(), topic_of(buyer), topic_of(fee)],
                    U256::from(25u32).to_be_bytes_vec(),
                ),
                log(
                    Address::repeat_byte(0xcc),
                    vec![
                        TOPIC_TRANSFER.into(),
                        topic_of(seller),
                        topic_of(buyer),
                        topic_word(U256::from(7u8)),
                    ],
                    vec![],
                ),
                log(
                    venue,
                    vec![
                        TOPIC_TRADED.into(),
                        topic_of(buyer),
                        format!("0x{}", "ab".repeat(32)),
                    ],
                    vec![],
                ),
            ],
        };
        let proof = proof_of(&r, &receipt, buyer);
        assert_eq!(proof.venue_event.as_deref(), Some("Traded"));
        assert_eq!(proof.token_moves, vec![(seller, buyer)]);
        assert_eq!(proof.mana_out, U256::from(1000u32));
        assert_eq!(proof.proceeds.get(&seller), Some(&U256::from(975u32)));
        assert_eq!(proof.proceeds.get(&fee), Some(&U256::from(25u32)));
        let other_ref = ExecutionRow {
            reference: Some(format!("0x{}", "cd".repeat(32))),
            ..r.clone()
        };
        assert_eq!(
            proof_of(&other_ref, &receipt, buyer).venue_event,
            None,
            "a Traded for another signature proves nothing"
        );
        let json = proof_json(&proof);
        assert_eq!(json["manaOut"], "1000");
        assert_eq!(json["proceeds"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn the_topics_are_the_indexes_constants() {
        for (name, topic) in topics() {
            assert!(topic.starts_with("0x") && topic.len() == 66, "{name}");
        }
        assert_eq!(
            TOPIC_ORDER_SUCCESSFUL,
            "0x695ec315e8a642a74d450a4505eeea53df699b47a7378c7d752e97d5b16eb9bb"
        );
    }
}
