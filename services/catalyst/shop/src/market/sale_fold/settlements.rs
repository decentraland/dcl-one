//! A receipt contains the whole transaction. Split it at venue executions
//! before matching payments, and require every leg to conserve base units.
use std::collections::{BTreeMap, BTreeSet};

use std::sync::LazyLock;

use alloy_primitives::U256;
use alloy_sol_types::{SolType, sol_data};
use anyhow::{Context, ensure};
use serde_json::json;
use sqlx::{FromRow, PgConnection};

use super::{ChainNet, Rpc, chain_walk, ethereum_sales, nft_index, sales_index, trade_events};
use crate::chain_index::rpc::{RpcLog, hex_to_i64};

#[derive(FromRow)]
struct Execution {
    block: i64,
    index: i32,
    tx_hash: String,
    venue: String,
    kind: String,
}

#[derive(FromRow, Debug)]
pub(super) struct Payment {
    block: i64,
    index: i32,
    venue: String,
    kind: String,
    ordinal: i32,
    currency: String,
    gross: String,
    payer: Option<String>,
    beneficiary: Option<String>,
    bid_fee: Option<String>,
    collector: Option<String>,
    rate: Option<i32>,
    royalty_rate: Option<i32>,
    extra: Option<String>,
}

#[derive(Clone, Debug)]
struct Transfer {
    currency: String,
    from: String,
    to: String,
    amount: U256,
}

fn transfer(log: &RpcLog) -> Option<Transfer> {
    if log.topics.len() != 3 || !log.topics[0].eq_ignore_ascii_case(nft_index::TOPIC_TRANSFER) {
        return None;
    }
    let address = |v: &str| {
        let word = v.strip_prefix("0x")?;
        (word.len() == 64
            && word.starts_with("000000000000000000000000")
            && word.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("0x{}", word[24..].to_ascii_lowercase()))
    };
    let word = log.data.strip_prefix("0x")?;
    if word.len() != 64 {
        return None;
    }
    Some(Transfer {
        currency: log.address.to_ascii_lowercase(),
        from: address(&log.topics[1])?,
        to: address(&log.topics[2])?,
        amount: U256::from_str_radix(word, 16).ok()?,
    })
}

#[derive(Debug)]
struct Leg {
    role: &'static str,
    recipient: String,
    amount: U256,
}

fn leg(role: &'static str, recipient: &str, amount: U256) -> Leg {
    Leg {
        role,
        recipient: recipient.into(),
        amount,
    }
}

fn cut(gross: U256, ppm: i32) -> Option<U256> {
    let ppm = u32::try_from(ppm).ok()?;
    if ppm > 1_000_000 {
        return None;
    }
    // Divide before multiplying the quotient; the bounded remainder also fits.
    let unit = U256::from(1_000_000);
    let rate = U256::from(ppm);
    Some((gross / unit) * rate + ((gross % unit) * rate) / unit)
}

fn legacy(net: ChainNet, venue: &str) -> bool {
    venue == nft_index::legacy_marketplaces(net)[0].0
        || (net == ChainNet::Ethereum && venue == ethereum_sales::ERC721_BID)
}

fn legacy_payout(net: ChainNet, p: &Payment, moves: &[&Transfer], gross: U256) -> Option<Vec<Leg>> {
    let (net_move, fees) = moves.split_last()?;
    if net_move.to != *p.beneficiary.as_ref()? || fees.len() > 2 {
        return None;
    }
    let fee = gross.checked_sub(net_move.amount)?;
    if let Some(bid_fee) = &p.bid_fee
        && U256::from_str_radix(bid_fee, 10).ok()? != fee
    {
        return None;
    }
    let mut out = vec![leg("seller", &net_move.to, net_move.amount)];
    if legacy(net, &p.venue) {
        if fees.len() > 1 || p.rate.is_some_and(|rate| cut(gross, rate) != Some(fee)) {
            return None;
        }
        if let Some(f) = fees.first() {
            if p.collector.as_ref().is_some_and(|c| c != &f.to) {
                return None;
            }
            out.push(leg("venue_fee", &f.to, f.amount));
        }
    } else {
        let royalty = cut(gross, p.royalty_rate?)?;
        let dao = cut(gross, p.rate?)?;
        let collector = p.collector.as_ref()?;
        let mut it = fees.iter();
        if let Some(f) = it.next() {
            if fees.len() == 2 || f.to != *collector {
                if f.amount != royalty {
                    return None;
                }
                out.push(leg("royalty", &f.to, f.amount));
                if let Some(d) = it.next() {
                    if d.to != *collector || d.amount != dao {
                        return None;
                    }
                    out.push(leg("venue_fee", &d.to, d.amount));
                } else if dao != U256::ZERO {
                    return None;
                }
            } else {
                // A missing royalties receiver redirects its cut to the fee collector.
                if f.amount != dao.checked_add(royalty)? {
                    return None;
                }
                out.push(leg("venue_fee", &f.to, f.amount));
            }
        } else if fee != U256::ZERO {
            return None;
        }
    }
    Some(out)
}

fn trade_payout(net: ChainNet, p: &Payment, gross: U256) -> Option<Vec<Leg>> {
    let payee = p.beneficiary.as_ref()?;
    let payer = p.payer.as_ref()?;
    let (pay_collector, royalties) = if net == ChainNet::Matic {
        type Split = (
            sol_data::Bool,
            sol_data::Uint<256>,
            sol_data::Array<sol_data::Address>,
        );
        let data = hex::decode(p.extra.as_ref()?.strip_prefix("0x")?).ok()?;
        let (pay, n, recipients) = Split::abi_decode_params(&data).ok()?;
        let n = usize::try_from(n).ok()?;
        let recipients = recipients
            .get(..n)?
            .iter()
            .map(|a| format!("{a:#x}"))
            .collect::<Vec<_>>();
        (pay || n == 0, recipients)
    } else {
        (true, Vec::new())
    };
    let mut out = Vec::new();
    let fee = if pay_collector {
        let fee = cut(gross, p.rate?)?;
        out.push(leg("venue_fee", p.collector.as_ref()?, fee));
        fee
    } else {
        U256::ZERO
    };
    let royalty = if royalties.is_empty() {
        U256::ZERO
    } else {
        cut(gross, p.royalty_rate?)?
    };
    let mut remainder = U256::ZERO;
    if !royalties.is_empty() {
        let n = U256::from(royalties.len());
        for to in royalties {
            out.push(leg("royalty", &to, royalty / n));
        }
        remainder = royalty % n;
    }
    let net_amount = gross.checked_sub(fee)?.checked_sub(royalty)?;
    if net == ChainNet::Ethereum {
        out.insert(0, leg("seller", payee, net_amount))
    } else {
        out.push(leg("seller", payee, net_amount))
    }
    if remainder != U256::ZERO {
        out.push(leg("retained", payer, remainder))
    }
    Some(out)
}

struct Split {
    payer: Option<String>,
    payee: Option<String>,
    legs: Vec<Leg>,
}

fn store(rows: &[Payment], moves: &[Transfer]) -> Option<Vec<Split>> {
    let first = rows.first()?;
    let mut observed = moves.iter().filter(|m| m.currency == first.currency);
    let mut total_fee = U256::ZERO;
    let mut lines = Vec::new();
    let mut payer = None;
    for p in rows {
        let gross = U256::from_str_radix(&p.gross, 10).ok()?;
        let fee = if gross == U256::ZERO {
            U256::ZERO
        } else {
            cut(gross, p.rate?)?
        };
        let payee = if gross != U256::ZERO {
            let t = observed.next()?;
            if t.amount != gross.checked_sub(fee)? || payer.is_some_and(|from| from != &t.from) {
                return None;
            }
            payer = Some(&t.from);
            Some(t.to.clone())
        } else {
            None
        };
        total_fee = total_fee.checked_add(fee)?;
        lines.push((payee, gross - fee, fee));
    }
    let fee_move = if total_fee != U256::ZERO {
        let t = observed.next()?;
        if t.amount != total_fee
            || Some(&t.from) != payer
            || first.collector.as_ref().is_some_and(|c| c != &t.to)
        {
            return None;
        }
        Some(&t.to)
    } else {
        None
    };
    if observed.next().is_some() {
        return None;
    }
    // A free-only store call doesn't identify a payer in its transfer logs.
    let payer = payer.cloned();
    Some(
        lines
            .into_iter()
            .map(|(payee, net, fee)| {
                let mut legs = Vec::new();
                if let Some(to) = &payee {
                    legs.push(leg("seller", to, net));
                }
                if let Some(to) = fee_move {
                    legs.push(leg("store_fee", to, fee));
                }
                Split {
                    payer: payer.clone(),
                    payee,
                    legs,
                }
            })
            .collect(),
    )
}

static BURNERS: LazyLock<[String; 2]> = LazyLock::new(|| {
    let addresses: serde_json::Value =
        serde_json::from_slice(crate::evm::contracts::ADDRESSES_JSON)
            .expect("bundled contract registry");
    ["MANABurner", "BidMANABurner"].map(|name| {
        addresses["mainnet"][name]
            .as_str()
            .expect("registered Ethereum MANA burner")
            .to_ascii_lowercase()
    })
});

fn classify(net: ChainNet, rows: &[Payment], moves: &[Transfer]) -> Option<Vec<Split>> {
    let first = rows.first()?;
    if first.kind == "store" {
        return store(rows, moves);
    }
    let mut out = Vec::new();
    let currencies: BTreeSet<_> = rows.iter().map(|p| p.currency.as_str()).collect();
    let mut movements = moves
        .iter()
        .filter(|m| {
            currencies.contains(m.currency.as_str())
                && rows.iter().any(|p| p.payer.as_ref() == Some(&m.from))
        })
        .peekable();
    for p in rows {
        let gross = U256::from_str_radix(&p.gross, 10).ok()?;
        let payer = p.payer.as_ref()?;
        let mut legs = match p.kind.as_str() {
            "trade" => {
                let legs = trade_payout(net, p, gross)?;
                for leg in legs.iter().filter(|l| l.role != "retained") {
                    let t = movements.next()?;
                    if t.currency != p.currency
                        || t.from != *payer
                        || t.to != leg.recipient
                        || t.amount != leg.amount
                    {
                        return None;
                    }
                }
                legs
            }
            "order" | "bid" => {
                let observed: Vec<_> = movements.by_ref().collect();
                legacy_payout(net, p, &observed, gross)?
            }
            "name_fee" => {
                let t = movements.next()?;
                if t.amount != gross || t.from != *payer || t.currency != p.currency {
                    return None;
                }
                vec![leg("name_fee", &t.to, gross)]
            }
            _ => return None,
        };
        if net == ChainNet::Ethereum {
            for leg in &mut legs {
                if (leg.role == "name_fee" && p.venue == ethereum_sales::BURNING_NAME_CONTROLLER)
                    || (matches!(leg.role, "venue_fee" | "name_fee")
                        && BURNERS.contains(&leg.recipient))
                {
                    leg.role = "burn";
                }
            }
        }
        if legs
            .iter()
            .try_fold(U256::ZERO, |v, l| v.checked_add(l.amount))?
            != gross
        {
            return None;
        }
        out.push(Split {
            payer: Some(payer.clone()),
            payee: p.beneficiary.clone(),
            legs,
        });
    }
    if movements.next().is_some() {
        return None;
    }
    Some(out)
}

fn venue_event(net: ChainNet, log: &RpcLog) -> Option<&'static str> {
    let address = log.address.to_ascii_lowercase();
    let topic = log.topics.first()?.to_ascii_lowercase();
    match topic.as_str() {
        sales_index::TOPIC_BOUGHT
            if net == ChainNet::Matic && address == sales_index::STORE_MATIC =>
        {
            Some("store")
        }
        nft_index::TOPIC_ORDER_SUCCESSFUL
            if nft_index::legacy_marketplaces(net)
                .iter()
                .any(|(a, _)| *a == address) =>
        {
            Some("order")
        }
        sales_index::TOPIC_BID_ACCEPTED
            if address
                == match net {
                    ChainNet::Ethereum => ethereum_sales::ERC721_BID,
                    ChainNet::Matic => sales_index::BID_V2_MATIC,
                } =>
        {
            Some("bid")
        }
        trade_events::TOPIC_TRADED | trade_events::TOPIC_TRADED_V3
            if trade_events::marketplaces(net)
                .iter()
                .any(|d| d.0 == address) =>
        {
            Some("trade")
        }
        ethereum_sales::TOPIC_NAME_BOUGHT
            if net == ChainNet::Ethereum
                && ethereum_sales::NAME_CONTROLLERS.contains(&address.as_str()) =>
        {
            Some("name_fee")
        }
        _ => None,
    }
}

// Optional Issue logs can come from any contract called during the fill.
fn traded_issue(
    l: &RpcLog,
    venue: &str,
    contracts: &BTreeSet<&str>,
) -> Option<(String, String, String, String)> {
    if l.topics.len() != 4 || !l.topics[0].eq_ignore_ascii_case(sales_index::TOPIC_ISSUE) {
        return None;
    }
    let contract = l.address.to_ascii_lowercase();
    if !contracts.contains(contract.as_str()) {
        return None;
    }
    let data = hex::decode(l.data.strip_prefix("0x")?).ok()?;
    let (_, caller) = <(sol_data::Uint<256>, sol_data::Address)>::abi_decode_params(&data).ok()?;
    if format!("{caller:#x}") != venue {
        return None;
    }
    Some((
        contract,
        chain_walk::topic_u256(&l.topics[3])?,
        chain_walk::topic_address(&l.topics[1])?,
        chain_walk::topic_u256(&l.topics[2])?,
    ))
}

/// A Traded collection-item leg supplies an item id. Its matching Issue in
/// this execution supplies the minted token id, including repeated copies.
async fn issued_tokens(
    c: &mut PgConnection,
    net: ChainNet,
    p: &Execution,
    logs: &[&RpcLog],
) -> anyhow::Result<()> {
    let assets: Vec<(i32, String, String, String)> = sqlx::query_as(
        "SELECT ordinal,contract_address,item_id::text,buyer FROM shop.sale_asset
         WHERE network=$1 AND block_number=$2 AND log_index=$3 AND kind='mint' ORDER BY ordinal",
    )
    .bind(net.db_name())
    .bind(p.block)
    .bind(p.index)
    .fetch_all(&mut *c)
    .await?;
    if assets.is_empty() {
        return Ok(());
    }
    let contracts = assets
        .iter()
        .map(|(_, contract, _, _)| contract.as_str())
        .collect();
    let issues: Vec<_> = logs
        .iter()
        .filter_map(|l| traded_issue(l, &p.venue, &contracts))
        .collect();
    if assets.len() != issues.len() {
        return Ok(());
    }
    for ((_, contract, item, buyer), (issuer, minted, recipient, _)) in assets.iter().zip(&issues) {
        if issuer != contract || minted != item || recipient != buyer {
            return Ok(());
        }
    }
    for ((ordinal, _, _, _), (_, _, _, token)) in assets.into_iter().zip(issues) {
        sqlx::query("UPDATE shop.sale_asset SET token_id=$5::text::numeric WHERE (network,block_number,log_index,ordinal)=($1,$2,$3,$4)")
            .bind(net.db_name()).bind(p.block).bind(p.index).bind(ordinal).bind(token).execute(&mut *c).await?;
    }
    Ok(())
}

async fn save(
    conn: &mut PgConnection,
    net: ChainNet,
    p: &Payment,
    split: Split,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE shop.sale_payment SET settled=true,payer=$5,beneficiary=$6 WHERE (network,block_number,log_index,ordinal)=($1,$2,$3,$4)")
        .bind(net.db_name()).bind(p.block).bind(p.index).bind(p.ordinal).bind(split.payer).bind(split.payee).execute(&mut *conn).await?;
    for (n, l) in split.legs.iter().enumerate() {
        sqlx::query("INSERT INTO shop.sale_payout (network,block_number,log_index,payment_ordinal,ordinal,role,recipient,amount) VALUES($1,$2,$3,$4,$5,$6,$7,$8::text::numeric)")
            .bind(net.db_name()).bind(p.block).bind(p.index).bind(p.ordinal).bind(n as i32).bind(l.role).bind(&l.recipient).bind(l.amount.to_string()).execute(&mut *conn).await?;
    }
    Ok(())
}

pub(super) async fn write(
    conn: &mut PgConnection,
    rpc: &Rpc<'_>,
    net: ChainNet,
    from: i64,
    to: i64,
) -> anyhow::Result<Vec<i64>> {
    let retry:Vec<String>=sqlx::query_scalar(
       "SELECT tx_hash FROM shop.sale_execution WHERE network=$1 AND block_number <= $2 AND NOT receipt_read
          AND (receipt_attempted_at IS NULL OR receipt_attempted_at<timezone('UTC',now()) - interval '5 minutes')
        ORDER BY receipt_attempted_at NULLS FIRST,block_number LIMIT 48")
       .bind(net.db_name()).bind(from).fetch_all(&mut *conn).await?;
    // An exchange of existing ERC-721s supplies every token ID in Traded.
    // Complete its receipt work in the inserting transaction, or on its
    // first eligible retry if an earlier version left the execution pending.
    sqlx::query(
        "UPDATE shop.sale_execution e SET receipt_read=true FROM shop.trade_event t
         WHERE e.network=$1 AND e.kind='trade' AND NOT e.receipt_read
           AND ((e.block_number>$2 AND e.block_number<=$3) OR e.tx_hash=ANY($4))
           AND (t.network,t.block_number,t.log_index)=(e.network,e.block_number,e.log_index)
           AND (SELECT bool_and((leg->>'assetType') IS NOT DISTINCT FROM '3')
                FROM jsonb_array_elements((t.assets->'sent') || (t.assets->'received')) leg)",
    )
    .bind(net.db_name())
    .bind(from)
    .bind(to)
    .bind(&retry)
    .execute(&mut *conn)
    .await?;
    let rows: Vec<Payment> = sqlx::query_as(include_str!("settlements.sql"))
        .bind(net.db_name())
        .bind(from)
        .bind(to)
        .bind(&retry)
        .fetch_all(&mut *conn)
        .await?;
    let mut payments = BTreeMap::new();
    for row in rows {
        payments
            .entry((row.block, row.index))
            .or_insert_with(Vec::new)
            .push(row);
    }
    // Barters and giveaways can mint a token without emitting a payment.
    // Their venue and Issue logs still need the same canonical receipt.
    let executions: Vec<Execution> = sqlx::query_as(
        "SELECT block_number AS block,log_index AS index,tx_hash,venue,kind
         FROM shop.sale_execution WHERE network=$1 AND NOT receipt_read
           AND ((block_number>$2 AND block_number<=$3) OR tx_hash=ANY($4))
         ORDER BY block_number,tx_hash,log_index",
    )
    .bind(net.db_name())
    .bind(from)
    .bind(to)
    .bind(&retry)
    .fetch_all(&mut *conn)
    .await?;
    let txs: BTreeMap<_, _> = executions
        .chunk_by(|a, b| a.tx_hash == b.tx_hash)
        .map(|tx| (tx[0].tx_hash.as_str(), tx))
        .collect();
    let hashes: Vec<_> = txs.keys().copied().collect();
    sqlx::query("UPDATE shop.sale_execution SET receipt_attempted_at=timezone('UTC',now()) WHERE network=$1 AND tx_hash=ANY($2) AND NOT receipt_read")
        .bind(net.db_name()).bind(&hashes).execute(&mut *conn).await?;
    let mut updated = Vec::new();
    // Only one response body is retained. Split an unreadable batch into
    // individual envelopes, each subject to the same transport limit.
    let mut pending: std::collections::VecDeque<_> =
        hashes.chunks(24).map(|batch| (batch, false)).collect();
    while let Some((batch, individual)) = pending.pop_front() {
        let mut receipts = Vec::new();
        if individual {
            let hash = batch[0];
            if let Ok(mut response) = rpc.call("eth_getTransactionReceipt", json!([hash])).await {
                receipts.push((hash, response["result"].take()));
            }
        } else {
            let requests: Vec<_> = batch.iter().enumerate().map(|(i, hash)| {
                json!({"jsonrpc":"2.0","id":i,"method":"eth_getTransactionReceipt","params":[hash]})
            }).collect();
            let mut returned = vec![false; batch.len()];
            for mut answer in rpc.batch(&requests).await.unwrap_or_default() {
                let Some(i) = answer["id"]
                    .as_u64()
                    .and_then(|id| usize::try_from(id).ok())
                else {
                    continue;
                };
                let Some(done) = returned.get_mut(i) else {
                    continue;
                };
                if !*done && let Some(result) = answer.get_mut("result") {
                    *done = true;
                    receipts.push((batch[i], result.take()));
                }
            }
            // A provider can reject the whole batch, return per-call errors,
            // or omit IDs. Retry only the calls without a result envelope.
            for (one, done) in batch.chunks(1).zip(returned) {
                if !done {
                    pending.push_back((one, true));
                }
            }
        }
        let heights: BTreeSet<_> = receipts
            .iter()
            .filter(|(_, r)| r["status"] == "0x1")
            .map(|(h, _)| txs[h][0].block as u64)
            .collect();
        let (headers, _) = rpc
            .block_headers(&heights.into_iter().collect::<Vec<_>>())
            .await?;
        for (expected, mut receipt) in receipts {
            if receipt["status"] != "0x1" {
                continue;
            }
            let tx = txs[expected];
            let first = &tx[0];
            ensure!(
                receipt["transactionHash"]
                    .as_str()
                    .is_some_and(|h| h.eq_ignore_ascii_case(expected))
                    && receipt["blockNumber"].as_str().and_then(hex_to_i64) == Some(first.block),
                "receipt position mismatch"
            );
            let hash = &headers[&(first.block as u64)].hash;
            ensure!(
                receipt["blockHash"]
                    .as_str()
                    .is_some_and(|h| h.eq_ignore_ascii_case(hash)),
                "sale receipt on another branch"
            );
            let logs: Vec<RpcLog> = serde_json::from_value(receipt["logs"].take())?;
            let mut seen = BTreeSet::new();
            let mut previous = -1;
            let mut moves = Vec::new();
            let mut issue_logs = Vec::new();
            for log in &logs {
                let index =
                    i32::try_from(hex_to_i64(&log.log_index).context("receipt log index")?)?;
                ensure!(
                    index > previous
                        && hex_to_i64(&log.block_number) == Some(first.block)
                        && log
                            .transaction_hash
                            .as_deref()
                            .is_some_and(|h| h.eq_ignore_ascii_case(&first.tx_hash))
                        && log
                            .block_hash
                            .as_deref()
                            .is_some_and(|h| h.eq_ignore_ascii_case(hash)),
                    "inconsistent sale receipt log"
                );
                previous = index;
                if let Some(kind) = venue_event(net, log) {
                    if let Ok(i) = tx.binary_search_by_key(&index, |e| e.index) {
                        let execution = &tx[i];
                        ensure!(
                            execution.kind == kind
                                && log.address.eq_ignore_ascii_case(&execution.venue),
                            "receipt venue mismatch"
                        );
                        seen.insert(index);
                        if kind == "trade" {
                            issued_tokens(conn, net, execution, &issue_logs).await?;
                        }
                        if let Some(slice) = payments.get(&(execution.block, index))
                            && let Some(splits) = classify(net, slice, &moves)
                        {
                            for (p, split) in slice.iter().zip(splits) {
                                save(conn, net, p, split).await?;
                            }
                        }
                    }
                    moves.clear();
                    issue_logs.clear();
                } else if log
                    .topics
                    .first()
                    .is_some_and(|t| t.eq_ignore_ascii_case(sales_index::TOPIC_ISSUE))
                {
                    issue_logs.push(log);
                } else if let Some(m) = transfer(log) {
                    moves.push(m);
                }
            }
            ensure!(
                tx.iter().all(|p| seen.contains(&p.index)),
                "receipt missing expected venue execution"
            );
            sqlx::query(
                "UPDATE shop.sale_execution SET receipt_read=true WHERE network=$1 AND tx_hash=$2",
            )
            .bind(net.db_name())
            .bind(&first.tx_hash)
            .execute(&mut *conn)
            .await?;
            updated.push(expected);
        }
    }
    let days=sqlx::query_scalar(
        "SELECT DISTINCT block_time/86400*86400 FROM shop.sale_execution WHERE network=$1 AND (tx_hash=ANY($2) OR (block_number>$3 AND block_number<=$4))")
        .bind(net.db_name()).bind(updated).bind(from).bind(to).fetch_all(&mut *conn).await?;
    Ok(days)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;
    use alloy_sol_types::SolValue;

    #[test]
    fn sales_trade_ignores_unrelated_and_malformed_issue_logs() {
        let collection = "0x1111111111111111111111111111111111111111";
        let buyer = "0x2222222222222222222222222222222222222222";
        let venue = "0x3333333333333333333333333333333333333333";
        let contracts = BTreeSet::from([collection]);
        let mut log =
            sales_index::tests::issue_log(collection, buyer, U256::from(21), 3, venue, 100, 1);
        assert_eq!(
            traded_issue(&log, venue, &contracts),
            Some((collection.into(), "3".into(), buyer.into(), "21".into()))
        );
        assert!(traded_issue(&log, venue, &BTreeSet::from([buyer])).is_none());
        log.data = "0x01".into();
        assert!(traded_issue(&log, venue, &contracts).is_none());
    }

    fn payment(kind: &str) -> Payment {
        Payment {
            block: 1,
            index: 12,
            venue: String::from("venue"),
            kind: kind.into(),
            ordinal: 0,
            currency: "mana".into(),
            gross: "503".into(),
            payer: Some("payer".into()),
            beneficiary: Some("payee".into()),
            bid_fee: None,
            collector: Some("collector".into()),
            rate: Some(50_000),
            royalty_rate: Some(100_000),
            extra: None,
        }
    }
    fn tx(payer: &str, to: &str, amount: u64) -> Transfer {
        Transfer {
            currency: "mana".into(),
            from: payer.into(),
            to: to.into(),
            amount: U256::from(amount),
        }
    }

    #[test]
    fn sales_legacy_estate_receipt_conserves_the_observed_seller_and_burner_payments() {
        let logs: Vec<RpcLog> = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/shop/estate-execution-7824033.json"
        )))
        .unwrap();
        let order = nft_index::decode_log(logs.last().unwrap(), 0)
            .unwrap()
            .unwrap();
        let mut p = payment("order");
        p.venue = order.emitter;
        p.gross = order.amount.unwrap();
        p.payer = order.to_address;
        p.beneficiary = order.from_address;
        p.collector = None;
        p.rate = None;
        p.currency = super::super::listings::mana_token(1).unwrap().into();
        let moves: Vec<_> = logs.iter().filter_map(transfer).collect();
        let splits = classify(ChainNet::Ethereum, &[p], &moves).unwrap();
        let legs = &splits[0].legs;
        assert_eq!(legs[0].amount.to_string(), "1286999010000000000000000");
        assert_eq!(legs[1].role, "burn");
        assert_eq!(
            legs[1].recipient,
            "0xadfeb1de7876fcabeaf87df5a6c566b70f970018"
        );
        assert_eq!(legs[1].amount.to_string(), "12999990000000000000000");
    }

    #[test]
    fn sales_polygon_bundle_respects_the_logged_recipients_and_returns_rounding_to_the_payer() {
        let addresses = vec![
            Address::repeat_byte(1),
            Address::repeat_byte(2),
            Address::repeat_byte(3),
            Address::ZERO,
        ];
        let mut p = payment("trade");
        p.extra = Some(format!(
            "0x{}",
            hex::encode((true, U256::from(3), addresses.clone()).abi_encode_params())
        ));
        let mut moves = vec![tx("unrelated", "other", 100), tx("payer", "collector", 25)];
        for a in &addresses[..3] {
            moves.push(tx("payer", &format!("{a:#x}"), 16));
        }
        moves.push(tx("payer", "payee", 428));
        let rows = [p];
        let splits = classify(ChainNet::Matic, &rows, &moves).unwrap();
        let retained = splits[0].legs.last().unwrap();
        assert_eq!(
            (retained.role, retained.recipient.as_str(), retained.amount),
            ("retained", "payer", U256::from(2))
        );
        assert_eq!(
            splits[0].legs.iter().map(|l| l.amount).sum::<U256>(),
            U256::from(503)
        );
        moves.push(tx("payer", "collector", 1));
        assert!(classify(ChainNet::Matic, &rows, &moves).is_none());
    }

    #[test]
    fn sales_free_store_line_has_zero_fees_without_inventing_a_payer() {
        let mut p = payment("store");
        p.gross = "0".into();
        p.rate = None;
        let split = classify(ChainNet::Matic, &[p], &[]).unwrap();
        assert!(split[0].legs.is_empty() && split[0].payer.is_none());
        assert!(classify(ChainNet::Matic, &[payment("store")], &[]).is_none());
    }

    #[test]
    fn sales_store_fee_is_rounded_per_line_and_can_go_to_a_different_address_from_the_buyer() {
        let mut a = payment("store");
        a.gross = "51".into();
        a.rate = Some(100_000);
        let mut b = payment("store");
        b.gross = "19".into();
        b.rate = a.rate;
        b.ordinal = 1;
        let rows = [a, b];
        let moves = [
            tx("payer", "creator-payee", 46),
            tx("payer", "second-payee", 18),
            tx("payer", "collector", 6),
        ];
        let split = classify(ChainNet::Matic, &rows, &moves).unwrap();
        assert_eq!(split[0].payer.as_deref(), Some("payer"));
        assert_eq!(split[0].payee.as_deref(), Some("creator-payee"));
        assert_eq!(split[0].legs[1].amount, U256::from(5));
        assert_eq!(split[1].legs[1].amount, U256::from(1));
        assert!(classify(ChainNet::Matic, &rows, &moves[..2]).is_none());
    }
}
