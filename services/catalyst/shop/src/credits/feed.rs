//! The MANA/USD rate a credits group's USD-pegged lines are sized at: the
//! answer of the feed the group's own marketplace converts them with when the
//! trade settles, read for every authorization (never cached, never the
//! globally configured aggregator, never the polled snapshot as a fallback).
//!
//! One read is coherent: the endpoint's chain id is checked, the latest
//! block's hash is taken, and the marketplace's `manaUsdAggregator()` and
//! `manaUsdAggregatorTolerance()` and the aggregator's `decimals()` and
//! `latestRoundData()` are all read at that hash with `requireCanonical`
//! (EIP-1898), so a reorg between the calls fails the read instead of
//! mixing two states. The endpoint must support EIP-1898 block hashes in
//! `eth_call`; one that does not is refused, never read at a number or at
//! `latest`.
//!
//! Freshness follows the marketplace's own check (`AggregatorHelper`:
//! `updatedAt < block.timestamp - tolerance` reverts, so a tolerance of 0
//! demands a round from that very block, and a tolerance above the block's
//! timestamp makes the subtraction revert), judged at the read block, and
//! separately `CREDITS_USD_PEGGED_FEED_MAX_AGE_SECS` against this node's
//! clock. Any failure, a zero aggregator, a non-positive answer or a round
//! newer than the block or the clock refuses the authorization before
//! anything is signed or reserved. [`READ_DEADLINE`] bounds the whole read,
//! response bodies included. A move of the feed after the read (during the
//! credit's TTL) is outside what this read can see.

use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Address, B256, U256};
use alloy_sol_types::{SolCall, sol};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::evm::chain::ChainComponent;
use crate::evm::oracle;
use crate::types::ManaUsdRate;

pub const SOURCE: &str = "credits-usd-pegged-feed";
pub const CONFIGURE: &str = "RPC_ENDPOINT_POLYGON";
pub const READ_DEADLINE: Duration = Duration::from_secs(5);
/// How far a round's `updatedAt` may run ahead of this node's clock.
const CLOCK_SKEW_SECS: i64 = 60;

sol! {
    function manaUsdAggregator() external view returns (address);
    function manaUsdAggregatorTolerance() external view returns (uint256);
    function latestRoundData() external view returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound);
    function decimals() external view returns (uint8);
}

/// One coherent read of a marketplace's feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedReading {
    /// The answer at 18 decimals.
    pub rate: ManaUsdRate,
    pub aggregator: Address,
    pub tolerance_secs: u64,
    pub updated_at_s: i64,
    pub block: u64,
    pub block_hash: B256,
}

#[async_trait]
pub trait MarketplaceFeed: Send + Sync {
    /// `Err(reason)` refuses the authorization; the reason never holds a URL.
    async fn read(&self, chain_id: u64, marketplace: Address) -> Result<FeedReading, String>;
}

/// No endpoint for the credits chain: every USD-pegged group is refused.
pub struct NoFeed;

#[async_trait]
impl MarketplaceFeed for NoFeed {
    async fn read(&self, _chain_id: u64, _marketplace: Address) -> Result<FeedReading, String> {
        Err("no JSON-RPC endpoint for the credits chain is configured".to_string())
    }
}

/// The feed over the credits chain's JSON-RPC endpoint.
pub struct ChainFeed {
    chain: Arc<ChainComponent>,
    max_age_secs: u64,
    deadline: Duration,
}

impl ChainFeed {
    pub fn new(chain: Arc<ChainComponent>, max_age_secs: u64) -> Self {
        Self {
            chain,
            max_age_secs,
            deadline: READ_DEADLINE,
        }
    }

    #[cfg(test)]
    fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline = deadline;
        self
    }

    async fn call_at(
        &self,
        to: Address,
        data: Vec<u8>,
        block_hash: B256,
        what: &str,
    ) -> Result<Vec<u8>, String> {
        let params = json!([
            { "to": format!("{to:#x}"), "data": format!("0x{}", hex::encode(data)) },
            { "blockHash": format!("{block_hash:#x}"), "requireCanonical": true },
        ]);
        let out = self
            .chain
            .rpc()
            .call_hex("eth_call", params)
            .await
            .map_err(|e| format!("{what} could not be read ({})", redact(&e.to_string())))?;
        hex::decode(out.trim_start_matches("0x"))
            .map_err(|_| format!("{what} answered a non-hex result"))
    }

    async fn read_pinned(
        &self,
        chain_id: u64,
        marketplace: Address,
        now_s: i64,
    ) -> Result<FeedReading, String> {
        let rpc = self.chain.rpc();
        let endpoint_chain = rpc
            .call_hex("eth_chainId", json!([]))
            .await
            .map_err(|e| format!("eth_chainId could not be read ({})", redact(&e.to_string())))?;
        let endpoint_chain =
            quantity(&endpoint_chain).ok_or("eth_chainId answered a non-quantity")?;
        if endpoint_chain != chain_id {
            return Err(format!(
                "the credits chain endpoint serves chain {endpoint_chain}, not {chain_id}"
            ));
        }
        let head = rpc
            .call("eth_getBlockByNumber", json!(["latest", false]))
            .await
            .map_err(|e| {
                format!(
                    "the latest block could not be read ({})",
                    redact(&e.to_string())
                )
            })?;
        let (block, block_hash, block_time) =
            block_header(&head).ok_or("the latest block has no number, hash or timestamp")?;
        let tag = block_hash;

        let aggregator = self
            .call_at(
                marketplace,
                manaUsdAggregatorCall {}.abi_encode(),
                tag,
                "manaUsdAggregator()",
            )
            .await?;
        let aggregator = manaUsdAggregatorCall::abi_decode_returns(&aggregator)
            .map_err(|_| "manaUsdAggregator() is not an address".to_string())?;
        if aggregator == Address::ZERO {
            return Err(format!(
                "marketplace {marketplace:#x} has no MANA/USD aggregator"
            ));
        }
        let tolerance = self
            .call_at(
                marketplace,
                manaUsdAggregatorToleranceCall {}.abi_encode(),
                tag,
                "manaUsdAggregatorTolerance()",
            )
            .await?;
        let tolerance = manaUsdAggregatorToleranceCall::abi_decode_returns(&tolerance)
            .map_err(|_| "manaUsdAggregatorTolerance() is not a uint256".to_string())?;
        // The contract computes `block.timestamp - tolerance`: a tolerance
        // above the timestamp reverts there, so it is refused here.
        if tolerance > U256::from(u64::try_from(block_time).unwrap_or(0)) {
            return Err(format!(
                "manaUsdAggregatorTolerance() is {tolerance}, above block {block}'s timestamp; the marketplace's check would revert"
            ));
        }
        let tolerance_secs = u64::try_from(tolerance).unwrap_or(u64::MAX);

        let decimals = self
            .call_at(aggregator, decimalsCall {}.abi_encode(), tag, "decimals()")
            .await?;
        let round = self
            .call_at(
                aggregator,
                latestRoundDataCall {}.abi_encode(),
                tag,
                "latestRoundData()",
            )
            .await?;
        let reading = oracle::decode_rate_reading(&decimals, &round).map_err(|e| e.to_string())?;
        let rate = ManaUsdRate::new(reading.rate_18, 18)
            .ok_or("the MANA/USD answer is not a positive rate")?;

        let updated_at_s = reading.updated_at_s;
        if updated_at_s > block_time || updated_at_s > now_s + CLOCK_SKEW_SECS {
            return Err(format!(
                "the MANA/USD round is dated {updated_at_s}, after block {block} ({block_time}) or this node's clock"
            ));
        }
        // The marketplace's check at the read block: 0 tolerance demands a
        // round from that block.
        let oldest_for_contract = block_time - tolerance_secs as i64;
        if updated_at_s < oldest_for_contract {
            return Err(format!(
                "the MANA/USD round ({updated_at_s}) is older than block {block}'s timestamp ({block_time}) minus the marketplace's tolerance ({tolerance_secs}s); the marketplace would revert"
            ));
        }
        let age = now_s - updated_at_s;
        if age > i64::try_from(self.max_age_secs).unwrap_or(i64::MAX) {
            return Err(format!(
                "the MANA/USD round is {age}s old, past CREDITS_USD_PEGGED_FEED_MAX_AGE_SECS ({}s)",
                self.max_age_secs
            ));
        }
        Ok(FeedReading {
            rate,
            aggregator,
            tolerance_secs,
            updated_at_s,
            block,
            block_hash,
        })
    }
}

#[async_trait]
impl MarketplaceFeed for ChainFeed {
    async fn read(&self, chain_id: u64, marketplace: Address) -> Result<FeedReading, String> {
        let now_s = chrono::Utc::now().timestamp();
        tokio::time::timeout(
            self.deadline,
            self.read_pinned(chain_id, marketplace, now_s),
        )
        .await
        .map_err(|_| {
            format!(
                "the MANA/USD feed read took longer than {}ms",
                self.deadline.as_millis()
            )
        })?
    }
}

fn quantity(hex_text: &str) -> Option<u64> {
    u64::from_str_radix(hex_text.strip_prefix("0x")?, 16).ok()
}

fn block_header(block: &Value) -> Option<(u64, B256, i64)> {
    let number = quantity(block.get("number")?.as_str()?)?;
    let hash_text = block.get("hash")?.as_str()?.strip_prefix("0x")?;
    if hash_text.len() != 64 {
        return None;
    }
    let hash = B256::from_slice(&hex::decode(hash_text).ok()?);
    let time = i64::try_from(quantity(block.get("timestamp")?.as_str()?)?).ok()?;
    Some((number, hash, time))
}

fn redact(message: &str) -> String {
    message
        .split_whitespace()
        .map(|word| if word.contains("://") { "<url>" } else { word })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(200)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::Outbound;
    use alloy_primitives::I256;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn rate_18(usd_per_mana: &str) -> Option<ManaUsdRate> {
        ManaUsdRate::parse_usd_per_mana(usd_per_mana)
    }

    #[derive(Clone)]
    struct Chain {
        chain_id: u64,
        block: u64,
        block_time: i64,
        aggregator: Address,
        tolerance: U256,
        answer: I256,
        decimals: u8,
        updated_at: u64,
        stall: bool,
        /// Whether `eth_call` takes an EIP-1898 block object.
        eip1898: bool,
        /// After this many eth_calls the read block stops being canonical
        /// (another block of the same height replaces it).
        reorg_after: Option<usize>,
    }

    const MARKETPLACE: Address = Address::new([0x54; 20]);
    const AGGREGATOR: Address = Address::new([0xa1; 20]);
    const OTHER_AGGREGATOR: Address = Address::new([0xb2; 20]);
    const HASH_A: B256 = B256::new([0xaa; 32]);

    fn chain(now: i64) -> Chain {
        Chain {
            chain_id: 137,
            block: 0x1234,
            block_time: now,
            aggregator: AGGREGATOR,
            tolerance: U256::from(3_600),
            // 0.45 USD/MANA at 8 decimals.
            answer: I256::try_from(45_000_000i64).unwrap(),
            decimals: 8,
            updated_at: (now - 30) as u64,
            stall: false,
            eip1898: true,
            reorg_after: None,
        }
    }

    fn word(v: U256) -> String {
        hex::encode(v.to_be_bytes::<32>())
    }

    /// A JSON-RPC endpoint answering from `c`, recording the block parameter
    /// of every eth_call. Block `HASH_A` is the latest; once it is reorged
    /// out, a call that requires it canonical fails, and one that does not
    /// would read the replacing block, whose marketplace names another
    /// aggregator.
    async fn serve(c: Chain) -> (String, Arc<Mutex<Vec<Value>>>) {
        let blocks = Arc::new(Mutex::new(Vec::new()));
        let seen = blocks.clone();
        let calls = Arc::new(AtomicUsize::new(0));
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move |axum::Json(req): axum::Json<Value>| {
                let c = c.clone();
                let seen = seen.clone();
                let calls = calls.clone();
                async move {
                    if c.stall {
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                    let id = req.get("id").cloned().unwrap_or(json!(1));
                    let fail = |message: &str| {
                        axum::Json(json!({ "jsonrpc": "2.0", "id": id.clone(), "error": { "code": -32000, "message": message } }))
                    };
                    let result = match req["method"].as_str() {
                        Some("eth_chainId") => json!(format!("{:#x}", c.chain_id)),
                        Some("eth_getBlockByNumber") => json!({
                            "number": format!("{:#x}", c.block),
                            "hash": format!("{HASH_A:#x}"),
                            "timestamp": format!("{:#x}", c.block_time),
                        }),
                        Some("eth_call") => {
                            let block = req["params"][1].clone();
                            seen.lock().unwrap().push(block.clone());
                            let n = calls.fetch_add(1, Ordering::SeqCst);
                            let reorged = c.reorg_after.is_some_and(|after| n >= after);
                            let aggregator = if block.is_object() {
                                if !c.eip1898 {
                                    return fail("invalid argument 1: hex string without 0x prefix");
                                }
                                if block["blockHash"] != json!(format!("{HASH_A:#x}")) {
                                    return fail("header not found");
                                }
                                if reorged && block["requireCanonical"] == json!(true) {
                                    return fail("block hash is not canonical");
                                }
                                if reorged { OTHER_AGGREGATOR } else { c.aggregator }
                            } else if reorged {
                                OTHER_AGGREGATOR
                            } else {
                                c.aggregator
                            };
                            let to = req["params"][0]["to"].as_str().unwrap_or("").to_string();
                            let data = req["params"][0]["data"].as_str().unwrap_or("").to_string();
                            let sel = |s: [u8; 4]| data.starts_with(&format!("0x{}", hex::encode(s)));
                            let on_marketplace = to == format!("{MARKETPLACE:#x}");
                            let on_feed = to == format!("{:#x}", c.aggregator)
                                || to == format!("{OTHER_AGGREGATOR:#x}");
                            let out = if on_marketplace && sel(manaUsdAggregatorCall::SELECTOR) {
                                format!("{:0>64}", hex::encode(aggregator))
                            } else if on_marketplace && sel(manaUsdAggregatorToleranceCall::SELECTOR) {
                                word(c.tolerance)
                            } else if on_feed && sel(decimalsCall::SELECTOR) {
                                word(U256::from(c.decimals))
                            } else if on_feed && sel(latestRoundDataCall::SELECTOR) {
                                format!(
                                    "{}{}{}{}{}",
                                    word(U256::from(1)),
                                    hex::encode(c.answer.to_be_bytes::<32>()),
                                    word(U256::from(c.updated_at)),
                                    word(U256::from(c.updated_at)),
                                    word(U256::from(1)),
                                )
                            } else {
                                return fail("execution reverted");
                            };
                            json!(format!("0x{out}"))
                        }
                        _ => json!(null),
                    };
                    axum::Json(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}/"), blocks)
    }

    async fn feed(c: Chain, max_age_secs: u64) -> (ChainFeed, Arc<Mutex<Vec<Value>>>) {
        let (url, blocks) = serve(c).await;
        let component = Arc::new(ChainComponent::new(
            Outbound::for_tests(),
            &url,
            Duration::from_secs(30),
        ));
        (ChainFeed::new(component, max_age_secs), blocks)
    }

    #[tokio::test]
    async fn the_read_comes_from_the_marketplace_feed_at_one_canonical_block_hash() {
        let now = chrono::Utc::now().timestamp();
        let (f, blocks) = feed(chain(now), 3_600).await;
        let got = f.read(137, MARKETPLACE).await.unwrap();
        assert_eq!(got.aggregator, AGGREGATOR);
        assert_eq!(got.tolerance_secs, 3_600);
        assert_eq!((got.block, got.block_hash), (0x1234, HASH_A));
        assert_eq!(
            got.rate,
            rate_18("0.45").unwrap(),
            "8 decimals normalized to 18"
        );
        let blocks = blocks.lock().unwrap().clone();
        assert_eq!(blocks.len(), 4, "aggregator, tolerance, decimals, round");
        let pinned = json!({ "blockHash": format!("{HASH_A:#x}"), "requireCanonical": true });
        assert!(
            blocks.iter().all(|b| *b == pinned),
            "every call at the canonical hash: {blocks:?}"
        );
    }

    #[tokio::test]
    async fn a_reorg_between_the_calls_refuses_instead_of_mixing_two_blocks() {
        let now = chrono::Utc::now().timestamp();
        for after in 1..4 {
            let (f, _) = feed(
                Chain {
                    reorg_after: Some(after),
                    ..chain(now)
                },
                3_600,
            )
            .await;
            let err = f.read(137, MARKETPLACE).await.unwrap_err();
            assert!(
                err.contains("not canonical"),
                "reorg after {after} calls: {err}"
            );
        }
    }

    #[tokio::test]
    async fn an_endpoint_without_block_hash_calls_is_refused() {
        let now = chrono::Utc::now().timestamp();
        let (f, blocks) = feed(
            Chain {
                eip1898: false,
                ..chain(now)
            },
            3_600,
        )
        .await;
        assert!(f.read(137, MARKETPLACE).await.is_err());
        let blocks = blocks.lock().unwrap().clone();
        assert!(
            blocks.iter().all(Value::is_object),
            "no fallback to a number or latest: {blocks:?}"
        );
    }

    #[tokio::test]
    async fn freshness_follows_the_marketplaces_check_and_the_local_limit() {
        let now = chrono::Utc::now().timestamp();
        let at = |tolerance: u64, updated_at: i64| Chain {
            tolerance: U256::from(tolerance),
            updated_at: updated_at as u64,
            ..chain(now)
        };
        let cases: Vec<(&str, Chain, u64, bool)> = vec![
            (
                "tolerance 0, a round 30s old",
                at(0, now - 30),
                3_600,
                false,
            ),
            (
                "tolerance 0, a round of the block itself",
                at(0, now),
                3_600,
                true,
            ),
            ("exactly at the tolerance", at(30, now - 30), 3_600, true),
            (
                "one second past the tolerance",
                at(30, now - 31),
                3_600,
                false,
            ),
            (
                "within the tolerance, past the local limit",
                at(3_600, now - 30),
                10,
                false,
            ),
            (
                "a tolerance above the block's timestamp",
                Chain {
                    tolerance: U256::from(now as u64 + 1),
                    ..chain(now)
                },
                3_600,
                false,
            ),
            (
                "an unrepresentable tolerance",
                Chain {
                    tolerance: U256::MAX,
                    ..chain(now)
                },
                3_600,
                false,
            ),
        ];
        for (what, c, max_age, ok) in cases {
            let (f, _) = feed(c, max_age).await;
            assert_eq!(f.read(137, MARKETPLACE).await.is_ok(), ok, "{what}");
        }
    }

    #[tokio::test]
    async fn every_unusable_read_is_refused() {
        let now = chrono::Utc::now().timestamp();
        let cases: Vec<(&str, Chain)> = vec![
            (
                "another chain",
                Chain {
                    chain_id: 1,
                    ..chain(now)
                },
            ),
            (
                "no aggregator",
                Chain {
                    aggregator: Address::ZERO,
                    ..chain(now)
                },
            ),
            (
                "a zero answer",
                Chain {
                    answer: I256::ZERO,
                    ..chain(now)
                },
            ),
            (
                "a negative answer",
                Chain {
                    answer: I256::try_from(-1i64).unwrap(),
                    ..chain(now)
                },
            ),
            (
                "more than 18 decimals",
                Chain {
                    decimals: 19,
                    ..chain(now)
                },
            ),
            (
                "a round after the block",
                Chain {
                    updated_at: (now + 5) as u64,
                    ..chain(now)
                },
            ),
            (
                "a round from the future",
                Chain {
                    updated_at: (now + 600) as u64,
                    block_time: now + 600,
                    ..chain(now)
                },
            ),
        ];
        for (what, c) in cases {
            let (f, _) = feed(c, 3_600).await;
            assert!(
                f.read(137, MARKETPLACE).await.is_err(),
                "{what} must be refused"
            );
        }
    }

    #[tokio::test]
    async fn a_stalled_endpoint_fails_within_the_deadline() {
        let now = chrono::Utc::now().timestamp();
        let (f, _) = feed(
            Chain {
                stall: true,
                ..chain(now)
            },
            3_600,
        )
        .await;
        let f = f.with_deadline(Duration::from_millis(300));
        let started = std::time::Instant::now();
        let err = f.read(137, MARKETPLACE).await.unwrap_err();
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        assert!(err.contains("longer than"), "{err}");
    }

    #[tokio::test]
    async fn no_endpoint_refuses() {
        assert!(NoFeed.read(137, MARKETPLACE).await.is_err());
    }
}
