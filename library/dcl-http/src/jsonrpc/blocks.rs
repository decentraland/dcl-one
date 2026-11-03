//! The last block at or before a timestamp: a binary search over block
//! timestamps (the search of `@dcl/block-indexer`), transport-agnostic so any
//! chain reader can drive it. Every block read is kept, so the next search
//! starts from the closest known blocks and a repeated one asks nothing.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Mutex;

use serde_json::{Value, json};

use super::RpcError;

/// Blocks kept before the memo starts over.
const KNOWN_CAP: usize = 100_000;

/// A block's number and its timestamp (unix seconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockInfo {
    pub number: u64,
    pub timestamp: i64,
}

/// Which block to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTag {
    Latest,
    Number(u64),
}

impl BlockTag {
    /// The JSON-RPC block parameter.
    pub fn param(self) -> String {
        match self {
            BlockTag::Latest => "latest".to_string(),
            BlockTag::Number(n) => format!("0x{n:x}"),
        }
    }
}

/// `eth_getBlockByNumber` params for `tag`, without transactions.
pub fn get_block_params(tag: BlockTag) -> Value {
    json!([tag.param(), false])
}

/// The `eth_getBlockByNumber` result: `None` for a block the node does not have.
pub fn block_info(result: &Value) -> Result<Option<BlockInfo>, RpcError> {
    if result.is_null() {
        return Ok(None);
    }
    let quantity = |key: &str| {
        result
            .get(key)
            .and_then(Value::as_str)
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .ok_or_else(|| RpcError::Malformed(format!("block without a hex {key}")))
    };
    Ok(Some(BlockInfo {
        number: quantity("number")?,
        timestamp: i64::try_from(quantity("timestamp")?)
            .map_err(|_| RpcError::Malformed("block timestamp out of range".into()))?,
    }))
}

/// The search and the blocks it has read (timestamp to the highest block seen at it).
#[derive(Debug, Default)]
pub struct BlockSearch {
    known: Mutex<BTreeMap<i64, u64>>,
}

impl BlockSearch {
    pub fn new() -> Self {
        Self::default()
    }

    fn remember(&self, block: BlockInfo) {
        let mut known = self.known.lock().unwrap_or_else(|e| e.into_inner());
        if known.len() >= KNOWN_CAP {
            known.clear();
        }
        let slot = known.entry(block.timestamp).or_insert(block.number);
        *slot = (*slot).max(block.number);
    }

    fn bounds(&self, timestamp: i64) -> (Option<BlockInfo>, Option<BlockInfo>) {
        let known = self.known.lock().unwrap_or_else(|e| e.into_inner());
        let info = |(t, n): (&i64, &u64)| BlockInfo {
            number: *n,
            timestamp: *t,
        };
        (
            known.range(..=timestamp).next_back().map(info),
            known.range(timestamp.saturating_add(1)..).next().map(info),
        )
    }

    /// The last block at or before `timestamp` (unix seconds); the head when
    /// the head is older, `None` before the first block. `fetch` reads one block.
    pub async fn block_at<F, Fut, E>(
        &self,
        timestamp: i64,
        fetch: F,
    ) -> Result<Option<BlockInfo>, E>
    where
        F: Fn(BlockTag) -> Fut,
        Fut: Future<Output = Result<BlockInfo, E>>,
    {
        let (lo, hi) = self.bounds(timestamp);
        let mut hi = match hi {
            Some(hi) => hi,
            None => {
                let head = fetch(BlockTag::Latest).await?;
                self.remember(head);
                if head.timestamp <= timestamp {
                    return Ok(Some(head));
                }
                head
            }
        };
        let mut lo = match lo {
            Some(lo) if lo.number < hi.number => lo,
            _ => {
                let first = fetch(BlockTag::Number(0)).await?;
                self.remember(first);
                if first.timestamp > timestamp {
                    return Ok(None);
                }
                first
            }
        };
        while hi.number > lo.number + 1 {
            let mid = lo.number + (hi.number - lo.number) / 2;
            let block = fetch(BlockTag::Number(mid)).await?;
            self.remember(block);
            if block.timestamp <= timestamp {
                lo = block;
            } else {
                hi = block;
            }
        }
        Ok(Some(lo))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Block `n` at second `1000 + 2n`, head at block 1000.
    fn chain(
        reads: &AtomicUsize,
    ) -> impl Fn(BlockTag) -> std::future::Ready<Result<BlockInfo, ()>> + '_ {
        move |tag| {
            reads.fetch_add(1, Ordering::SeqCst);
            let number = match tag {
                BlockTag::Latest => 1000,
                BlockTag::Number(n) => n,
            };
            std::future::ready(Ok(BlockInfo {
                number,
                timestamp: 1000 + 2 * number as i64,
            }))
        }
    }

    #[tokio::test]
    async fn finds_the_last_block_at_or_before_and_reuses_what_it_read() {
        let reads = AtomicUsize::new(0);
        let search = BlockSearch::new();
        let at = |t| search.block_at(t, chain(&reads));
        assert_eq!(at(1501).await.unwrap().unwrap().number, 250);
        assert_eq!(at(1500).await.unwrap().unwrap().number, 250);
        let cold = reads.load(Ordering::SeqCst);
        assert!(cold <= 14, "a cold search is logarithmic: {cold}");
        reads.store(0, Ordering::SeqCst);
        assert_eq!(at(1501).await.unwrap().unwrap().number, 250);
        assert_eq!(
            reads.load(Ordering::SeqCst),
            0,
            "a repeated search reads nothing"
        );
        assert_eq!(
            at(5000).await.unwrap().unwrap().number,
            1000,
            "past the head: the head"
        );
        assert_eq!(at(1000).await.unwrap().unwrap().number, 0);
        assert_eq!(at(999).await.unwrap(), None, "before the first block");
    }

    #[tokio::test]
    async fn a_failed_read_fails_the_search() {
        let search = BlockSearch::new();
        let failing = |_| std::future::ready(Err::<BlockInfo, _>("down"));
        assert_eq!(search.block_at(10, failing).await, Err("down"));
    }

    #[test]
    fn the_block_answer_is_parsed() {
        assert_eq!(
            block_info(&json!({ "number": "0x10", "timestamp": "0x5f5e100" })).unwrap(),
            Some(BlockInfo {
                number: 16,
                timestamp: 100_000_000
            })
        );
        assert_eq!(block_info(&Value::Null).unwrap(), None);
        assert!(block_info(&json!({ "number": "0x1" })).is_err());
        assert_eq!(
            get_block_params(BlockTag::Number(255)),
            json!(["0xff", false])
        );
    }
}
