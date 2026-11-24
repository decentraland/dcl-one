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

/// Why one block could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockError {
    /// The call failed.
    Rpc(RpcError),
    /// The node answered `null`: it does not have the block (yet).
    Missing(BlockTag),
    /// The node answered a block without a usable number or timestamp.
    Shape(String),
}

impl std::fmt::Display for BlockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockError::Rpc(e) => e.fmt(f),
            BlockError::Missing(tag) => write!(f, "no block {}", tag.param()),
            BlockError::Shape(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for BlockError {}

impl From<RpcError> for BlockError {
    fn from(e: RpcError) -> Self {
        BlockError::Rpc(e)
    }
}

/// The block in the `eth_getBlockByNumber` result for `tag`.
pub fn block_info(result: &Value, tag: BlockTag) -> Result<BlockInfo, BlockError> {
    if result.is_null() {
        return Err(BlockError::Missing(tag));
    }
    let quantity = |key: &str| {
        result
            .get(key)
            .and_then(Value::as_str)
            .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .ok_or_else(|| BlockError::Shape(format!("block without a hex {key}")))
    };
    Ok(BlockInfo {
        number: quantity("number")?,
        timestamp: i64::try_from(quantity("timestamp")?)
            .map_err(|_| BlockError::Shape("block timestamp out of range".into()))?,
    })
}

/// The search and the blocks it has read (timestamp to the highest block seen at it).
#[derive(Debug)]
pub struct BlockSearch {
    known: Mutex<BTreeMap<i64, u64>>,
    cap: usize,
}

impl Default for BlockSearch {
    fn default() -> Self {
        Self {
            known: Mutex::default(),
            cap: KNOWN_CAP,
        }
    }
}

impl BlockSearch {
    pub fn new() -> Self {
        Self::default()
    }

    fn remember(&self, block: BlockInfo) {
        let mut known = self.known.lock().unwrap_or_else(|e| e.into_inner());
        if known.len() >= self.cap {
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

    /// Whether `answer`, a [`Self::block_at`] answer for `timestamp`, is final: before the
    /// first block, or the first known block after `timestamp` is its successor. A head-bound
    /// answer is not, even when another search has read past `timestamp` since.
    pub fn is_final(&self, timestamp: i64, answer: Option<BlockInfo>) -> bool {
        let Some(answer) = answer else {
            return true;
        };
        self.bounds(timestamp)
            .1
            .is_some_and(|next| next.number == answer.number + 1)
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
        let found = search.block_at(1501, chain(&reads)).await.unwrap();
        assert!(search.is_final(1501, found), "its successor was read");
        let head = search.block_at(5000, chain(&reads)).await.unwrap();
        assert!(!search.is_final(5000, head), "the head may move");
        assert_eq!(at(1000).await.unwrap().unwrap().number, 0);
        assert_eq!(at(999).await.unwrap(), None, "before the first block");
    }

    /// A head-bound answer stays unsettled after a later search reads past its timestamp.
    #[tokio::test]
    async fn a_head_bound_answer_is_not_final_once_another_search_reads_past_it() {
        let reads = AtomicUsize::new(0);
        let search = BlockSearch::new();
        let head_at = |head: u64| {
            let reads = &reads;
            move |tag| {
                reads.fetch_add(1, Ordering::SeqCst);
                let number = match tag {
                    BlockTag::Latest => head,
                    BlockTag::Number(n) => n.min(head),
                };
                std::future::ready(Ok::<_, ()>(BlockInfo {
                    number,
                    timestamp: 1000 + 2 * number as i64,
                }))
            }
        };
        let stale = search.block_at(1030, head_at(10)).await.unwrap();
        assert_eq!(stale.unwrap().number, 10, "the head then");
        let later = search.block_at(1029, head_at(20)).await.unwrap();
        assert_eq!(later.unwrap().number, 14);
        assert!(!search.is_final(1030, stale), "block 15 is at 1030");
        assert!(search.is_final(1029, later));
        assert!(search.is_final(999, None), "before the first block");
    }

    #[tokio::test]
    async fn a_full_memo_starts_over_and_still_finds_the_block() {
        let reads = AtomicUsize::new(0);
        let search = BlockSearch {
            cap: 8,
            ..BlockSearch::default()
        };
        for t in [1501, 1777, 1203, 1999, 1501] {
            let block = search.block_at(t, chain(&reads)).await.unwrap().unwrap();
            assert_eq!(block.number, ((t - 1000) / 2) as u64, "{t}");
            assert!(search.known.lock().unwrap().len() <= 8);
        }
        reads.store(0, Ordering::SeqCst);
        search.block_at(1999, chain(&reads)).await.unwrap();
        assert!(
            reads.load(Ordering::SeqCst) > 0,
            "the cleared memo reads again"
        );
    }

    #[tokio::test]
    async fn a_failed_read_fails_the_search() {
        let search = BlockSearch::new();
        let failing = |_| std::future::ready(Err::<BlockInfo, _>("down"));
        assert_eq!(search.block_at(10, failing).await, Err("down"));
    }

    #[tokio::test]
    async fn concurrent_searches_share_one_memo_and_each_find_their_block() {
        let reads = AtomicUsize::new(0);
        let search = BlockSearch::new();
        let answers = futures::future::join_all(
            (0..64).map(|i| search.block_at(1000 + 31 * i, chain(&reads))),
        )
        .await;
        for (i, answer) in answers.into_iter().enumerate() {
            let expected = (31 * i as u64) / 2;
            assert_eq!(answer.unwrap().unwrap().number, expected, "search {i}");
        }
        reads.store(0, Ordering::SeqCst);
        assert_eq!(
            search
                .block_at(1000 + 31 * 7, chain(&reads))
                .await
                .unwrap()
                .unwrap()
                .number,
            108
        );
        assert_eq!(reads.load(Ordering::SeqCst), 0, "answered from the memo");
    }

    #[test]
    fn the_block_answer_is_parsed() {
        let tag = BlockTag::Number(255);
        assert_eq!(
            block_info(&json!({ "number": "0x10", "timestamp": "0x5f5e100" }), tag).unwrap(),
            BlockInfo {
                number: 16,
                timestamp: 100_000_000
            }
        );
        assert_eq!(block_info(&Value::Null, tag), Err(BlockError::Missing(tag)));
        for bad in [
            json!({ "number": "0x1" }),
            json!({ "timestamp": "0x1" }),
            json!({ "number": "0x1", "timestamp": "soon" }),
            json!({ "number": "0x1", "timestamp": 1 }),
            json!({ "number": "0x1", "timestamp": "0xffffffffffffffff" }),
        ] {
            assert!(
                matches!(block_info(&bad, tag), Err(BlockError::Shape(_))),
                "{bad}"
            );
        }
        assert_eq!(BlockError::Missing(tag).to_string(), "no block 0xff");
        assert_eq!(
            BlockError::from(RpcError::Status(502)),
            BlockError::Rpc(RpcError::Status(502))
        );
        assert_eq!(get_block_params(tag), json!(["0xff", false]));
    }
}
