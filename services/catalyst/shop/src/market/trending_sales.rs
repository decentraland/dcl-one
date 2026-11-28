//! The trending signal: each network's newest sales of collection items,
//! read from the published sales history (`shop.sale_execution`,
//! `shop.sale_asset`) on the caller's own snapshot.
//!
//! The rows are those the collections subgraphs' `sales` held, read their
//! way: a sale of a catalogue item (no LAND, estate or NAME) at a positive
//! MANA price, through the CollectionStore (`mint`), a legacy marketplace
//! order or a bid; the window starts after midnight UTC `days` ago; each
//! network gives its newest [`TRENDING_SALES_LIMIT`], latest log first
//! within a block, Ethereum's before Polygon's. The off-chain marketplace's
//! trades, which those subgraphs never indexed, are left out.
//!
//! An Ethereum v1 resale carries a token ID, not an item ID, in the ledger.
//! Its item is the chain-1 catalogue item named by the token's latest
//! `Issue` at or before the sale (the collection's URN and the Issue's
//! wearable ID), as the historical holdings read it; a sale whose item
//! that names none is not counted.
//!
//! A route checks the publications' deadline again when it answers
//! ([`Published::ensure_fresh`]), after whatever it read on the signal.
//!
//! That is the signal of a node that serves the mainnet sales index
//! ([`native`]). Elsewhere (a testnet, or the index off) the routes keep the
//! collections subgraphs' `sales` ([`subgraph_sales`]: both readers needed,
//! 10 min, the same window, cap and order), read with no connection held
//! before their page's read, as before.

use axum::response::Response;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::PgConnection;

use super::chain_walk::ChainNet;
use super::{MarketplaceState, listings, readers, sale_fold, sale_reads};

/// The rows each network gives at most, newest first.
pub const TRENDING_SALES_LIMIT: i64 = 1000;

/// The networks the signal reads, in the order their rows come.
const NETWORKS: [ChainNet; 2] = [ChainNet::Ethereum, ChainNet::Matic];

/// One sale of the signal: the item, what it sold for in wei of MANA, and
/// the item's creator in the catalogue.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SignalSale {
    pub contract_address: String,
    pub item_id: String,
    pub price: String,
    pub creator: Option<String>,
}

impl SignalSale {
    /// The price in wei; a value past `u128` counts as nothing, as the
    /// subgraph rows' did.
    pub fn wei(&self) -> u128 {
        self.price.parse().unwrap_or(0)
    }
}

const RECENT_SALES: &str = "SELECT a.contract_address, coalesce(a.item_id, v1.item_id)::text AS item_id, \
         a.price::text AS price, coalesce(i.creator, v1.creator) AS creator \
     FROM shop.sale_execution e \
     JOIN shop.sale_asset a USING (network, block_number, log_index) \
     LEFT JOIN shop.items i ON (i.network, i.contract_address, i.item_id) = (a.network, a.contract_address, a.item_id) \
       AND i.chain_id = $5 \
     LEFT JOIN LATERAL ( \
         SELECT issue.detail FROM shop.nft_event issue \
         WHERE a.item_id IS NULL AND a.token_id IS NOT NULL \
           AND (issue.network, issue.contract_address, issue.token_id) = (a.network, a.contract_address, a.token_id) \
           AND issue.kind = 'issue_v1' AND (issue.block_number, issue.log_index) <= (a.block_number, a.log_index) \
         ORDER BY issue.block_number DESC, issue.log_index DESC LIMIT 1 \
     ) issue ON true \
     LEFT JOIN shop.collections co ON issue.detail IS NOT NULL \
       AND (co.network, co.contract_address) = (a.network, a.contract_address) AND co.chain_id = $5 \
     LEFT JOIN shop.items v1 ON (v1.network, v1.contract_address) = (a.network, a.contract_address) \
       AND v1.chain_id = $5 AND v1.urn = co.urn || ':' || issue.detail \
     WHERE e.network = $1 AND e.block_number <= $2 AND e.block_time > $3 \
       AND e.kind IN ('store', 'order', 'bid') \
       AND coalesce(a.item_id, v1.item_id) IS NOT NULL AND a.price > 0 AND a.currency = $4 \
     ORDER BY e.block_time DESC, e.block_number DESC, e.log_index DESC, a.ordinal DESC \
     LIMIT $6";

/// Both networks' sales publications, as the request's snapshot took them.
#[derive(Debug, Clone)]
pub struct Published {
    nets: Vec<(ChainNet, sale_fold::Published)>,
    deadline: DateTime<Utc>,
}

impl Published {
    /// The published histories as this snapshot took them, for a memo of
    /// what they rank: each network's height and generation, so neither an
    /// advance nor a replaced history serves a ranking of other sales.
    pub fn key(&self) -> String {
        self.nets
            .iter()
            .map(|(net, p)| format!("{}={}@{}", net.db_name(), p.generation, p.through))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// `503 sales-index` once a publication this snapshot took has expired:
    /// a route checks it when it answers, after everything it read on the
    /// signal, a memoized answer included.
    pub fn ensure_fresh(&self) -> Result<(), Response> {
        sale_reads::ensure_fresh(self.deadline)
    }

    /// The sales of the last `days` (from midnight UTC) within these
    /// publications, read on `c`, the snapshot that took them
    /// ([`Self::sales_since`]).
    pub async fn sales(
        &self,
        c: &mut PgConnection,
        days: i64,
    ) -> Result<Vec<SignalSale>, Response> {
        self.sales_since(c, super::items::midnight_days_ago(days))
            .await
    }

    /// The sales after `from` (unix seconds) within these publications, read
    /// on `c`, the snapshot that took them. A publication that expires
    /// during the read answers `503 sales-index`.
    pub async fn sales_since(
        &self,
        c: &mut PgConnection,
        from: i64,
    ) -> Result<Vec<SignalSale>, Response> {
        let mut out = Vec::new();
        for (net, published) in &self.nets {
            let rows: Vec<SignalSale> = sqlx::query_as(RECENT_SALES)
                .bind(net.db_name())
                .bind(published.through)
                .bind(from)
                .bind(listings::mana_token(net.chain_id()))
                .bind(net.chain_id())
                .bind(TRENDING_SALES_LIMIT)
                .fetch_all(&mut *c)
                .await
                .map_err(readers::index_read_error)?;
            out.extend(rows);
        }
        self.ensure_fresh()?;
        Ok(out)
    }
}

/// Both networks' sales publications on `c`, the request's snapshot. A
/// disabled index, or a network without a valid, fresh publication, answers
/// `503 sales-index`, as `/v1/sales` does.
pub async fn published(
    state: &MarketplaceState,
    c: &mut PgConnection,
) -> Result<Published, Response> {
    sale_reads::require_enabled(state)?;
    let now = Utc::now();
    let mut nets = Vec::with_capacity(NETWORKS.len());
    for net in NETWORKS {
        let published = sale_fold::published(c, net, now)
            .await
            .map_err(published_error)?
            .ok_or_else(sale_reads::no_fresh_publication)?;
        nets.push((net, published));
    }
    let deadline = nets.iter().map(|(_, p)| p.fresh_until).min().unwrap_or(now);
    let published = Published { nets, deadline };
    // The clock was read before the publication's statements, which may
    // have waited.
    published.ensure_fresh()?;
    Ok(published)
}

/// The signal's sales of the last `days` on `c`, with the publications to
/// check again when the route answers: [`published`], then
/// [`Published::sales`].
pub async fn recent(
    state: &MarketplaceState,
    c: &mut PgConnection,
    days: i64,
) -> Result<(Published, Vec<SignalSale>), Response> {
    let published = published(state, c).await?;
    let sales = published.sales(c, days).await?;
    Ok((published, sales))
}

/// Whether the signal comes from the published sales history on this node:
/// where it serves the mainnet sales index. Elsewhere it is
/// [`subgraph_sales`].
pub fn native(state: &MarketplaceState) -> bool {
    sale_reads::enabled(state)
}

/// `503 sales-index` once `published`, the native signal's publications,
/// has expired; nothing to check for the subgraph signal.
pub fn ensure_fresh(published: Option<&Published>) -> Result<(), Response> {
    published.map_or(Ok(()), Published::ensure_fresh)
}

const SUBGRAPH_SALES: &str = r#"
query recentSales($from: BigInt!, $first: Int!) {
  sales(first: $first, orderBy: timestamp, orderDirection: desc, where: { timestamp_gt: $from }) {
    id
    searchItemId
    searchContractAddress
    price
    timestamp
    type
    seller
    buyer
    item { id creator }
  }
}"#;

/// The signal of a node without the mainnet sales index: the last `days`
/// of `sales` on both collections subgraphs (each required), at most
/// [`TRENDING_SALES_LIMIT`] per network, memoized 10 min. A row keeps an
/// empty contract or item when the subgraph has none; the readers skip
/// those as they did. An external call: made with no connection held.
pub async fn subgraph_sales(
    state: &MarketplaceState,
    days: i64,
) -> Result<Vec<SignalSale>, Response> {
    let urls = readers::require_both_collections(state)?;
    let from = super::items::midnight_days_ago(days);
    let mut out = Vec::new();
    for (network, url) in urls {
        let key = format!("sales:{}:{from}", network.db_name());
        let data = readers::cached(state, readers::Ttl::M10, key, || async {
            readers::query(
                state,
                url,
                SUBGRAPH_SALES,
                json!({ "from": from.to_string(), "first": TRENDING_SALES_LIMIT }),
            )
            .await
        })
        .await
        .map_err(|e| readers::upstream_failed("/v1/trendings", &e))?;
        out.extend(readers::rows(&data, "sales").iter().map(|sale| {
            SignalSale {
                contract_address: readers::str_of(sale, "searchContractAddress").to_lowercase(),
                item_id: readers::str_of(sale, "searchItemId"),
                price: readers::str_of(sale, "price"),
                creator: readers::at(sale, "item/creator")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            }
        }));
    }
    Ok(out)
}

fn published_error(error: anyhow::Error) -> Response {
    match error.downcast::<sqlx::Error>() {
        Ok(e) => readers::index_read_error(e),
        Err(error) => {
            tracing::error!(%error, "reading the sales publication");
            super::internal_server_error()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use axum::http::StatusCode;
    use chrono::Utc;
    use serde_json::Value;
    use sqlx::PgPool;

    use super::super::chain_walk::ChainNet;
    use super::super::sale_fold::test_support::{Sale, insert, publish};
    use super::super::{MarketplaceState, Settings};
    use super::*;

    const BUYER: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const SELLER: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn state(pool: &PgPool, sales_enabled: bool) -> Arc<MarketplaceState> {
        let mut settings = Settings::for_tests();
        settings.market.indexes.sales_enabled = sales_enabled;
        Arc::new(MarketplaceState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            settings,
        ))
    }

    async fn add_item(pool: &PgPool, net: ChainNet, contract: &str, item: &str, creator: &str) {
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, \
                 total_supply, max_supply, available, price, image, name, category, body_shapes, \
                 search_is_collection_approved, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ($1, $2, $3::text::numeric, $4, 'wearable_v2', $5, $6, 'rare', 1, 100, 99, 1, 'https://img/x', \
                 'item', 'hat', ARRAY['BaseMale'], true, 1, 1, 1, timezone('UTC', now())) \
             ON CONFLICT DO NOTHING",
        )
        .bind(net.db_name())
        .bind(contract)
        .bind(item)
        .bind(net.chain_id())
        .bind(format!("urn:{contract}:{item}"))
        .bind(creator)
        .execute(pool)
        .await
        .unwrap();
    }

    fn sale(
        net: ChainNet,
        block: i64,
        log: i32,
        time: i64,
        execution: &'static str,
        item: &str,
    ) -> Sale {
        Sale {
            net,
            block,
            log,
            ordinal: 0,
            time,
            execution,
            kind: super::super::sale_fold::test_support::asset_kind(execution),
            contract: "0x00000000000000000000000000000000000000c1".into(),
            item_id: Some(item.into()),
            token_id: Some(format!("{block}{log}")),
            price: "1000000000000000000".into(),
            currency: None,
            buyer: BUYER.into(),
            seller: Some(SELLER.into()),
        }
    }

    /// The signal after `from`, an anchor the test computed once.
    async fn read(
        state: &MarketplaceState,
        pool: &PgPool,
        from: i64,
    ) -> Result<Vec<SignalSale>, Response> {
        let mut c = pool.acquire().await.unwrap();
        published(state, &mut c)
            .await?
            .sales_since(&mut c, from)
            .await
    }

    fn week() -> i64 {
        super::super::items::midnight_days_ago(7)
    }

    async fn refusal(r: Response) -> (StatusCode, Value) {
        let code = r.status();
        let bytes = axum::body::to_bytes(r.into_body(), usize::MAX)
            .await
            .unwrap();
        (code, serde_json::from_slice(&bytes).unwrap())
    }

    /// The captured `sales` of the collections subgraphs (the newest 1,000
    /// on Polygon and 200 on Ethereum), written as the fold writes them
    /// (`trending_parity_tests::insert_fixture_sales`: an Ethereum v1 resale
    /// with no item ID, its token's Issue naming the item) over the seeded
    /// catalogue: for each window, the signal is the rows the subgraph query
    /// returned for it, in its order, a tie on the timestamp latest log first.
    #[tokio::test]
    async fn the_signal_is_the_subgraphs_sales_on_captured_fixtures() {
        use super::super::trending_parity_tests::{
            fixture_sales, insert_fixture_sales, seed_catalogue,
        };
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let sales = fixture_sales();
        seed_catalogue(&pool).await;
        let top = insert_fixture_sales(&pool, &sales).await;
        for net in ChainNet::ALL {
            assert_eq!(publish(&pool, net, top).await, top);
        }
        let state = state(&pool, true);
        for days in [1, 7, 30, 90, 365] {
            let from = super::super::items::midnight_days_ago(days);
            let expected: Vec<SignalSale> = [ChainNet::Ethereum, ChainNet::Matic]
                .into_iter()
                .flat_map(|net| {
                    sales
                        .iter()
                        .filter(move |s| s.net == net && s.time > from)
                        .take(1000)
                        .map(|s| SignalSale {
                            contract_address: s.contract.clone(),
                            item_id: s.item.clone(),
                            price: s.price.clone(),
                            creator: Some(s.creator.clone()),
                        })
                })
                .collect();
            if days >= 90 {
                assert!(
                    sales
                        .iter()
                        .any(|s| s.net == ChainNet::Ethereum && s.time > from),
                    "{days} days hold Ethereum v1 resales"
                );
            }
            let got = read(&state, &pool, from).await.unwrap();
            assert_eq!(got.len(), expected.len(), "{days} days");
            assert_eq!(got, expected, "{days} days");
        }
        pool.close().await;
    }

    /// Only what the collections subgraphs held: a trade or a NAME fee, a
    /// zero or non-MANA price, a sale with no item (LAND), a sale past the
    /// published block and one at the window's start are left out; a primary
    /// mint, an order and a bid count, each network's under its MANA.
    #[tokio::test]
    async fn the_signal_takes_only_what_the_subgraphs_held() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let from = super::super::items::midnight_days_ago(7);
        let now = Utc::now().timestamp() - 60;
        let counted = [
            sale(ChainNet::Matic, 1, 0, now, "store", "1"),
            sale(ChainNet::Matic, 2, 0, now - 10, "order", "2"),
            sale(ChainNet::Matic, 3, 0, now - 20, "bid", "3"),
            sale(ChainNet::Matic, 4, 0, from + 1, "store", "4"),
            sale(ChainNet::Ethereum, 1, 0, now, "order", "5"),
        ];
        let mut trade = sale(ChainNet::Matic, 5, 0, now, "trade", "6");
        trade.kind = "mint";
        let mut zero = sale(ChainNet::Matic, 6, 0, now, "store", "7");
        zero.price = "0".into();
        let mut foreign = sale(ChainNet::Matic, 7, 0, now, "order", "8");
        foreign.currency = Some("0x0000000000000000000000000000000000000bad".into());
        let mut land = sale(ChainNet::Ethereum, 2, 0, now, "order", "9");
        land.item_id = None;
        let mut eth_mana_on_polygon = sale(ChainNet::Matic, 8, 0, now, "order", "10");
        eth_mana_on_polygon.currency = listings::mana_token(1).map(str::to_string);
        let left_out = [
            trade,
            sale(ChainNet::Ethereum, 3, 0, now, "name_fee", "11"),
            zero,
            foreign,
            land,
            eth_mana_on_polygon,
            sale(ChainNet::Matic, 9, 0, from, "store", "12"),
            sale(ChainNet::Matic, 101, 0, now, "store", "13"),
        ];
        for s in counted.iter().chain(&left_out) {
            insert(&pool, s).await;
        }
        for net in ChainNet::ALL {
            publish(&pool, net, 100).await;
        }
        add_item(&pool, ChainNet::Matic, &counted[0].contract, "1", SELLER).await;
        let got = read(&state(&pool, true), &pool, from).await.unwrap();
        let items: Vec<&str> = got.iter().map(|s| s.item_id.as_str()).collect();
        assert_eq!(items, ["5", "1", "2", "3", "4"]);
        assert_eq!(
            got[1].creator.as_deref(),
            Some(SELLER),
            "the catalogue's creator"
        );
        assert_eq!(got[0].creator, None, "an item the catalogue lacks has none");
        pool.close().await;
    }

    /// An Ethereum v1 resale counts for the chain-1 catalogue item its
    /// token's latest Issue at or before the sale names (the collection's
    /// URN and the Issue's wearable ID), with that item's creator. A later
    /// Issue does not change it; an Issue naming no catalogue item, or a
    /// token with no Issue (LAND, NAME), is not counted.
    #[tokio::test]
    async fn an_ethereum_v1_resale_counts_for_the_item_its_issue_names() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let contract = "0x00000000000000000000000000000000000000e1";
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, \
                 is_approved, is_editable, minters, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('ethereum', $1, 1, 'Vintage', $2, $2, 'urn:decentraland:ethereum:collections-v1:vintage', true, true, \
                 false, ARRAY[]::text[], 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(contract)
        .bind(SELLER)
        .execute(&pool)
        .await
        .unwrap();
        for (item, wearable, creator) in [("7", "hat", BUYER), ("8", "cap", SELLER)] {
            add_item(&pool, ChainNet::Ethereum, contract, item, creator).await;
            sqlx::query(
                "UPDATE shop.items SET item_type = 'wearable_v1', \
                     urn = 'urn:decentraland:ethereum:collections-v1:vintage:' || $3 \
                 WHERE network = 'ethereum' AND contract_address = $1 AND item_id = $2::text::numeric",
            )
            .bind(contract)
            .bind(item)
            .bind(wearable)
            .execute(&pool)
            .await
            .unwrap();
        }
        // Token 1 was issued as the hat; token 2 as the cap, and again as
        // the hat after the sale; token 3 as a wearable the catalogue lacks;
        // token 4 never.
        for (block, token, wearable) in
            [(1, 1, "hat"), (1, 2, "cap"), (9, 2, "hat"), (1, 3, "gone")]
        {
            sqlx::query(
                "INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, \
                     contract_address, token_id, from_address, to_address, amount, detail, tx_hash) \
                 VALUES ('ethereum', $1, $2, 1, 'issue_v1', $3, $3, $2, NULL, $4, $2, $5, NULL)",
            )
            .bind(block)
            .bind(token)
            .bind(contract)
            .bind(SELLER)
            .bind(wearable)
            .execute(&pool)
            .await
            .unwrap();
        }
        let from = week();
        let now = Utc::now().timestamp() - 60;
        for token in 1..=4 {
            let mut resale = sale(ChainNet::Ethereum, 5, token, now, "order", "0");
            resale.contract = contract.into();
            resale.item_id = None;
            resale.token_id = Some(token.to_string());
            insert(&pool, &resale).await;
        }
        for net in ChainNet::ALL {
            assert_eq!(publish(&pool, net, 100).await, 100);
        }
        let got = read(&state(&pool, true), &pool, from).await.unwrap();
        let rows: Vec<(&str, Option<&str>)> = got
            .iter()
            .map(|s| (s.item_id.as_str(), s.creator.as_deref()))
            .collect();
        assert_eq!(
            rows,
            [("8", Some(SELLER)), ("7", Some(BUYER))],
            "latest log first"
        );
        pool.close().await;
    }

    /// Each network gives its newest 1,000, Ethereum's first; a block's
    /// sales come latest log first, a log's assets latest first.
    #[tokio::test]
    async fn each_network_gives_its_newest_thousand() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let from = week();
        let now = Utc::now().timestamp() - 60;
        let mut rows = Vec::new();
        for i in 0..1001i64 {
            rows.push(sale(
                ChainNet::Matic,
                1 + i,
                0,
                now - 1001 + i,
                "store",
                &i.to_string(),
            ));
        }
        // Three sales at the newest time: two logs of one block, the later
        // with two assets.
        rows.push(sale(ChainNet::Ethereum, 7, 1, now, "order", "9001"));
        let mut second = sale(ChainNet::Ethereum, 7, 2, now, "order", "9002");
        rows.push(second.clone());
        second.ordinal = 1;
        second.item_id = Some("9003".into());
        second.token_id = Some("73".into());
        rows.push(second);
        rows.push(sale(ChainNet::Ethereum, 6, 0, now, "order", "9000"));
        for s in &rows {
            insert(&pool, s).await;
        }
        assert_eq!(publish(&pool, ChainNet::Matic, 2000).await, 2000);
        assert_eq!(publish(&pool, ChainNet::Ethereum, 2000).await, 2000);
        let got = read(&state(&pool, true), &pool, from).await.unwrap();
        let items: Vec<&str> = got.iter().map(|s| s.item_id.as_str()).collect();
        assert_eq!(&items[..4], ["9003", "9002", "9001", "9000"]);
        assert_eq!(items.len(), 4 + 1000);
        assert_eq!(items[4], "1000", "the newest Polygon sale");
        assert_eq!(items[4 + 999], "1", "the oldest of the newest 1,000");
        let distinct: BTreeSet<&str> = items.iter().copied().collect();
        assert_eq!(distinct.len(), items.len());
        pool.close().await;
    }

    /// A disabled index, a network without a publication, a stale one and a
    /// publication that expires during the read answer `503 sales-index`.
    #[tokio::test]
    async fn the_signal_needs_both_networks_fresh_publications() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let now = Utc::now().timestamp() - 60;
        insert(&pool, &sale(ChainNet::Matic, 1, 0, now, "store", "1")).await;
        let enabled = state(&pool, true);
        let refused = |r: Result<Vec<SignalSale>, Response>| async move {
            let (code, body) = refusal(r.unwrap_err()).await;
            assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{body}");
            assert_eq!(body["source"], "sales-index", "{body}");
            body["message"].as_str().unwrap().to_string()
        };
        assert_eq!(
            refused(read(&enabled, &pool, week()).await).await,
            "the requested sales history has no fresh publication"
        );
        publish(&pool, ChainNet::Matic, 100).await;
        assert_eq!(
            refused(read(&enabled, &pool, week()).await).await,
            "the requested sales history has no fresh publication",
            "Polygon alone"
        );
        publish(&pool, ChainNet::Ethereum, 100).await;
        assert_eq!(read(&enabled, &pool, week()).await.unwrap().len(), 1);
        assert_eq!(
            refused(read(&state(&pool, false), &pool, week()).await).await,
            "the mainnet sales index is disabled"
        );
        let mut c = pool.acquire().await.unwrap();
        let mut published = published(&enabled, &mut c).await.unwrap();
        published.deadline = Utc::now() - chrono::Duration::seconds(1);
        assert_eq!(
            refused(published.sales(&mut c, 7).await).await,
            "the sales publication expired during the read"
        );
        drop(c);
        sqlx::query(
            "UPDATE shop.chain_cursor SET folded_at = timezone('UTC', now()) - interval '31 minutes' \
             WHERE network = 'ethereum' AND indexer = 'sales'",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            refused(read(&enabled, &pool, week()).await).await,
            "the requested sales history has no fresh publication",
            "a stale Ethereum input"
        );
        pool.close().await;
    }
}
