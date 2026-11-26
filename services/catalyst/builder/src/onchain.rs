//! Whether the node reads a collections subgraph (`marketplace::graph`, the
//! ONE guarded client with its retry ladder and body cap).

use crate::chain_index::ChainIndex;
use crate::chain_index::graph::any_collections_reader;

pub fn configured(market: Option<&ChainIndex>) -> bool {
    market.is_some_and(any_collections_reader)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn without_a_collections_reader_the_address_reads_are_unavailable() {
        let market = crate::tests::support::market_without_db(
            crate::chain_index::IndexSettings::for_tests(),
        );
        assert!(!configured(Some(&market)));
        assert!(!configured(None));
    }
}
