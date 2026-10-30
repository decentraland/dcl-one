//! The MANA/USD rate a credits group's USD-pegged lines are sized at: the
//! answer of the feed the group's own marketplace converts them with when the
//! trade settles, read for every authorization (never cached, never the
//! globally configured aggregator, never the polled snapshot as a fallback).
//!
//! One read is coherent: the endpoint's chain id is checked, a block is
//! pinned, and the marketplace's `manaUsdAggregator()` and
//! `manaUsdAggregatorTolerance()` and the aggregator's `decimals()` and
//! `latestRoundData()` are all read at that block. Any failure, a zero
//! aggregator, a non-positive answer, a round newer than the block or the
//! clock, or one older than the contract's tolerance or
//! `CREDITS_USD_PEGGED_FEED_MAX_AGE_SECS` refuses the authorization before
//! anything is signed or reserved. [`READ_DEADLINE`] bounds the whole read,
//! response bodies included.

use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::Address;
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
        block: &str,
        what: &str,
    ) -> Result<Vec<u8>, String> {
        let params = json!([{ "to": format!("{to:#x}"), "data": format!("0x{}", hex::encode(data)) }, block]);
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
        let (block, block_time) =
            block_number_and_time(&head).ok_or("the latest block has no number or timestamp")?;
        let tag = format!("{block:#x}");

        let aggregator = self
            .call_at(
                marketplace,
                manaUsdAggregatorCall {}.abi_encode(),
                &tag,
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
                &tag,
                "manaUsdAggregatorTolerance()",
            )
            .await?;
        let tolerance = manaUsdAggregatorToleranceCall::abi_decode_returns(&tolerance)
            .map_err(|_| "manaUsdAggregatorTolerance() is not a uint256".to_string())?;
        let tolerance_secs = u64::try_from(tolerance).unwrap_or(u64::MAX);

        let decimals = self
            .call_at(aggregator, decimalsCall {}.abi_encode(), &tag, "decimals()")
            .await?;
        let round = self
            .call_at(
                aggregator,
                latestRoundDataCall {}.abi_encode(),
                &tag,
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
        let limit = match tolerance_secs {
            0 => self.max_age_secs,
            t => t.min(self.max_age_secs),
        };
        let age = now_s.max(block_time) - updated_at_s;
        if age > i64::try_from(limit).unwrap_or(i64::MAX) {
            return Err(format!(
                "the MANA/USD round is {age}s old, past the {limit}s the marketplace's tolerance ({tolerance_secs}s) and CREDITS_USD_PEGGED_FEED_MAX_AGE_SECS ({}s) allow",
                self.max_age_secs
            ));
        }
        Ok(FeedReading {
            rate,
            aggregator,
            tolerance_secs,
            updated_at_s,
            block,
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

fn block_number_and_time(block: &Value) -> Option<(u64, i64)> {
    let number = quantity(block.get("number")?.as_str()?)?;
    let time = i64::try_from(quantity(block.get("timestamp")?.as_str()?)?).ok()?;
    Some((number, time))
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
    use alloy_primitives::{I256, U256};
    use std::sync::Mutex;

    fn rate_18(usd_per_mana: &str) -> Option<ManaUsdRate> {
        ManaUsdRate::parse_usd_per_mana(usd_per_mana)
    }

    #[derive(Clone)]
    struct Chain {
        chain_id: u64,
        block: u64,
        block_time: i64,
        aggregator: Address,
        tolerance: u64,
        answer: I256,
        decimals: u8,
        updated_at: u64,
        stall: bool,
    }

    const MARKETPLACE: Address = Address::new([0x54; 20]);
    const AGGREGATOR: Address = Address::new([0xa1; 20]);

    fn chain(now: i64) -> Chain {
        Chain {
            chain_id: 137,
            block: 0x1234,
            block_time: now,
            aggregator: AGGREGATOR,
            tolerance: 3_600,
            // 0.45 USD/MANA at 8 decimals.
            answer: I256::try_from(45_000_000i64).unwrap(),
            decimals: 8,
            updated_at: (now - 30) as u64,
            stall: false,
        }
    }

    fn word(v: U256) -> String {
        hex::encode(v.to_be_bytes::<32>())
    }

    /// A JSON-RPC endpoint answering from `c`, recording the block tag of
    /// every eth_call.
    async fn serve(c: Chain) -> (String, Arc<Mutex<Vec<String>>>) {
        let tags = Arc::new(Mutex::new(Vec::new()));
        let seen = tags.clone();
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move |axum::Json(req): axum::Json<Value>| {
                let c = c.clone();
                let seen = seen.clone();
                async move {
                    if c.stall {
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                    let id = req.get("id").cloned().unwrap_or(json!(1));
                    let result = match req["method"].as_str() {
                        Some("eth_chainId") => json!(format!("{:#x}", c.chain_id)),
                        Some("eth_getBlockByNumber") => json!({
                            "number": format!("{:#x}", c.block),
                            "timestamp": format!("{:#x}", c.block_time),
                        }),
                        Some("eth_call") => {
                            seen.lock().unwrap().push(req["params"][1].as_str().unwrap_or("").to_string());
                            let to = req["params"][0]["to"].as_str().unwrap_or("").to_string();
                            let data = req["params"][0]["data"].as_str().unwrap_or("").to_string();
                            let sel = |s: [u8; 4]| data.starts_with(&format!("0x{}", hex::encode(s)));
                            let out = if to == format!("{MARKETPLACE:#x}") && sel(manaUsdAggregatorCall::SELECTOR) {
                                format!("{:0>64}", hex::encode(c.aggregator))
                            } else if to == format!("{MARKETPLACE:#x}") && sel(manaUsdAggregatorToleranceCall::SELECTOR) {
                                word(U256::from(c.tolerance))
                            } else if to == format!("{:#x}", c.aggregator) && sel(decimalsCall::SELECTOR) {
                                word(U256::from(c.decimals))
                            } else if to == format!("{:#x}", c.aggregator) && sel(latestRoundDataCall::SELECTOR) {
                                format!(
                                    "{}{}{}{}{}",
                                    word(U256::from(1)),
                                    hex::encode(c.answer.to_be_bytes::<32>()),
                                    word(U256::from(c.updated_at)),
                                    word(U256::from(c.updated_at)),
                                    word(U256::from(1)),
                                )
                            } else {
                                return axum::Json(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": 3, "message": "execution reverted" } }));
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
        (format!("http://{addr}/"), tags)
    }

    async fn feed(c: Chain, max_age_secs: u64) -> (ChainFeed, Arc<Mutex<Vec<String>>>) {
        let (url, tags) = serve(c).await;
        let component = Arc::new(ChainComponent::new(
            Outbound::for_tests(),
            &url,
            Duration::from_secs(30),
        ));
        (ChainFeed::new(component, max_age_secs), tags)
    }

    #[tokio::test]
    async fn the_read_comes_from_the_marketplace_feed_at_one_pinned_block() {
        let now = chrono::Utc::now().timestamp();
        let (f, tags) = feed(chain(now), 3_600).await;
        let got = f.read(137, MARKETPLACE).await.unwrap();
        assert_eq!(got.aggregator, AGGREGATOR);
        assert_eq!(got.tolerance_secs, 3_600);
        assert_eq!(got.block, 0x1234);
        assert_eq!(
            got.rate,
            rate_18("0.45").unwrap(),
            "8 decimals normalized to 18"
        );
        let tags = tags.lock().unwrap().clone();
        assert_eq!(tags.len(), 4, "aggregator, tolerance, decimals, round");
        assert!(
            tags.iter().all(|t| t == "0x1234"),
            "every call at the pinned block: {tags:?}"
        );
    }

    #[tokio::test]
    async fn every_unusable_read_is_refused() {
        let now = chrono::Utc::now().timestamp();
        let cases: Vec<(&str, Chain, u64)> = vec![
            (
                "another chain",
                Chain {
                    chain_id: 1,
                    ..chain(now)
                },
                3_600,
            ),
            (
                "no aggregator",
                Chain {
                    aggregator: Address::ZERO,
                    ..chain(now)
                },
                3_600,
            ),
            (
                "a zero answer",
                Chain {
                    answer: I256::ZERO,
                    ..chain(now)
                },
                3_600,
            ),
            (
                "a negative answer",
                Chain {
                    answer: I256::try_from(-1i64).unwrap(),
                    ..chain(now)
                },
                3_600,
            ),
            (
                "more than 18 decimals",
                Chain {
                    decimals: 19,
                    ..chain(now)
                },
                3_600,
            ),
            (
                "a round after the block",
                Chain {
                    updated_at: (now + 5) as u64,
                    ..chain(now)
                },
                3_600,
            ),
            (
                "a round from the future",
                Chain {
                    updated_at: (now + 600) as u64,
                    block_time: now + 600,
                    ..chain(now)
                },
                3_600,
            ),
            (
                "older than the marketplace's tolerance",
                Chain {
                    tolerance: 20,
                    ..chain(now)
                },
                3_600,
            ),
            ("older than the local limit", chain(now), 10),
            (
                "no tolerance and older than the local limit",
                Chain {
                    tolerance: 0,
                    ..chain(now)
                },
                10,
            ),
        ];
        for (what, c, max_age) in cases {
            let (f, _) = feed(c, max_age).await;
            assert!(
                f.read(137, MARKETPLACE).await.is_err(),
                "{what} must be refused"
            );
        }
        let (f, _) = feed(
            Chain {
                tolerance: 0,
                ..chain(now)
            },
            3_600,
        )
        .await;
        assert!(
            f.read(137, MARKETPLACE).await.is_ok(),
            "no tolerance leaves the local limit"
        );
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
