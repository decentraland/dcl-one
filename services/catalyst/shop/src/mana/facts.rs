// from: shop@94584a5c:app/src/lib/api.ts (fetchTrade, fetchClassicItemOrders, fetchSecondarySaleState: which record a cart line names), marketplace@f3956826:webapp/src/modules/{order,rental,ens}/sagas.ts (what an order, a rental and a NAME claim read before paying), SHOP.md "Enforce payment eligibility at every entry"
//! What a cart line offers, from the authoritative record it names and
//! nothing else: a stored signed trade (`shop.trades`), a legacy order the
//! nft index folded (`shop.nft_order`), a rentals listing this node stores
//! (`signatures.rentals`) or a NAME label against the registrar. Each
//! resolves to the rule's [`Offer`] and the [`Facts`] it is judged on; the
//! contract role, the item and the token come from the static registry, the
//! mirrors and the indexes, and read [`Lookup::Unknown`] when their source
//! has no complete pass, so the rule answers `Incomplete` rather than a
//! narrower choice. A client-supplied `primary`, price or sale kind has no
//! field to arrive through.

use alloy_primitives::{Address, U256};
use sqlx::{PgPool, Row};

use super::venues::{Book, Chain};
use super::wire::{LineRequest, ListingRef};
use crate::checkout::trade::TradeIn;
use crate::market::chain_walk::{self, ChainNet, INDEXER_NFT};
use crate::market::trades;
use crate::policy::Facts;
use crate::rentals::calls::{RentTerms, rent_terms};
use crate::rentals::types::RentalListing;
use crate::types::{
    AssetId, ChainId, ContractRef, ContractRole, EvmAddress, ItemKind, ItemRecord, Lookup, ManaWei,
    NameLabel, Offer, Session, SignedTrade, TokenRecord, TokenRef, TradeAsset, TradeAssetType,
};

/// A legacy order as the nft index folded it (`shop.nft_order`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyOrder {
    pub order_id: String,
    pub marketplace: Address,
    pub status: String,
    pub seller: Address,
    pub price: U256,
    /// Milliseconds.
    pub expires_at: i64,
    pub updated_block: i64,
}

/// The record a line resolved to, kept beside the offer for the
/// verification and the calldata.
#[derive(Debug, Clone)]
pub enum Listing {
    Store {
        venue: Address,
        price: ManaWei,
    },
    Trade {
        trade: TradeIn,
        venue: Address,
        hashed_signature: String,
        /// The trade-events verdict (`open` | `sold` | `cancelled`) and its
        /// executions, when the index is published; None when it is not.
        verdict: Option<(String, i32)>,
        index_ready: bool,
    },
    LegacyOrder {
        order: LegacyOrder,
        index_ready: bool,
    },
    Rental {
        listing: RentalListing,
        terms: RentTerms,
        rentals: Address,
        status_index_ready: bool,
    },
    Name {
        label: NameLabel,
        registrar: Address,
        controller: Address,
    },
}

impl Listing {
    pub fn source(&self) -> &'static str {
        match self {
            Self::Store { .. } => "items-mirror",
            Self::Trade { .. } => "trades-mirror+trade-events-index",
            Self::LegacyOrder { .. } => "nft-index",
            Self::Rental { .. } => "rentals-store+rentals-index",
            Self::Name { .. } => "name-registrar",
        }
    }

    pub fn index_ready(&self) -> bool {
        match self {
            Self::Trade { index_ready, .. } | Self::LegacyOrder { index_ready, .. } => *index_ready,
            Self::Rental {
                status_index_ready, ..
            } => *status_index_ready,
            Self::Store { .. } | Self::Name { .. } => true,
        }
    }

    pub fn verdict(&self) -> Option<String> {
        match self {
            Self::Trade { verdict, .. } => verdict.as_ref().map(|(s, _)| s.clone()),
            Self::LegacyOrder { order, .. } => Some(order.status.clone()),
            Self::Rental { listing, .. } => Some(listing.status.clone()),
            Self::Store { .. } | Self::Name { .. } => None,
        }
    }
}

/// One resolved line.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub chain: Chain,
    pub asset: AssetId,
    pub offer: Offer,
    pub facts: Facts,
    pub listing: Listing,
    /// Remaining mintable supply of a primary item (the items mirror).
    pub available: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    #[error("a line names an asset or a NAME")]
    NoAsset,
    #[error("chain {0} is not one the Shop settles on")]
    UnknownChain(ChainId),
    #[error("a line names one listing: tradeId, orderId, rentalId, name or acquisition=store")]
    Ambiguous,
    #[error(
        "a line names the listing it buys (tradeId, orderId, rentalId, name or acquisition=store)"
    )]
    NoListing,
    #[error("{0} is not an approved item offered by the CollectionStore")]
    StoreUnavailable(AssetId),
    #[error("trade {0} is not known to this node")]
    TradeNotFound(String),
    #[error("trade {id} is on chain {trade_chain}, the line on {line_chain}")]
    TradeChain {
        id: String,
        trade_chain: u64,
        line_chain: u64,
    },
    #[error(
        "trade {0} was signed for a contract that is not an off-chain marketplace of its chain"
    )]
    TradeVenue(String),
    #[error("trade {0} does not sell the line's asset")]
    TradeAsset(String),
    #[error("trade {0} is malformed: {1}")]
    TradeShape(String, String),
    #[error("{0} has no legacy order the nft index knows")]
    OrderNotFound(AssetId),
    #[error("the order named is {named}, the token's current one {current}")]
    OrderMismatch { named: String, current: String },
    #[error("rental listing {0} is not known to this node")]
    RentalNotFound(String),
    #[error("rental listing {0} is for another asset")]
    RentalAsset(String),
    #[error("the rent terms: {0}")]
    Rent(#[from] crate::rentals::calls::RentError),
    #[error("the chain has no {0}")]
    NoVenue(&'static str),
    #[error("{0}")]
    Name(String),
    #[error("database: {0}")]
    Db(String),
    /// The source that holds the listing is not published yet; the route
    /// answers the unavailable envelope naming it.
    #[error("{index} is not published yet")]
    IndexNotReady { index: &'static str },
}

impl From<sqlx::Error> for ResolveError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e.to_string())
    }
}

fn evm(a: Address) -> EvmAddress {
    EvmAddress::new(a)
}

fn item_kind(item_type: &str) -> ItemKind {
    if item_type.starts_with("emote") {
        ItemKind::Emote
    } else {
        ItemKind::Wearable
    }
}

/// The contract's role: the static registry first, then the collections
/// mirror (only after a complete pass, else `Unknown`).
pub async fn contract_role(
    pool: &PgPool,
    chain: &Chain,
    contract: Address,
) -> Result<Lookup<ContractRole>, sqlx::Error> {
    if chain.is_land_registry(contract) {
        return Ok(Lookup::Found(ContractRole::Land));
    }
    if chain.is_estate_registry(contract) {
        return Ok(Lookup::Found(ContractRole::Estate));
    }
    if chain.name_registrar == Some(contract) {
        return Ok(Lookup::Found(ContractRole::NameRegistrar));
    }
    let Some(net) = chain.net else {
        return Ok(Lookup::Absent);
    };
    let row = sqlx::query(
        "SELECT (SELECT last_complete_pass_at IS NOT NULL FROM shop.mirror_state WHERE mirror = 'collections' AND network = $1) AS ready, \
                EXISTS (SELECT 1 FROM shop.collections WHERE network = $1 AND contract_address = $2) AS is_collection",
    )
    .bind(net.db_name())
    .bind(format!("{contract:#x}"))
    .fetch_one(pool)
    .await?;
    let ready: Option<bool> = row.get("ready");
    let is_collection: bool = row.get("is_collection");
    Ok(match ready {
        Some(true) if is_collection => Lookup::Found(ContractRole::Collection),
        Some(true) => Lookup::Absent,
        _ => Lookup::Unknown,
    })
}

/// The items mirror's record of `(collection, item_id)` and its remaining
/// supply; `Unknown` before the mirror's first complete pass.
pub async fn item_record(
    pool: &PgPool,
    chain: &Chain,
    collection: Address,
    item_id: U256,
) -> Result<(Lookup<ItemRecord>, Option<i64>), sqlx::Error> {
    let Some(net) = chain.net else {
        return Ok((Lookup::Absent, None));
    };
    let row = sqlx::query(
        "SELECT (SELECT last_complete_pass_at IS NOT NULL FROM shop.mirror_state WHERE mirror = 'items' AND network = $1) AS ready, \
                i.item_type, i.creator, i.available::text AS available \
         FROM (SELECT 1) AS one LEFT JOIN shop.items i ON i.network = $1 AND i.contract_address = $2 AND i.item_id = $3::numeric",
    )
    .bind(net.db_name())
    .bind(format!("{collection:#x}"))
    .bind(item_id.to_string())
    .fetch_one(pool)
    .await?;
    let ready: Option<bool> = row.get("ready");
    let item_type: Option<String> = row.get("item_type");
    let creator: Option<String> = row.get("creator");
    let available: Option<String> = row.get("available");
    Ok(match (ready, item_type) {
        (Some(true), Some(item_type)) => match creator
            .as_deref()
            .and_then(|c| EvmAddress::parse(&c.to_ascii_lowercase()).ok())
        {
            Some(creator) => (
                Lookup::Found(ItemRecord {
                    chain_id: chain.chain_id,
                    collection: evm(collection),
                    item_id,
                    kind: item_kind(&item_type),
                    creator,
                }),
                available.and_then(|a| a.parse::<i64>().ok()),
            ),
            None => (Lookup::Unknown, None),
        },
        (Some(true), None) => (Lookup::Absent, None),
        _ => (Lookup::Unknown, None),
    })
}

/// The indexed token (its owner and item): `shop.nft` for collections and
/// names, `land_authz.token_right` for parcels and estates, each only while
/// its index is published; `Unknown` otherwise.
pub async fn token_record(
    pool: &PgPool,
    chain: &Chain,
    role: &Lookup<ContractRole>,
    contract: Address,
    token_id: U256,
) -> Result<Lookup<TokenRecord>, sqlx::Error> {
    let asset = AssetId::token(chain.chain_id, evm(contract), token_id);
    let now = chrono::Utc::now();
    match role {
        Lookup::Found(ContractRole::Land) | Lookup::Found(ContractRole::Estate) => {
            if !crate::market::nft_reads::land_ready(pool).await {
                return Ok(Lookup::Unknown);
            }
            let owner: Option<Option<String>> = sqlx::query_scalar("SELECT owner FROM land_authz.token_right WHERE token_address = $1 AND token_id = $2::numeric")
                .bind(format!("{contract:#x}"))
                .bind(token_id.to_string())
                .fetch_optional(pool)
                .await?;
            Ok(
                match owner.flatten().and_then(|o| EvmAddress::parse(&o).ok()) {
                    Some(owner) => Lookup::Found(TokenRecord {
                        asset,
                        item_id: None,
                        owner,
                    }),
                    None => Lookup::Absent,
                },
            )
        }
        _ => {
            let Some(net) = chain.net else {
                return Ok(Lookup::Unknown);
            };
            if chain_walk::ready_cursor(pool, INDEXER_NFT, net, now)
                .await
                .is_none()
            {
                return Ok(Lookup::Unknown);
            }
            let row = sqlx::query("SELECT owner, item_id::text AS item_id FROM shop.nft WHERE network = $1 AND contract_address = $2 AND token_id = $3::numeric")
                .bind(net.db_name())
                .bind(format!("{contract:#x}"))
                .bind(token_id.to_string())
                .fetch_optional(pool)
                .await?;
            Ok(match row {
                Some(r) => match r
                    .get::<Option<String>, _>("owner")
                    .and_then(|o| EvmAddress::parse(&o).ok())
                {
                    Some(owner) => Lookup::Found(TokenRecord {
                        asset,
                        item_id: r
                            .get::<Option<String>, _>("item_id")
                            .and_then(|i| U256::from_str_radix(&i, 10).ok()),
                        owner,
                    }),
                    None => Lookup::Absent,
                },
                None => Lookup::Absent,
            })
        }
    }
}

fn trade_asset(a: &crate::checkout::trade::TradeAssetIn) -> Result<TradeAsset, String> {
    let asset_type = TradeAssetType::from_code(u64::try_from(a.asset_type).unwrap_or(u64::MAX));
    let value = match asset_type {
        TradeAssetType::Erc20 | TradeAssetType::UsdPeggedMana => a.amount.as_deref(),
        TradeAssetType::Erc721 => a.token_id.as_deref(),
        TradeAssetType::CollectionItem => a.item_id.as_deref(),
        TradeAssetType::Unsupported(code) => return Err(format!("unsupported asset type {code}")),
    }
    .ok_or("an asset without its value")?;
    Ok(TradeAsset {
        asset_type,
        contract: EvmAddress::parse(&a.contract_address.trim().to_ascii_lowercase())
            .map_err(|e| format!("asset contract: {e}"))?,
        value: crate::types::parse_decimal(value.trim())
            .map_err(|e| format!("asset value: {e}"))?,
    })
}

/// A stored trade as the rule's signed trade.
pub fn signed_trade(trade: &TradeIn, venue: ContractRef) -> Result<SignedTrade, String> {
    Ok(SignedTrade {
        venue,
        signer: EvmAddress::parse(&trade.signer.trim().to_ascii_lowercase())
            .map_err(|e| format!("signer: {e}"))?,
        sent: trade
            .sent
            .iter()
            .map(trade_asset)
            .collect::<Result<_, _>>()?,
        received: trade
            .received
            .iter()
            .map(trade_asset)
            .collect::<Result<_, _>>()?,
    })
}

/// The stored trade by id, as the trades read serializes it.
pub async fn load_trade(pool: &PgPool, id: &str) -> Result<Option<TradeIn>, ResolveError> {
    // Whatever its copy's age: cancelling a leftover must keep working, and
    // the chain checks every action on a trade.
    let Some(trade) = trades::get_trade_by_id(pool, id, None).await? else {
        return Ok(None);
    };
    let value = serde_json::to_value(&trade).map_err(|e| ResolveError::Db(e.to_string()))?;
    serde_json::from_value(value)
        .map(Some)
        .map_err(|e| ResolveError::TradeShape(id.to_string(), e.to_string()))
}

/// The trade-events verdict of a stored trade, when its network is published.
pub async fn trade_verdict(
    pool: &PgPool,
    net: ChainNet,
    trade_id: &str,
) -> Result<(Option<(String, i32)>, bool), sqlx::Error> {
    let now = chrono::Utc::now();
    if chain_walk::ready_cursor(pool, chain_walk::INDEXER_TRADE_EVENTS, net, now)
        .await
        .is_none()
    {
        return Ok((None, false));
    }
    let row: Option<(String, i32)> = sqlx::query_as("SELECT status, executions FROM shop.trade_chain_state WHERE trade_id = $1::uuid AND folded_block >= 0")
        .bind(trade_id)
        .fetch_optional(pool)
        .await?;
    Ok((row.or(Some(("open".to_string(), 0))), true))
}

/// The legacy order of a token, when the nft index is published.
pub async fn legacy_order(
    pool: &PgPool,
    chain: &Chain,
    contract: Address,
    token_id: U256,
) -> Result<(Option<LegacyOrder>, bool), sqlx::Error> {
    let Some(net) = chain.net else {
        return Ok((None, false));
    };
    if chain_walk::ready_cursor(pool, INDEXER_NFT, net, chrono::Utc::now())
        .await
        .is_none()
    {
        return Ok((None, false));
    }
    let row = sqlx::query(
        "SELECT order_id, marketplace_address, status, seller, price::text AS price, expires_at, updated_block \
         FROM shop.nft_order WHERE network = $1 AND contract_address = $2 AND token_id = $3::numeric",
    )
    .bind(net.db_name())
    .bind(format!("{contract:#x}"))
    .bind(token_id.to_string())
    .fetch_optional(pool)
    .await?;
    let order = row.and_then(|r| {
        Some(LegacyOrder {
            order_id: r.get("order_id"),
            marketplace: r.get::<String, _>("marketplace_address").parse().ok()?,
            status: r.get("status"),
            seller: r.get::<String, _>("seller").parse().ok()?,
            price: U256::from_str_radix(&r.get::<String, _>("price"), 10).ok()?,
            expires_at: r.get("expires_at"),
            updated_block: r.get("updated_block"),
        })
    });
    Ok((order, true))
}

fn listing_count(l: &ListingRef) -> usize {
    [
        l.trade_id.is_some(),
        l.order_id.is_some(),
        l.rental_id.is_some(),
        l.name.is_some(),
        l.acquisition.as_deref() == Some("store"),
    ]
    .into_iter()
    .filter(|b| *b)
    .count()
}

#[cfg(test)]
mod store_tests {
    use super::*;
    use crate::policy::{Authoritative, Quote, eligibility, quote};
    use crate::types::{ManaUsdRate, PaymentMethod, SaleKind};

    #[tokio::test]
    async fn store_quotes_use_mirrored_identity_price_and_mint_permission() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
            return;
        };
        let _mirror_rows = crate::market::tests::support::MIRROR_STATE_LOCK
            .lock()
            .await;
        let collection = "0x000000000000000000000000000000000000c011";
        let creator = "0x000000000000000000000000000000000000c4ea";
        sqlx::query("INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) VALUES ('items','matic','fixture',now()), ('collections','matic','fixture',now()) ON CONFLICT (mirror,network) DO UPDATE SET last_complete_pass_at = now()")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO shop.collections (network,contract_address,chain_id,name,creator,owner,urn,is_completed,is_approved,is_editable,created_at,updated_at,reviewed_at,mirrored_at) VALUES ('matic',$1,137,'Store fixture',$2,$2,'urn:store-quote',true,true,false,0,0,0,now())")
            .bind(collection).bind(creator).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO shop.items (network,contract_address,item_id,chain_id,item_type,urn,creator,rarity,total_supply,max_supply,available,price,name,category,search_is_store_minter,search_is_collection_approved,created_at,updated_at,reviewed_at,mirrored_at) VALUES ('matic',$1,7,137,'wearable_v2','urn:store-quote:7',$2,'common',0,10,10,2300000000000000001,'Hat','hat',true,true,0,0,0,now())")
            .bind(collection).bind(creator).execute(&pool).await.unwrap();
        let book = Book::production(|_| None);
        let rentals = crate::rentals::db::Database::new(pool.clone());
        let mut request: LineRequest = serde_json::from_value(serde_json::json!({
            "asset": {"chainId":137,"contract":collection,"token":{"kind":"item","itemId":"7"}},
            "listing":{"acquisition":"store"}, "price": "1", "sale": "secondary"
        }))
        .unwrap();
        let buyer = EvmAddress::parse(creator).unwrap();
        for session in [
            Session::Web2 { account: buyer },
            Session::Web3 { wallet: buyer },
        ] {
            let resolved = resolve(&pool, &rentals, &book, session, &request)
                .await
                .unwrap();
            assert_eq!(resolved.available, Some(10));
            assert_eq!(resolved.listing.source(), "items-mirror");
            let line = Authoritative::derive(resolved.facts).unwrap();
            assert_eq!(line.sale(), SaleKind::PrimaryMint);
            let choices = eligibility(&line);
            assert_eq!(choices.allowed.first(), Some(&PaymentMethod::Credits));
            if matches!(session, Session::Web3 { .. }) {
                assert_eq!(
                    choices.allowed,
                    vec![PaymentMethod::Credits, PaymentMethod::Mana]
                );
                assert_eq!(
                    quote(&line, PaymentMethod::Mana, None).unwrap(),
                    Quote::Mana {
                        amount: ManaWei::new(U256::from(2_300_000_000_000_000_001u64))
                    }
                );
            }
            let rate = ManaUsdRate::parse_usd_per_mana("0.5").unwrap();
            // shop@94584a5c mana-convert.ts floors USD wei before rounding cents up.
            assert!(
                matches!(quote(&line, PaymentMethod::Credits, Some(&rate)).unwrap(), Quote::Credits { quoted } if quoted.get() == 115)
            );
        }
        sqlx::query("UPDATE shop.items SET price=2300000000000000002 WHERE contract_address=$1")
            .bind(collection)
            .execute(&pool)
            .await
            .unwrap();
        let repriced = resolve(
            &pool,
            &rentals,
            &book,
            Session::Web2 { account: buyer },
            &request,
        )
        .await
        .unwrap();
        let repriced = Authoritative::derive(repriced.facts).unwrap();
        let rate = ManaUsdRate::parse_usd_per_mana("0.5").unwrap();
        assert!(
            matches!(quote(&repriced, PaymentMethod::Credits, Some(&rate)).unwrap(), Quote::Credits { quoted } if quoted.get() == 116)
        );
        request.listing.order_id = Some("cannot-combine-with-store".into());
        assert_eq!(
            resolve(&pool, &rentals, &book, Session::SignedOut, &request)
                .await
                .unwrap_err(),
            ResolveError::Ambiguous
        );
        request.listing.order_id = None;
        let asset = request.asset.unwrap();
        request.asset = Some(AssetId::token(
            asset.chain_id,
            asset.contract,
            U256::from(7),
        ));
        assert!(matches!(
            resolve(&pool, &rentals, &book, Session::SignedOut, &request).await,
            Err(ResolveError::StoreUnavailable(_))
        ));
        request.asset = Some(asset);
        sqlx::query("UPDATE shop.items SET search_is_store_minter=false WHERE contract_address=$1")
            .bind(collection)
            .execute(&pool)
            .await
            .unwrap();
        assert!(matches!(
            resolve(&pool, &rentals, &book, Session::SignedOut, &request).await,
            Err(ResolveError::StoreUnavailable(_))
        ));
        sqlx::query("UPDATE shop.items SET search_is_store_minter=true, search_is_collection_approved=false WHERE contract_address=$1").bind(collection).execute(&pool).await.unwrap();
        assert!(matches!(
            resolve(&pool, &rentals, &book, Session::SignedOut, &request).await,
            Err(ResolveError::StoreUnavailable(_))
        ));
        sqlx::query("UPDATE shop.items SET search_is_collection_approved=true, available=0 WHERE contract_address=$1").bind(collection).execute(&pool).await.unwrap();
        let sold = resolve(&pool, &rentals, &book, Session::SignedOut, &request)
            .await
            .unwrap();
        let (checks, _, _) = crate::mana::verify::checks(&sold, None).await;
        assert!(!crate::mana::verify::executable(&checks));
        sqlx::query("UPDATE shop.mirror_state SET last_complete_pass_at=NULL WHERE mirror='items' AND network='matic'").execute(&pool).await.unwrap();
        assert!(matches!(
            resolve(&pool, &rentals, &book, Session::SignedOut, &request).await,
            Err(ResolveError::IndexNotReady { .. })
        ));
        sqlx::query("DELETE FROM shop.items WHERE contract_address=$1")
            .bind(collection)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM shop.collections WHERE contract_address=$1")
            .bind(collection)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM shop.mirror_state WHERE mirror IN ('items','collections') AND network='matic'").execute(&pool).await.unwrap();
    }
}

/// The order-book price of a legacy order: the record's `price`.
fn legacy_offer(chain: &Chain, order: &LegacyOrder, contract: Address, token_id: U256) -> Offer {
    Offer::LegacyOrder {
        venue: chain.contract(order.marketplace),
        nft: evm(contract),
        token_id,
        price: ManaWei::new(order.price),
    }
}

/// Resolves one line against the book, the mirrors, the indexes and the
/// rentals store. `session` is carried into the facts unchanged.
pub async fn resolve(
    pool: &PgPool,
    rentals: &crate::rentals::db::Database,
    book: &Book,
    session: Session,
    line: &LineRequest,
) -> Result<Resolved, ResolveError> {
    match listing_count(&line.listing) {
        0 => return Err(ResolveError::NoListing),
        1 => {}
        _ => return Err(ResolveError::Ambiguous),
    }
    if let Some(name) = &line.listing.name {
        return resolve_name(book, session, line, name).await;
    }
    let asset = line.asset.ok_or(ResolveError::NoAsset)?;
    let chain = book
        .chain(asset.chain_id)
        .ok_or(ResolveError::UnknownChain(asset.chain_id))?
        .clone();
    if line.listing.acquisition.as_deref() == Some("store") {
        return resolve_store(pool, chain, session, asset).await;
    }
    if let Some(id) = &line.listing.trade_id {
        return resolve_trade(pool, chain, session, asset, id).await;
    }
    let TokenRef::Token { token_id } = asset.token else {
        return Err(ResolveError::OrderNotFound(asset));
    };
    if let Some(rental_id) = &line.listing.rental_id {
        return resolve_rental(
            pool, rentals, chain, session, asset, token_id, rental_id, line,
        )
        .await;
    }
    let (order, ready) = legacy_order(pool, &chain, asset.contract.address(), token_id).await?;
    if !ready {
        return Err(ResolveError::IndexNotReady { index: "nft-index" });
    }
    let order = order.ok_or(ResolveError::OrderNotFound(asset))?;
    if let Some(named) = line
        .listing
        .order_id
        .as_deref()
        .filter(|n| !n.eq_ignore_ascii_case(&order.order_id))
    {
        return Err(ResolveError::OrderMismatch {
            named: named.to_string(),
            current: order.order_id.clone(),
        });
    }
    let role = contract_role(pool, &chain, asset.contract.address()).await?;
    let token = token_record(pool, &chain, &role, asset.contract.address(), token_id).await?;
    let offer = legacy_offer(&chain, &order, asset.contract.address(), token_id);
    Ok(Resolved {
        facts: Facts {
            session,
            offer: offer.clone(),
            contract: role,
            item: Lookup::Unknown,
            token,
        },
        chain,
        asset,
        offer,
        listing: Listing::LegacyOrder {
            order,
            index_ready: true,
        },
        available: None,
    })
}

/// A quote reads only the requested item's mint state and exact unit price. The existing
/// (network, contract_address, item_id) key serves this bounded lookup; no new projection.
async fn resolve_store(
    pool: &PgPool,
    chain: Chain,
    session: Session,
    asset: AssetId,
) -> Result<Resolved, ResolveError> {
    let TokenRef::Item { item_id } = asset.token else {
        return Err(ResolveError::StoreUnavailable(asset));
    };
    let venue = crate::credits::batch::collection_store(chain.chain_id)
        .ok_or(ResolveError::NoVenue("CollectionStore"))?;
    let (item, available) = item_record(pool, &chain, asset.contract.address(), item_id).await?;
    match &item {
        Lookup::Unknown => {
            return Err(ResolveError::IndexNotReady {
                index: "items-mirror",
            });
        }
        Lookup::Absent => return Err(ResolveError::StoreUnavailable(asset)),
        Lookup::Found(_) => {}
    }
    let net = chain.net.ok_or(ResolveError::StoreUnavailable(asset))?;
    let price: Option<String> = sqlx::query_scalar(
        "SELECT price::text FROM shop.items WHERE network = $1 AND contract_address = $2 AND item_id = $3::numeric \
         AND search_is_store_minter AND search_is_collection_approved",
    )
    .bind(net.db_name()).bind(asset.contract.to_string()).bind(item_id.to_string()).fetch_optional(pool).await?;
    let price = price.ok_or(ResolveError::StoreUnavailable(asset))?;
    let price = ManaWei::new(
        crate::types::parse_decimal(&price)
            .map_err(|_| ResolveError::Db("invalid CollectionStore price".into()))?,
    );
    let role = contract_role(pool, &chain, asset.contract.address()).await?;
    let offer = Offer::StoreMint {
        venue: chain.contract(venue.address()),
        collection: asset.contract,
        item_id,
        price,
    };
    Ok(Resolved {
        facts: Facts {
            session,
            offer: offer.clone(),
            contract: role,
            item,
            token: Lookup::Absent,
        },
        chain,
        asset,
        offer,
        listing: Listing::Store {
            venue: venue.address(),
            price,
        },
        available,
    })
}

async fn resolve_trade(
    pool: &PgPool,
    chain: Chain,
    session: Session,
    asset: AssetId,
    id: &str,
) -> Result<Resolved, ResolveError> {
    let trade = load_trade(pool, id)
        .await?
        .ok_or_else(|| ResolveError::TradeNotFound(id.to_string()))?;
    if let Some(trade_chain) = trade.chain_id
        && trade_chain != chain.chain_id.0
    {
        return Err(ResolveError::TradeChain {
            id: id.to_string(),
            trade_chain,
            line_chain: chain.chain_id.0,
        });
    }
    let venue: Address = trade
        .contract
        .as_deref()
        .and_then(|c| c.trim().parse().ok())
        .ok_or_else(|| ResolveError::TradeVenue(id.to_string()))?;
    if chain.marketplace(venue).is_none() {
        return Err(ResolveError::TradeVenue(id.to_string()));
    }
    let signed = signed_trade(&trade, chain.contract(venue))
        .map_err(|e| ResolveError::TradeShape(id.to_string(), e))?;
    let subject = signed
        .sent
        .first()
        .ok_or_else(|| ResolveError::TradeShape(id.to_string(), "no sent asset".into()))?;
    let sells = match (subject.asset_type, asset.token) {
        (TradeAssetType::Erc721, TokenRef::Token { token_id }) => subject.value == token_id,
        (TradeAssetType::CollectionItem, TokenRef::Item { item_id }) => subject.value == item_id,
        _ => false,
    } && subject.contract == asset.contract;
    if !sells {
        return Err(ResolveError::TradeAsset(id.to_string()));
    }
    let hashed_signature = hashed(&trade.signature);
    let (verdict, index_ready) = match chain.net {
        Some(net) => trade_verdict(pool, net, &trade.id).await?,
        None => (None, false),
    };
    let role = contract_role(pool, &chain, asset.contract.address()).await?;
    let (item, token, available) = match asset.token {
        TokenRef::Item { item_id } => {
            let (item, available) =
                item_record(pool, &chain, asset.contract.address(), item_id).await?;
            (item, Lookup::Absent, available)
        }
        TokenRef::Token { token_id } => {
            let token =
                token_record(pool, &chain, &role, asset.contract.address(), token_id).await?;
            let item = match &token {
                Lookup::Found(t) if t.item_id.is_some() => {
                    item_record(pool, &chain, asset.contract.address(), t.item_id.unwrap())
                        .await?
                        .0
                }
                _ => Lookup::Unknown,
            };
            (item, token, None)
        }
    };
    let offer = Offer::Trade {
        trade: signed,
        coupon: None,
    };
    Ok(Resolved {
        facts: Facts {
            session,
            offer: offer.clone(),
            contract: role,
            item,
            token,
        },
        chain,
        asset,
        offer,
        listing: Listing::Trade {
            trade,
            venue,
            hashed_signature,
            verdict,
            index_ready,
        },
        available,
    })
}

/// keccak256 of a trade's signature as the events and `shop.trades` spell it.
pub fn hashed(signature: &str) -> String {
    super::calls::hashed_signature(signature)
        .map(|h| format!("{h:#x}"))
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
async fn resolve_rental(
    pool: &PgPool,
    rentals: &crate::rentals::db::Database,
    chain: Chain,
    session: Session,
    asset: AssetId,
    token_id: U256,
    rental_id: &str,
    line: &LineRequest,
) -> Result<Resolved, ResolveError> {
    let listing = rentals
        .get_listing_by_id(rental_id)
        .await?
        .ok_or_else(|| ResolveError::RentalNotFound(rental_id.to_string()))?;
    let same_asset = listing
        .contract_address
        .eq_ignore_ascii_case(&asset.contract.to_string())
        && listing.token_id == token_id.to_string()
        && listing.chain_id == chain.chain_id.0 as i64;
    if !same_asset {
        return Err(ResolveError::RentalAsset(rental_id.to_string()));
    }
    let rentals_contract = chain
        .rentals
        .ok_or(ResolveError::NoVenue("rentals contract"))?;
    let terms = rent_terms(&listing, line.period_index.unwrap_or(0), line.rental_days)?;
    let role = contract_role(pool, &chain, asset.contract.address()).await?;
    let token = token_record(pool, &chain, &role, asset.contract.address(), token_id).await?;
    let offer = Offer::Rental {
        venue: chain.contract(rentals_contract),
        contract: asset.contract,
        token_id,
        price: terms.total,
    };
    let status_index_ready = crate::rentals::index::ready(pool, chrono::Utc::now()).await;
    Ok(Resolved {
        facts: Facts {
            session,
            offer: offer.clone(),
            contract: role,
            item: Lookup::Unknown,
            token,
        },
        chain,
        asset,
        offer,
        listing: Listing::Rental {
            listing,
            terms,
            rentals: rentals_contract,
            status_index_ready,
        },
        available: None,
    })
}

async fn resolve_name(
    book: &Book,
    session: Session,
    line: &LineRequest,
    name: &str,
) -> Result<Resolved, ResolveError> {
    let label = NameLabel::parse(name).map_err(|e| ResolveError::Name(e.to_string()))?;
    let chain = match line.asset {
        Some(asset) => book
            .chain(asset.chain_id)
            .ok_or(ResolveError::UnknownChain(asset.chain_id))?
            .clone(),
        None => book
            .chains()
            .find(|c| c.name_registrar.is_some() && c.name_controller.is_some())
            .ok_or(ResolveError::NoVenue("NAME registrar"))?
            .clone(),
    };
    let registrar = chain
        .name_registrar
        .ok_or(ResolveError::NoVenue("NAME registrar"))?;
    let controller = chain
        .name_controller
        .ok_or(ResolveError::NoVenue("NAME controller"))?;
    let asset = AssetId::name(chain.contract(registrar), &label);
    if let Some(named) = line.asset
        && named != asset
    {
        return Err(ResolveError::Name(format!(
            "the asset named is not the registrar token of {}",
            label.as_str()
        )));
    }
    // The price and the registration state are chain reads the verifier
    // makes; the offer carries the controller's fixed price once read, so
    // the resolver leaves it to `verify::name` and starts from zero here.
    let offer = Offer::NameRegistration {
        label: label.clone(),
        registrar: chain.contract(registrar),
        mana_venue: chain.contract(controller),
        credits_venue: chain.contract(controller),
        price: ManaWei::ZERO,
    };
    Ok(Resolved {
        facts: Facts {
            session,
            offer: offer.clone(),
            contract: Lookup::Found(ContractRole::NameRegistrar),
            item: Lookup::Absent,
            token: Lookup::Unknown,
        },
        chain,
        asset,
        offer,
        listing: Listing::Name {
            label,
            registrar,
            controller,
        },
        available: None,
    })
}

/// A NAME line once the registrar answered: the price the controller
/// charges and whether the label is registered.
pub fn with_name_reads(
    mut resolved: Resolved,
    price: ManaWei,
    registered_owner: Option<Address>,
) -> Resolved {
    if let Offer::NameRegistration { price: p, .. } = &mut resolved.offer {
        *p = price;
    }
    resolved.facts.offer = resolved.offer.clone();
    resolved.facts.token = match registered_owner {
        Some(owner) => Lookup::Found(TokenRecord {
            asset: resolved.asset,
            item_id: None,
            owner: evm(owner),
        }),
        None => Lookup::Absent,
    };
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_names_exactly_one_listing() {
        let one = ListingRef {
            trade_id: Some("t".into()),
            ..Default::default()
        };
        assert_eq!(listing_count(&one), 1);
        let two = ListingRef {
            trade_id: Some("t".into()),
            order_id: Some("o".into()),
            ..Default::default()
        };
        assert_eq!(listing_count(&two), 2);
        assert_eq!(listing_count(&ListingRef::default()), 0);
    }
}
