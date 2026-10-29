//! The Shop payment rule (SHOP.md slice 1) through its public entry points:
//! the eligibility table as derived from authoritative facts, the crafted
//! credit-spend bypasses, cart partitioning, and the pinned upstream rounding
//! and chain-read vectors (`tests/fixtures/shop/`).

use alloy_primitives::{U256, keccak256};
use catalyst::shop::policy::{
    Authoritative, CartLine, Facts, GroupTotal, LineRefusal, MAX_CREDIT_LINES_PER_GROUP,
    MissingFact, PartitionError, Quote, Reason, Refusal, SettlementGroup, WalletGuidance,
    eligibility, guard_credit_spend, guard_group_credit_spend, guard_method, partition, quote,
};
use catalyst::shop::types::{
    AssetId, AssetKind, ChainId, ContractRef, ContractRole, CreditSizing, CustomEffect, EvmAddress,
    ExternalCall, ItemKind, ItemRecord, Lookup, ManaUsdRate, ManaWei, NameLabel, Offer,
    PaymentMethod, SaleKind, Session, SettlementCall, SettlementGroupKey, SettlementOperation,
    SignedTrade, StoreItem, TokenRecord, TokenRef, TradeAsset, TradeAssetType, TradeCoupon,
    UsdCents, parse_decimal,
};
use serde_json::{Value, json};

use PaymentMethod::{Credits, Mana};

fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/shop/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn addr(text: &str) -> EvmAddress {
    EvmAddress::parse(text).unwrap()
}

fn synthetic(n: u64) -> EvmAddress {
    addr(&format!("0x{n:040x}"))
}

fn at(chain_id: ChainId, contract: EvmAddress) -> ContractRef {
    ContractRef { chain_id, contract }
}

/// The live Polygon contracts from `chain-reads.json`, synthetic addresses
/// for the rest (their role comes from the registry lookup, not the address).
struct Book {
    mana: EvmAddress,
    marketplace: ContractRef,
    store: ContractRef,
    legacy: ContractRef,
    executor: ContractRef,
    registrar: ContractRef,
    controller: ContractRef,
    eth_marketplace: ContractRef,
    eth_legacy: ContractRef,
    collection: EvmAddress,
    land: EvmAddress,
    estate: EvmAddress,
    rentals: ContractRef,
    creator: EvmAddress,
    buyer: EvmAddress,
    reseller: EvmAddress,
}

fn book() -> Book {
    let reads = fixture("chain-reads.json");
    let manager = &reads["polygon"]["creditsManager"];
    let text = |v: &Value| addr(v.as_str().unwrap());
    Book {
        mana: text(&manager["mana"]),
        marketplace: at(
            ChainId::POLYGON,
            addr("0xe38ef22abe871513555cba89adfe45ab4f548ada"),
        ),
        store: at(ChainId::POLYGON, text(&manager["collectionStore"])),
        legacy: at(ChainId::POLYGON, text(&manager["legacyMarketplace"])),
        executor: at(
            ChainId::POLYGON,
            text(&manager["allowedCustomExternalCalls"][0]["target"]),
        ),
        registrar: at(ChainId::ETHEREUM, text(&reads["ethereum"]["dclRegistrar"])),
        controller: at(ChainId::ETHEREUM, synthetic(0xc0)),
        eth_marketplace: at(ChainId::ETHEREUM, synthetic(0xe1)),
        eth_legacy: at(ChainId::ETHEREUM, synthetic(0xe2)),
        collection: synthetic(0xc011),
        land: synthetic(0x1a4d),
        estate: synthetic(0xe57a),
        rentals: at(ChainId::ETHEREUM, synthetic(0x4e47)),
        creator: synthetic(0xc4ea),
        buyer: synthetic(0xb0b),
        reseller: synthetic(0x5e11),
    }
}

fn web3(b: &Book) -> Session {
    Session::Web3 { wallet: b.buyer }
}

fn web2(b: &Book) -> Session {
    Session::Web2 { account: b.buyer }
}

const ITEM: u64 = 7;
const TOKEN: u64 = 42;

fn n(v: u64) -> U256 {
    U256::from(v)
}

fn item_record(b: &Book, chain_id: ChainId, kind: ItemKind) -> ItemRecord {
    ItemRecord {
        chain_id,
        collection: b.collection,
        item_id: n(ITEM),
        kind,
        creator: b.creator,
    }
}

fn token_record(chain_id: ChainId, contract: EvmAddress, owner: EvmAddress) -> TokenRecord {
    TokenRecord {
        asset: AssetId::token(chain_id, contract, n(TOKEN)),
        item_id: Some(n(ITEM)),
        owner,
    }
}

fn mana_price(b: &Book) -> TradeAsset {
    TradeAsset {
        asset_type: TradeAssetType::UsdPeggedMana,
        contract: b.mana,
        value: n(10).pow(n(18)),
    }
}

fn trade(b: &Book, venue: ContractRef, signer: EvmAddress, sent: Vec<TradeAsset>) -> SignedTrade {
    SignedTrade {
        venue,
        signer,
        sent,
        received: vec![mana_price(b)],
    }
}

fn collection_item(b: &Book) -> TradeAsset {
    TradeAsset {
        asset_type: TradeAssetType::CollectionItem,
        contract: b.collection,
        value: n(ITEM),
    }
}

fn erc721(contract: EvmAddress) -> TradeAsset {
    TradeAsset {
        asset_type: TradeAssetType::Erc721,
        contract,
        value: n(TOKEN),
    }
}

/// The CollectionStore price of the test item: 2.3 MANA for the wearable
/// ($1.15 at [`rate`]), 4.02 MANA for the emote ($2.01).
fn store_price(kind: ItemKind) -> ManaWei {
    let milli = if kind == ItemKind::Emote {
        4_020
    } else {
        2_300
    };
    ManaWei::new(n(milli) * n(10).pow(n(15)))
}

fn store_mint_facts(b: &Book, session: Session, kind: ItemKind) -> Facts {
    Facts {
        session,
        offer: Offer::StoreMint {
            venue: b.store,
            collection: b.collection,
            item_id: n(ITEM),
            price: store_price(kind),
        },
        contract: Lookup::Found(ContractRole::Collection),
        item: Lookup::Found(item_record(b, ChainId::POLYGON, kind)),
        token: Lookup::Absent,
    }
}

fn creator_order_trade(b: &Book) -> SignedTrade {
    trade(b, b.marketplace, b.creator, vec![collection_item(b)])
}

fn creator_order_facts(b: &Book, session: Session, kind: ItemKind) -> Facts {
    Facts {
        session,
        offer: Offer::Trade {
            trade: creator_order_trade(b),
            coupon: None,
        },
        contract: Lookup::Found(ContractRole::Collection),
        item: Lookup::Found(item_record(b, ChainId::POLYGON, kind)),
        token: Lookup::Absent,
    }
}

fn wearable_resale_trade(b: &Book, seller: EvmAddress) -> SignedTrade {
    trade(b, b.marketplace, seller, vec![erc721(b.collection)])
}

fn wearable_resale_facts(b: &Book, session: Session, seller: EvmAddress, kind: ItemKind) -> Facts {
    Facts {
        session,
        offer: Offer::Trade {
            trade: wearable_resale_trade(b, seller),
            coupon: None,
        },
        contract: Lookup::Found(ContractRole::Collection),
        item: Lookup::Found(item_record(b, ChainId::POLYGON, kind)),
        token: Lookup::Found(token_record(ChainId::POLYGON, b.collection, seller)),
    }
}

fn label() -> NameLabel {
    NameLabel::parse("Nacho").unwrap()
}

fn name_registration_facts(b: &Book, session: Session, registered: Lookup<TokenRecord>) -> Facts {
    name_facts(b, session, label(), registered)
}

fn name_facts(
    b: &Book,
    session: Session,
    label: NameLabel,
    registered: Lookup<TokenRecord>,
) -> Facts {
    Facts {
        session,
        offer: Offer::NameRegistration {
            label,
            registrar: b.registrar,
            mana_venue: b.controller,
            credits_venue: b.executor,
            price: ManaWei::from_mana(100),
        },
        contract: Lookup::Found(ContractRole::NameRegistrar),
        item: Lookup::Absent,
        token: registered,
    }
}

fn name_resale_facts(b: &Book, session: Session) -> Facts {
    Facts {
        session,
        offer: Offer::Trade {
            trade: trade(
                b,
                b.eth_marketplace,
                b.reseller,
                vec![erc721(b.registrar.contract)],
            ),
            coupon: None,
        },
        contract: Lookup::Found(ContractRole::NameRegistrar),
        item: Lookup::Absent,
        token: Lookup::Unknown,
    }
}

fn parcel_resale_facts(b: &Book, session: Session) -> Facts {
    Facts {
        session,
        offer: Offer::LegacyOrder {
            venue: b.eth_legacy,
            nft: b.land,
            token_id: n(TOKEN),
            price: ManaWei::from_mana(4000),
        },
        contract: Lookup::Found(ContractRole::Land),
        item: Lookup::Absent,
        token: Lookup::Found(token_record(ChainId::ETHEREUM, b.land, b.reseller)),
    }
}

fn estate_resale_facts(b: &Book, session: Session) -> Facts {
    Facts {
        session,
        offer: Offer::Trade {
            trade: trade(b, b.eth_marketplace, b.reseller, vec![erc721(b.estate)]),
            coupon: None,
        },
        contract: Lookup::Found(ContractRole::Estate),
        item: Lookup::Absent,
        token: Lookup::Unknown,
    }
}

fn rental_facts(b: &Book, session: Session) -> Facts {
    Facts {
        session,
        offer: Offer::Rental {
            venue: b.rentals,
            contract: b.land,
            token_id: n(TOKEN),
            price: ManaWei::from_mana(30),
        },
        contract: Lookup::Found(ContractRole::Land),
        item: Lookup::Absent,
        token: Lookup::Unknown,
    }
}

fn foreign_mint_facts(b: &Book, session: Session) -> Facts {
    Facts {
        session,
        offer: Offer::StoreMint {
            venue: b.store,
            collection: synthetic(0xf0),
            item_id: n(ITEM),
            price: ManaWei::from_mana(12),
        },
        contract: Lookup::Absent,
        item: Lookup::Absent,
        token: Lookup::Absent,
    }
}

fn foreign_resale_facts(b: &Book, session: Session) -> Facts {
    Facts {
        session,
        offer: Offer::Trade {
            trade: trade(b, b.marketplace, b.reseller, vec![erc721(synthetic(0xf0))]),
            coupon: None,
        },
        contract: Lookup::Absent,
        item: Lookup::Absent,
        token: Lookup::Unknown,
    }
}

fn derive(facts: Facts) -> Authoritative {
    Authoritative::derive(facts).unwrap()
}

/// The facts that express one AssetKind x SaleKind pair, or None when no
/// offer can (a rental or a parcel is never minted, only a NAME registers).
fn facts_for(b: &Book, kind: AssetKind, sale: SaleKind, session: Session) -> Option<Facts> {
    let item_kind = |kind| {
        if kind == AssetKind::Emote {
            ItemKind::Emote
        } else {
            ItemKind::Wearable
        }
    };
    Some(match (kind, sale) {
        (AssetKind::Wearable | AssetKind::Emote, SaleKind::PrimaryMint) => {
            store_mint_facts(b, session, item_kind(kind))
        }
        (AssetKind::Wearable | AssetKind::Emote, SaleKind::Secondary) => {
            wearable_resale_facts(b, session, b.reseller, item_kind(kind))
        }
        (AssetKind::Name, SaleKind::PrimaryNameRegistration) => {
            name_registration_facts(b, session, Lookup::Absent)
        }
        (AssetKind::Name, SaleKind::Secondary) => name_resale_facts(b, session),
        (AssetKind::Parcel, SaleKind::Secondary) => parcel_resale_facts(b, session),
        (AssetKind::Estate, SaleKind::Secondary) => estate_resale_facts(b, session),
        (AssetKind::Rental, SaleKind::Secondary) => rental_facts(b, session),
        (AssetKind::Other, SaleKind::PrimaryMint) => foreign_mint_facts(b, session),
        (AssetKind::Other, SaleKind::Secondary) => foreign_resale_facts(b, session),
        _ => return None,
    })
}

#[test]
fn the_derived_table_matches_the_confirmed_rule() {
    let b = book();
    let mut derived = 0;
    for kind in AssetKind::ALL {
        for sale in SaleKind::ALL {
            for session in [Session::SignedOut, web2(&b), web3(&b)] {
                let Some(facts) = facts_for(&b, kind, sale, session) else {
                    continue;
                };
                let line = derive(facts);
                let case = format!("{kind:?} {sale:?} {session:?}");
                assert_eq!((line.kind(), line.sale()), (kind, sale), "{case}");
                let got = eligibility(&line);
                let eligible = matches!(
                    (kind, sale),
                    (
                        AssetKind::Wearable | AssetKind::Emote,
                        SaleKind::PrimaryMint
                    ) | (AssetKind::Name, SaleKind::PrimaryNameRegistration)
                );
                if eligible {
                    let want = if session.has_wallet() {
                        vec![Credits, Mana]
                    } else {
                        vec![Credits]
                    };
                    assert_eq!(got.allowed, want, "{case}");
                    assert_eq!(got.default, Some(Credits), "{case}");
                    assert_eq!(got.guidance, None, "{case}");
                } else {
                    assert_eq!(got.allowed, vec![Mana], "{case}");
                    assert_eq!(got.default, Some(Mana), "{case}");
                    assert_eq!(got.guidance.is_some(), !session.has_wallet(), "{case}");
                    assert!(!got.reasons.contains(&Reason::CreditsFirst), "{case}");
                    assert!(guard_method(&line, Credits).is_err(), "{case}");
                }
                derived += 1;
            }
        }
    }
    assert_eq!(
        derived,
        11 * 3,
        "eleven derivable pairs, three sessions each"
    );
}

#[test]
fn pairs_no_offer_expresses_are_refused_when_crafted() {
    let b = book();
    for (role, contract) in [
        (ContractRole::Land, b.land),
        (ContractRole::Estate, b.estate),
        (ContractRole::NameRegistrar, b.registrar.contract),
    ] {
        let crafted = Facts {
            session: web3(&b),
            offer: Offer::Trade {
                trade: trade(
                    &b,
                    b.marketplace,
                    b.creator,
                    vec![TradeAsset {
                        asset_type: TradeAssetType::CollectionItem,
                        contract,
                        value: n(ITEM),
                    }],
                ),
                coupon: None,
            },
            contract: Lookup::Found(role),
            item: Lookup::Absent,
            token: Lookup::Absent,
        };
        let asset = AssetId::item(ChainId::POLYGON, contract, n(ITEM));
        assert_eq!(
            Authoritative::derive(crafted),
            Err(Refusal::NotMintable { asset }),
            "{role:?} mint"
        );
    }
    let taken = name_registration_facts(
        &b,
        web3(&b),
        Lookup::Found(TokenRecord {
            asset: AssetId::name(b.registrar, &label()),
            item_id: None,
            owner: b.reseller,
        }),
    );
    assert_eq!(
        Authoritative::derive(taken),
        Err(Refusal::NameTaken {
            asset: AssetId::name(b.registrar, &label())
        }),
        "a registered NAME is only ever a resale"
    );
}

#[test]
fn a_creator_reselling_a_minted_token_is_a_secondary_sale() {
    let b = book();
    let line = derive(wearable_resale_facts(
        &b,
        web3(&b),
        b.creator,
        ItemKind::Wearable,
    ));
    assert_eq!(line.sale(), SaleKind::Secondary);
    assert_eq!(line.asset().token, TokenRef::Token { token_id: n(TOKEN) });
    let got = eligibility(&line);
    assert_eq!(got.allowed, vec![Mana]);
    assert_eq!(got.reasons, vec![Reason::SecondarySale]);
    assert!(matches!(
        guard_credit_spend(&line, &authorize(ChainId::POLYGON)),
        Err(Refusal::CreditsNotAllowed {
            sale: SaleKind::Secondary,
            ..
        })
    ));
    let op = SettlementOperation::UseCredits {
        chain_id: ChainId::POLYGON,
        max_uncredited: ManaWei::ZERO,
        call: ExternalCall::MarketplaceAccept {
            venue: b.marketplace,
            trades: vec![wearable_resale_trade(&b, b.creator)],
            coupons: vec![],
        },
    };
    assert!(matches!(
        guard_credit_spend(&line, &op),
        Err(Refusal::CreditsNotAllowed { .. })
    ));
    let primary = derive(creator_order_facts(&b, web3(&b), ItemKind::Wearable));
    assert_eq!(
        guard_credit_spend(&primary, &op),
        Err(Refusal::SecondaryCreditSpend {
            asset: line.asset()
        }),
        "the creator's own order for the same item does not license spending on the minted token"
    );
}

fn cart(line: Authoritative, method: PaymentMethod) -> CartLine {
    CartLine { line, method }
}

/// $0.50 per MANA, 8 decimals (upstream shop `mana-rate.spec.ts` RATE_HALF).
fn rate() -> ManaUsdRate {
    ManaUsdRate::new(n(50_000_000), 8).unwrap()
}

fn priced(cart: &[CartLine]) -> Result<Vec<SettlementGroup>, PartitionError> {
    partition(cart, Some(&rate()))
}

/// The rate, upstream's 2% oracle buffer and `headroom_pct`.
fn sizing(headroom_pct: u32) -> CreditSizing {
    CreditSizing::new(rate(), 200, headroom_pct).unwrap()
}

/// Authorizing a zero-value credit: the tests that use it judge the lines.
fn authorize(chain_id: ChainId) -> SettlementOperation {
    SettlementOperation::AuthorizeCredit {
        chain_id,
        value: ManaWei::ZERO,
        sizing: sizing(100),
    }
}

#[test]
fn a_land_line_mixed_into_a_credits_group_is_refused_at_every_entry() {
    let b = book();
    let wearable = derive(store_mint_facts(&b, web3(&b), ItemKind::Wearable));
    let parcel = derive(parcel_resale_facts(&b, web3(&b)));
    let emote = derive(store_mint_facts(&b, web3(&b), ItemKind::Emote));

    let refused = priced(&[
        cart(wearable.clone(), Credits),
        cart(parcel.clone(), Credits),
        cart(emote.clone(), Credits),
    ])
    .unwrap_err()
    .refused;
    assert_eq!(
        refused,
        vec![LineRefusal {
            index: 1,
            refusal: Refusal::MethodNotAllowed {
                asset: parcel.asset(),
                method: Credits,
                allowed: vec![Mana]
            },
        }],
        "the parcel is refused, not moved to MANA or dropped"
    );

    assert!(matches!(
        guard_group_credit_spend(
            &[wearable.clone(), parcel.clone()],
            &authorize(ChainId::POLYGON)
        ),
        Err(Refusal::CreditsNotAllowed {
            kind: AssetKind::Parcel,
            ..
        })
    ));

    let land_on_polygon = trade(&b, b.marketplace, b.reseller, vec![erc721(b.land)]);
    let crafted = SettlementOperation::UseCredits {
        chain_id: ChainId::POLYGON,
        max_uncredited: ManaWei::ZERO,
        call: ExternalCall::MarketplaceAccept {
            venue: b.marketplace,
            trades: vec![creator_order_trade(&b), land_on_polygon],
            coupons: vec![],
        },
    };
    let order = derive(creator_order_facts(&b, web3(&b), ItemKind::Wearable));
    assert_eq!(
        guard_credit_spend(&order, &crafted),
        Err(Refusal::SecondaryCreditSpend {
            asset: AssetId::token(ChainId::POLYGON, b.land, n(TOKEN))
        })
    );

    let extra_mint = SettlementOperation::UseCredits {
        chain_id: ChainId::POLYGON,
        max_uncredited: ManaWei::ZERO,
        call: ExternalCall::CollectionStoreBuy {
            venue: b.store,
            items: vec![
                StoreItem {
                    collection: b.collection,
                    item_id: n(ITEM),
                    price: ManaWei::from_mana(12),
                },
                StoreItem {
                    collection: b.collection,
                    item_id: n(ITEM + 1),
                    price: ManaWei::from_mana(12),
                },
            ],
        },
    };
    assert_eq!(
        guard_credit_spend(&wearable, &extra_mint),
        Err(Refusal::AssetNotInGroup {
            asset: AssetId::item(ChainId::POLYGON, b.collection, n(ITEM + 1))
        }),
        "a batch cannot carry an unauthorized line under the group's credit"
    );
}

#[test]
fn a_hybrid_credits_and_mana_payment_on_a_resale_is_refused() {
    let b = book();
    let resale = derive(wearable_resale_facts(
        &b,
        web3(&b),
        b.reseller,
        ItemKind::Emote,
    ));
    let hybrid = SettlementOperation::UseCredits {
        chain_id: ChainId::POLYGON,
        max_uncredited: ManaWei::from_mana(50),
        call: ExternalCall::MarketplaceAccept {
            venue: b.marketplace,
            trades: vec![wearable_resale_trade(&b, b.reseller)],
            coupons: vec![],
        },
    };
    assert!(matches!(
        guard_credit_spend(&resale, &hybrid),
        Err(Refusal::CreditsNotAllowed { .. })
    ));
    let primary = derive(creator_order_facts(&b, web3(&b), ItemKind::Emote));
    assert_eq!(
        guard_credit_spend(&primary, &hybrid),
        Err(Refusal::SecondaryCreditSpend {
            asset: resale.asset()
        })
    );

    let primary_hybrid = SettlementOperation::UseCredits {
        chain_id: ChainId::POLYGON,
        max_uncredited: ManaWei::from_mana(50),
        call: ExternalCall::MarketplaceAccept {
            venue: b.marketplace,
            trades: vec![creator_order_trade(&b)],
            coupons: vec![],
        },
    };
    assert_eq!(
        guard_credit_spend(&primary, &primary_hybrid),
        Ok(()),
        "mixed payment stays within the web3 MANA option"
    );
    let web2_primary = derive(creator_order_facts(&b, web2(&b), ItemKind::Emote));
    assert_eq!(
        guard_credit_spend(&web2_primary, &primary_hybrid),
        Err(Refusal::ManaRemainderNotAllowed {
            asset: web2_primary.asset()
        })
    );
}

#[test]
fn a_relayed_custom_call_decoding_to_a_resale_is_refused() {
    let b = book();
    let primary = derive(creator_order_facts(&b, web3(&b), ItemKind::Wearable));
    let execute = [0xfd, 0x16, 0x5a, 0x73];
    let forwarded_resale = SettlementOperation::UseCredits {
        chain_id: ChainId::POLYGON,
        max_uncredited: ManaWei::ZERO,
        call: ExternalCall::Custom {
            venue: b.executor,
            selector: execute,
            effect: CustomEffect::Call(Box::new(ExternalCall::MarketplaceAccept {
                venue: b.marketplace,
                trades: vec![wearable_resale_trade(&b, b.reseller)],
                coupons: vec![],
            })),
        },
    };
    assert_eq!(
        guard_credit_spend(&primary, &forwarded_resale),
        Err(Refusal::ForwardedCustomCall {
            target: b.executor.contract,
            selector: execute
        })
    );
    let legacy_behind_custom = SettlementOperation::UseCredits {
        chain_id: ChainId::POLYGON,
        max_uncredited: ManaWei::ZERO,
        call: ExternalCall::Custom {
            venue: at(ChainId::POLYGON, synthetic(0xa99)),
            selector: [1, 2, 3, 4],
            effect: CustomEffect::Call(Box::new(ExternalCall::LegacyExecuteOrder {
                venue: b.legacy,
                nft: b.collection,
                token_id: n(TOKEN),
                price: ManaWei::from_mana(3),
            })),
        },
    };
    assert_eq!(
        guard_credit_spend(&primary, &legacy_behind_custom),
        Err(Refusal::ForwardedCustomCall {
            target: synthetic(0xa99),
            selector: [1, 2, 3, 4]
        })
    );
    let opaque = SettlementOperation::UseCredits {
        chain_id: ChainId::POLYGON,
        max_uncredited: ManaWei::ZERO,
        call: ExternalCall::Custom {
            venue: b.executor,
            selector: execute,
            effect: CustomEffect::Opaque,
        },
    };
    assert_eq!(
        guard_credit_spend(&primary, &opaque),
        Err(Refusal::UndecodedCustomCall {
            target: b.executor.contract,
            selector: execute
        })
    );

    let name = derive(name_registration_facts(&b, web3(&b), Lookup::Absent));
    let register = SettlementOperation::UseCredits {
        chain_id: ChainId::POLYGON,
        max_uncredited: ManaWei::ZERO,
        call: ExternalCall::Custom {
            venue: b.executor,
            selector: execute,
            effect: CustomEffect::NameRegistration {
                registrar: b.registrar,
                label: NameLabel::parse("nacho").unwrap(),
            },
        },
    };
    assert_eq!(
        guard_credit_spend(&name, &register),
        Ok(()),
        "the NAME identity is case-insensitive"
    );
    let resold_name = derive(name_resale_facts(&b, web3(&b)));
    assert!(matches!(
        guard_credit_spend(&resold_name, &register),
        Err(Refusal::CreditsNotAllowed { .. })
    ));
}

#[test]
fn a_custom_call_through_the_name_executor_never_settles_a_store_line() {
    let b = book();
    let execute = [0xfd, 0x16, 0x5a, 0x73];
    let mint = derive(store_mint_facts(&b, web3(&b), ItemKind::Wearable));
    assert_eq!(mint.venue(Credits), Some(b.store));
    let store_buy = ExternalCall::CollectionStoreBuy {
        venue: b.store,
        items: vec![StoreItem {
            collection: b.collection,
            item_id: n(ITEM),
            price: ManaWei::from_mana(12),
        }],
    };
    let through_executor = SettlementOperation::UseCredits {
        chain_id: ChainId::POLYGON,
        max_uncredited: ManaWei::ZERO,
        call: ExternalCall::Custom {
            venue: b.executor,
            selector: execute,
            effect: CustomEffect::Call(Box::new(store_buy.clone())),
        },
    };
    assert_eq!(
        guard_credit_spend(&mint, &through_executor),
        Err(Refusal::ForwardedCustomCall {
            target: b.executor.contract,
            selector: execute
        }),
        "the executor draws its own 100 MANA and forwards router calldata; a decoded inner call proves nothing"
    );
    let direct = SettlementOperation::UseCredits {
        chain_id: ChainId::POLYGON,
        max_uncredited: ManaWei::ZERO,
        call: store_buy,
    };
    assert_eq!(
        guard_credit_spend(&mint, &direct),
        Ok(()),
        "the manager's typed store route settles it"
    );
}

#[test]
fn a_listing_import_of_a_secondary_token_is_refused() {
    let b = book();
    let listing = wearable_resale_trade(&b, b.reseller);
    let resale = derive(wearable_resale_facts(
        &b,
        Session::SignedOut,
        b.reseller,
        ItemKind::Wearable,
    ));
    let import = SettlementOperation::CreditListing { trade: listing };
    assert!(matches!(
        guard_credit_spend(&resale, &import),
        Err(Refusal::CreditsNotAllowed { .. })
    ));
    let primary = derive(creator_order_facts(
        &b,
        Session::SignedOut,
        ItemKind::Wearable,
    ));
    assert_eq!(
        guard_credit_spend(&primary, &import),
        Err(Refusal::SecondaryCreditSpend {
            asset: resale.asset()
        })
    );
    let primary_import = SettlementOperation::CreditListing {
        trade: creator_order_trade(&b),
    };
    assert_eq!(
        guard_credit_spend(&primary, &primary_import),
        Ok(()),
        "primary inventory may be imported"
    );
}

#[test]
fn the_legacy_broker_spends_credits_on_mints_only() {
    let b = book();
    let resale = derive(Facts {
        session: web2(&b),
        offer: Offer::LegacyOrder {
            venue: b.legacy,
            nft: b.collection,
            token_id: n(TOKEN),
            price: ManaWei::from_mana(20),
        },
        contract: Lookup::Found(ContractRole::Collection),
        item: Lookup::Found(item_record(&b, ChainId::POLYGON, ItemKind::Wearable)),
        token: Lookup::Found(token_record(ChainId::POLYGON, b.collection, b.reseller)),
    });
    let buy_resale = SettlementOperation::LedgerBroker {
        debit: UsdCents::new(500),
        call: ExternalCall::LegacyExecuteOrder {
            venue: b.legacy,
            nft: b.collection,
            token_id: n(TOKEN),
            price: ManaWei::from_mana(20),
        },
    };
    assert!(matches!(
        guard_credit_spend(&resale, &buy_resale),
        Err(Refusal::CreditsNotAllowed { .. })
    ));
    let primary = derive(store_mint_facts(&b, web2(&b), ItemKind::Wearable));
    assert_eq!(
        guard_credit_spend(&primary, &buy_resale),
        Err(Refusal::SecondaryCreditSpend {
            asset: resale.asset()
        })
    );
    let buy_mint = SettlementOperation::LedgerBroker {
        debit: UsdCents::new(500),
        call: ExternalCall::CollectionStoreBuy {
            venue: b.store,
            items: vec![StoreItem {
                collection: b.collection,
                item_id: n(ITEM),
                price: ManaWei::from_mana(20),
            }],
        },
    };
    assert_eq!(guard_credit_spend(&primary, &buy_mint), Ok(()));
    let unpaid = SettlementOperation::LedgerBroker {
        debit: UsdCents::ZERO,
        call: buy_resale_call(&b),
    };
    assert_eq!(
        guard_credit_spend(&resale, &unpaid),
        Ok(()),
        "no debit, no credit spent"
    );
}

#[test]
fn a_negative_broker_debit_is_refused() {
    let b = book();
    let resale = derive(Facts {
        session: web2(&b),
        offer: Offer::LegacyOrder {
            venue: b.legacy,
            nft: b.collection,
            token_id: n(TOKEN),
            price: ManaWei::from_mana(20),
        },
        contract: Lookup::Found(ContractRole::Collection),
        item: Lookup::Found(item_record(&b, ChainId::POLYGON, ItemKind::Wearable)),
        token: Lookup::Found(token_record(ChainId::POLYGON, b.collection, b.reseller)),
    });
    let refund = SettlementOperation::LedgerBroker {
        debit: UsdCents::new(-100),
        call: buy_resale_call(&b),
    };
    assert_eq!(
        guard_credit_spend(&resale, &refund),
        Err(Refusal::NegativeDebit {
            debit: UsdCents::new(-100)
        }),
        "a debit that credits the ledger never pays for a resale or LAND buy"
    );
    let primary = derive(store_mint_facts(&b, web2(&b), ItemKind::Wearable));
    assert_eq!(
        guard_credit_spend(&primary, &refund),
        Err(Refusal::NegativeDebit {
            debit: UsdCents::new(-100)
        })
    );
}

fn buy_resale_call(b: &Book) -> ExternalCall {
    ExternalCall::LegacyExecuteOrder {
        venue: b.legacy,
        nft: b.collection,
        token_id: n(TOKEN),
        price: ManaWei::from_mana(20),
    }
}

#[test]
fn mana_operations_and_signed_out_spends() {
    let b = book();
    let parcel = derive(parcel_resale_facts(&b, web2(&b)));
    let direct = SettlementOperation::Mana {
        call: ExternalCall::LegacyExecuteOrder {
            venue: b.eth_legacy,
            nft: b.land,
            token_id: n(TOKEN),
            price: ManaWei::from_mana(4000),
        },
    };
    assert_eq!(guard_credit_spend(&parcel, &direct), Ok(()));
    assert_eq!(
        eligibility(&parcel).guidance,
        Some(WalletGuidance {
            chain_id: ChainId::ETHEREUM,
            native_gas_required: true,
            account: Some(b.buyer)
        })
    );
    let signed_out = derive(store_mint_facts(&b, Session::SignedOut, ItemKind::Wearable));
    assert_eq!(
        eligibility(&signed_out).reasons,
        vec![Reason::CreditsFirst, Reason::SignInForCredits]
    );
    assert_eq!(
        guard_credit_spend(&signed_out, &authorize(ChainId::POLYGON)),
        Err(Refusal::NotSignedIn)
    );
    let web2_line = derive(store_mint_facts(&b, web2(&b), ItemKind::Wearable));
    assert_eq!(
        guard_credit_spend(&web2_line, &authorize(ChainId::ETHEREUM)),
        Err(Refusal::CreditsChainUnsupported {
            chain_id: ChainId::ETHEREUM
        })
    );
    assert_eq!(
        guard_credit_spend(&web2_line, &authorize(ChainId::POLYGON)),
        Ok(())
    );
    let mixed_targets = [
        web2_line.clone(),
        derive(creator_order_facts(&b, web2(&b), ItemKind::Wearable)),
    ];
    assert_eq!(
        guard_group_credit_spend(&mixed_targets, &authorize(ChainId::POLYGON)),
        Err(Refusal::MixedGroup),
        "one credit covers one external call"
    );
    assert_eq!(
        guard_group_credit_spend(&[], &authorize(ChainId::POLYGON)),
        Err(Refusal::EmptyGroup)
    );
}

#[test]
fn missing_facts_make_a_purchase_unavailable_not_mana_only() {
    let b = book();
    let mut facts = store_mint_facts(&b, web3(&b), ItemKind::Wearable);
    facts.item = Lookup::Unknown;
    assert_eq!(
        Authoritative::derive(facts),
        Err(Refusal::Incomplete {
            missing: MissingFact::CollectionItem
        })
    );
    let mut facts = store_mint_facts(&b, web3(&b), ItemKind::Wearable);
    facts.contract = Lookup::Unknown;
    assert_eq!(
        Authoritative::derive(facts),
        Err(Refusal::Incomplete {
            missing: MissingFact::ContractRegistry
        })
    );
    let mut facts = store_mint_facts(&b, web3(&b), ItemKind::Wearable);
    facts.item = Lookup::Absent;
    assert!(matches!(
        Authoritative::derive(facts),
        Err(Refusal::UnknownItem { .. })
    ));
    let mut facts = store_mint_facts(&b, web3(&b), ItemKind::Wearable);
    facts.item = Lookup::Found(ItemRecord {
        item_id: n(ITEM + 9),
        ..item_record(&b, ChainId::POLYGON, ItemKind::Wearable)
    });
    assert!(matches!(
        Authoritative::derive(facts),
        Err(Refusal::RecordMismatch { .. })
    ));
    assert_eq!(
        Authoritative::derive(name_registration_facts(&b, web3(&b), Lookup::Unknown)),
        Err(Refusal::Incomplete {
            missing: MissingFact::NameRegistry
        })
    );
    let mut on_land = name_registration_facts(&b, web3(&b), Lookup::Absent);
    let Offer::NameRegistration { registrar, .. } = &mut on_land.offer else {
        unreachable!()
    };
    *registrar = at(ChainId::ETHEREUM, b.land);
    let asset = AssetId::name(at(ChainId::ETHEREUM, b.land), &label());
    for (role, want) in [
        (
            Lookup::Found(ContractRole::Land),
            Err(Refusal::NotARegistrar { asset }),
        ),
        (Lookup::Absent, Err(Refusal::NotARegistrar { asset })),
        (
            Lookup::Unknown,
            Err(Refusal::Incomplete {
                missing: MissingFact::ContractRegistry,
            }),
        ),
    ] {
        let facts = Facts {
            contract: role.clone(),
            ..on_land.clone()
        };
        assert_eq!(
            Authoritative::derive(facts),
            want,
            "a registration reads the registry role of its registrar: {role:?}"
        );
    }
    let mut resale = wearable_resale_facts(&b, web3(&b), b.reseller, ItemKind::Wearable);
    resale.contract = Lookup::Unknown;
    let line = derive(resale);
    assert_eq!(
        eligibility(&line).allowed,
        vec![Mana],
        "a resale needs no index to stay MANA only"
    );
}

#[test]
fn bundles_foreign_contracts_and_l1_mints_are_mana_only() {
    let b = book();
    let bundle = derive(Facts {
        session: web3(&b),
        offer: Offer::Trade {
            trade: trade(
                &b,
                b.marketplace,
                b.creator,
                vec![collection_item(&b), collection_item(&b)],
            ),
            coupon: None,
        },
        contract: Lookup::Found(ContractRole::Collection),
        item: Lookup::Found(item_record(&b, ChainId::POLYGON, ItemKind::Wearable)),
        token: Lookup::Absent,
    });
    assert_eq!(eligibility(&bundle).reasons, vec![Reason::Bundle]);
    assert_eq!(
        eligibility(&derive(foreign_mint_facts(&b, web3(&b)))).reasons,
        vec![Reason::NotDecentralandItem]
    );
    let l1_mint = derive(Facts {
        session: web3(&b),
        offer: Offer::StoreMint {
            venue: at(ChainId::ETHEREUM, synthetic(0x5701)),
            collection: b.collection,
            item_id: n(ITEM),
            price: ManaWei::from_mana(12),
        },
        contract: Lookup::Found(ContractRole::Collection),
        item: Lookup::Found(item_record(&b, ChainId::ETHEREUM, ItemKind::Wearable)),
        token: Lookup::Absent,
    });
    assert_eq!(
        eligibility(&l1_mint).reasons,
        vec![Reason::CreditsChainUnsupported]
    );
    let unpaid = trade(&b, b.marketplace, b.creator, vec![collection_item(&b)]);
    let no_mana = SignedTrade {
        received: vec![erc721(b.collection)],
        ..unpaid
    };
    assert!(matches!(
        Authoritative::derive(Facts {
            session: web3(&b),
            offer: Offer::Trade {
                trade: no_mana,
                coupon: None
            },
            contract: Lookup::Found(ContractRole::Collection),
            item: Lookup::Absent,
            token: Lookup::Absent,
        }),
        Err(Refusal::UnsupportedTrade { .. })
    ));
}

#[test]
fn a_mixed_cart_partitions_by_method_chain_and_contract() {
    let b = book();
    let lines = vec![
        cart(
            derive(store_mint_facts(&b, web3(&b), ItemKind::Wearable)),
            Credits,
        ),
        cart(
            derive(wearable_resale_facts(
                &b,
                web3(&b),
                b.reseller,
                ItemKind::Wearable,
            )),
            Mana,
        ),
        cart(
            derive(store_mint_facts(&b, web3(&b), ItemKind::Emote)),
            Credits,
        ),
        cart(
            derive(name_registration_facts(&b, web3(&b), Lookup::Absent)),
            Credits,
        ),
        cart(derive(parcel_resale_facts(&b, web3(&b))), Mana),
        cart(
            derive(creator_order_facts(&b, web3(&b), ItemKind::Wearable)),
            Mana,
        ),
        cart(
            derive(name_registration_facts(&b, web3(&b), Lookup::Absent)),
            Mana,
        ),
    ];
    let groups = priced(&lines).unwrap();
    let got: Vec<(SettlementGroupKey, Vec<usize>, GroupTotal, bool)> = groups
        .into_iter()
        .map(|g| (g.key, g.lines, g.total, g.mana_remainder_allowed))
        .collect();
    let key = |method, venue: ContractRef, call| SettlementGroupKey {
        method,
        chain_id: venue.chain_id,
        contract: venue.contract,
        call,
    };
    let mana_total = |whole| GroupTotal::Mana {
        amount: ManaWei::from_mana(whole),
    };
    let name = SettlementCall::One {
        asset: AssetId::name(b.registrar, &label()),
    };
    let parcel = SettlementCall::One {
        asset: AssetId::token(ChainId::ETHEREUM, b.land, n(TOKEN)),
    };
    assert_eq!(
        got,
        vec![
            (
                key(Credits, b.store, SettlementCall::StoreBuy),
                vec![0, 2],
                GroupTotal::Credits {
                    quoted: UsdCents::new(316),
                    charged: UsdCents::new(330)
                },
                true
            ),
            (
                key(Mana, b.marketplace, SettlementCall::Accept),
                vec![1, 5],
                mana_total(4),
                false
            ),
            (
                key(Credits, b.executor, name),
                vec![3],
                GroupTotal::Credits {
                    quoted: UsdCents::new(5000),
                    charged: UsdCents::new(5000)
                },
                true
            ),
            (
                key(Mana, b.eth_legacy, parcel),
                vec![4],
                mana_total(4000),
                false
            ),
            (
                key(Mana, b.controller, name),
                vec![6],
                mana_total(100),
                false
            ),
        ]
    );
}

const EXECUTE: [u8; 4] = [0xfd, 0x16, 0x5a, 0x73];

/// The call a guarded `useCredits` would make for one credits group.
fn group_call(key: &SettlementGroupKey, lines: &[Authoritative]) -> ExternalCall {
    let venue = at(key.chain_id, key.contract);
    match key.call {
        SettlementCall::Accept | SettlementCall::AcceptWithCoupon => {
            let (trades, coupons): (Vec<SignedTrade>, Vec<Option<TradeCoupon>>) = lines
                .iter()
                .map(|line| match line.offer() {
                    Offer::Trade { trade, coupon } => (trade.clone(), *coupon),
                    other => panic!("not a trade: {other:?}"),
                })
                .unzip();
            ExternalCall::MarketplaceAccept {
                venue,
                trades,
                coupons: coupons.into_iter().flatten().collect(),
            }
        }
        SettlementCall::StoreBuy => ExternalCall::CollectionStoreBuy {
            venue,
            items: lines
                .iter()
                .map(|line| match line.offer() {
                    Offer::StoreMint {
                        collection,
                        item_id,
                        ..
                    } => StoreItem {
                        collection: *collection,
                        item_id: *item_id,
                        price: ManaWei::from_mana(12),
                    },
                    other => panic!("not a mint: {other:?}"),
                })
                .collect(),
        },
        SettlementCall::One { .. } => match lines[0].offer() {
            Offer::NameRegistration {
                label, registrar, ..
            } => ExternalCall::Custom {
                venue,
                selector: EXECUTE,
                effect: CustomEffect::NameRegistration {
                    registrar: *registrar,
                    label: label.clone(),
                },
            },
            other => panic!("no credits call settles {other:?}"),
        },
    }
}

fn coupon() -> TradeCoupon {
    TradeCoupon {
        contract: synthetic(0xc0b0),
        discount_ppm: 300_000,
    }
}

fn discounted_order_facts(b: &Book, session: Session, kind: ItemKind) -> Facts {
    Facts {
        offer: Offer::Trade {
            trade: creator_order_trade(b),
            coupon: Some(coupon()),
        },
        ..creator_order_facts(b, session, kind)
    }
}

#[test]
fn discounted_and_plain_trades_settle_in_separate_groups() {
    let b = book();
    let plain = derive(creator_order_facts(&b, web3(&b), ItemKind::Wearable));
    let discounted = derive(discounted_order_facts(&b, web3(&b), ItemKind::Emote));
    let groups = priced(&[
        cart(plain.clone(), Credits),
        cart(discounted.clone(), Credits),
        cart(plain.clone(), Credits),
    ])
    .unwrap();
    let got: Vec<(SettlementCall, Vec<usize>)> = groups
        .iter()
        .map(|g| (g.key.call, g.lines.clone()))
        .collect();
    assert_eq!(
        got,
        vec![
            (SettlementCall::Accept, vec![0, 2]),
            (SettlementCall::AcceptWithCoupon, vec![1])
        ]
    );
    assert_eq!(
        groups[0].key.contract, groups[1].key.contract,
        "same marketplace, different calls"
    );
    assert_eq!(
        guard_group_credit_spend(
            &[plain.clone(), discounted.clone()],
            &authorize(ChainId::POLYGON)
        ),
        Err(Refusal::MixedGroup),
        "one useCredits call carries one selector"
    );
    let with_coupons = |coupons| SettlementOperation::UseCredits {
        chain_id: ChainId::POLYGON,
        max_uncredited: ManaWei::ZERO,
        call: ExternalCall::MarketplaceAccept {
            venue: b.marketplace,
            trades: vec![creator_order_trade(&b)],
            coupons,
        },
    };
    assert_eq!(
        guard_credit_spend(&discounted, &with_coupons(vec![coupon()])),
        Ok(())
    );
    assert_eq!(
        guard_credit_spend(&discounted, &with_coupons(vec![])),
        Err(Refusal::WrongCall {
            asset: discounted.asset()
        }),
        "a plain accept does not settle a discounted line"
    );
    assert_eq!(
        guard_credit_spend(&plain, &with_coupons(vec![coupon()])),
        Err(Refusal::WrongCall {
            asset: plain.asset()
        })
    );
    assert!(matches!(
        guard_credit_spend(&discounted, &with_coupons(vec![coupon(), coupon()])),
        Err(Refusal::UnsupportedTrade { .. })
    ));

    let resale = Facts {
        offer: Offer::Trade {
            trade: wearable_resale_trade(&b, b.reseller),
            coupon: Some(coupon()),
        },
        ..wearable_resale_facts(&b, web3(&b), b.reseller, ItemKind::Wearable)
    };
    assert!(
        matches!(
            Authoritative::derive(resale),
            Err(Refusal::UnsupportedTrade { .. })
        ),
        "a coupon discounts collection items only"
    );
    let free = Facts {
        offer: Offer::Trade {
            trade: creator_order_trade(&b),
            coupon: Some(TradeCoupon {
                discount_ppm: TradeCoupon::PPM,
                ..coupon()
            }),
        },
        ..creator_order_facts(&b, web3(&b), ItemKind::Wearable)
    };
    assert!(matches!(
        Authoritative::derive(free),
        Err(Refusal::UnsupportedTrade { .. })
    ));
}

#[test]
fn a_credit_is_bounded_by_its_groups_quoted_need() {
    let b = book();
    let lines = [
        derive(store_mint_facts(&b, web3(&b), ItemKind::Wearable)),
        derive(store_mint_facts(&b, web3(&b), ItemKind::Emote)),
    ];
    let signed = |value, headroom| SettlementOperation::AuthorizeCredit {
        chain_id: ChainId::POLYGON,
        value,
        sizing: sizing(headroom),
    };
    let need = ManaWei::new(n(6_446_400) * n(10).pow(n(12)));
    assert_eq!(
        sizing(100).cap(UsdCents::new(316)),
        Some(need),
        "316c at $0.50 plus the 2% buffer: 6.4464 MANA"
    );
    assert_eq!(guard_group_credit_spend(&lines, &signed(need, 100)), Ok(()));
    let over = need.checked_add(ManaWei::new(n(1))).unwrap();
    assert_eq!(
        guard_group_credit_spend(&lines, &signed(over, 100)),
        Err(Refusal::CreditValueTooLarge {
            value: over,
            cap: need
        }),
        "a credit signed above its group's need"
    );
    let upstream = sizing(150).cap(UsdCents::new(316)).unwrap();
    assert_eq!(
        guard_group_credit_spend(&lines, &signed(upstream, 150)),
        Ok(())
    );
    assert_eq!(
        upstream.get() - need.get(),
        need.get() / n(2),
        "at upstream's 150% the unconsumed half of the need stays spendable by the buyer until expiry"
    );
    assert_eq!(
        CreditSizing::new(rate(), 200, 151),
        None,
        "never above upstream's headroom"
    );
    assert_eq!(
        CreditSizing::new(rate(), 200, 99),
        None,
        "never below the need"
    );
    assert_eq!(CreditSizing::new(rate(), 1_001, 100), None);
}

#[test]
fn two_name_registrations_are_two_credits_groups() {
    let b = book();
    let alice = derive(name_facts(
        &b,
        web3(&b),
        NameLabel::parse("alice").unwrap(),
        Lookup::Absent,
    ));
    let bobby = derive(name_facts(
        &b,
        web3(&b),
        NameLabel::parse("bobby").unwrap(),
        Lookup::Absent,
    ));
    let groups = priced(&[cart(alice.clone(), Credits), cart(bobby.clone(), Credits)]).unwrap();
    assert_eq!(
        groups.iter().map(|g| g.lines.clone()).collect::<Vec<_>>(),
        vec![vec![0], vec![1]]
    );
    assert_eq!(
        groups[0].key.call,
        SettlementCall::One {
            asset: alice.asset()
        }
    );
    assert_eq!(
        guard_group_credit_spend(
            &[alice.clone(), bobby.clone()],
            &authorize(ChainId::POLYGON)
        ),
        Err(Refusal::MixedGroup),
        "one executor call registers one label, so one credit never covers two"
    );
}

#[test]
fn the_partition_and_the_use_credits_guard_agree_for_every_group_shape() {
    let b = book();
    let lines = [
        derive(store_mint_facts(&b, web3(&b), ItemKind::Wearable)),
        derive(creator_order_facts(&b, web3(&b), ItemKind::Wearable)),
        derive(name_facts(
            &b,
            web3(&b),
            NameLabel::parse("alice").unwrap(),
            Lookup::Absent,
        )),
        derive(discounted_order_facts(&b, web3(&b), ItemKind::Wearable)),
        derive(store_mint_facts(&b, web3(&b), ItemKind::Emote)),
        derive(creator_order_facts(&b, web3(&b), ItemKind::Emote)),
        derive(name_facts(
            &b,
            web3(&b),
            NameLabel::parse("bobby").unwrap(),
            Lookup::Absent,
        )),
        derive(discounted_order_facts(&b, web3(&b), ItemKind::Emote)),
    ];
    let cart_lines: Vec<CartLine> = lines
        .iter()
        .map(|line| cart(line.clone(), Credits))
        .collect();
    let groups = priced(&cart_lines).unwrap();
    assert_eq!(
        groups.len(),
        5,
        "store, plain accept, discounted accept, and one per NAME"
    );
    let members = |group: &SettlementGroup| -> Vec<Authoritative> {
        group.lines.iter().map(|&i| lines[i].clone()).collect()
    };
    for group in &groups {
        let group_lines = members(group);
        let case = format!("{:?}", group.key);
        let GroupTotal::Credits { quoted, .. } = group.total else {
            panic!("a credits group: {case}")
        };
        let exact = SettlementOperation::AuthorizeCredit {
            chain_id: group.key.chain_id,
            value: sizing(100).cap(quoted).unwrap(),
            sizing: sizing(100),
        };
        assert_eq!(
            guard_group_credit_spend(&group_lines, &exact),
            Ok(()),
            "{case}"
        );
        let spend = SettlementOperation::UseCredits {
            chain_id: group.key.chain_id,
            max_uncredited: ManaWei::ZERO,
            call: group_call(&group.key, &group_lines),
        };
        assert_eq!(
            guard_group_credit_spend(&group_lines, &spend),
            Ok(()),
            "{case}"
        );
    }
    for (i, one) in groups.iter().enumerate() {
        for other in &groups[i + 1..] {
            let merged: Vec<Authoritative> =
                members(one).into_iter().chain(members(other)).collect();
            assert_eq!(
                guard_group_credit_spend(&merged, &authorize(ChainId::POLYGON)),
                Err(Refusal::MixedGroup),
                "{:?} + {:?}",
                one.key,
                other.key
            );
        }
    }
}

#[test]
fn quotes_come_from_the_offer_and_the_rate_only() {
    let b = book();
    let usd = |cents: u64| TradeAsset {
        asset_type: TradeAssetType::UsdPeggedMana,
        contract: b.mana,
        value: n(cents) * n(10).pow(n(16)),
    };
    let order_with = |received: TradeAsset, coupon| {
        derive(Facts {
            offer: Offer::Trade {
                trade: SignedTrade {
                    received: vec![received],
                    ..creator_order_trade(&b)
                },
                coupon,
            },
            ..creator_order_facts(&b, web3(&b), ItemKind::Wearable)
        })
    };
    let credits = |cents| {
        Ok(Quote::Credits {
            quoted: UsdCents::new(cents),
        })
    };
    let mana_wei = |milli: u64| {
        Ok(Quote::Mana {
            amount: ManaWei::new(n(milli) * n(10).pow(n(15))),
        })
    };

    let pegged = order_with(usd(100), None);
    assert_eq!(
        quote(&pegged, Credits, None),
        credits(100),
        "a USD-pegged trade's credit price is its signed amount"
    );
    assert_eq!(
        quote(&pegged, Mana, Some(&rate())),
        mana_wei(2_000),
        "$1 at $0.50 per MANA"
    );
    assert_eq!(
        quote(&pegged, Mana, None),
        Err(Refusal::Incomplete {
            missing: MissingFact::ManaUsdRate
        })
    );
    let odd = order_with(
        TradeAsset {
            value: n(1_234_500_000_000_000_000),
            ..usd(0)
        },
        None,
    );
    assert_eq!(quote(&odd, Credits, None), credits(124), "cents round up");

    let legacy = order_with(
        TradeAsset {
            asset_type: TradeAssetType::Erc20,
            contract: b.mana,
            value: n(3) * n(10).pow(n(18)),
        },
        None,
    );
    assert_eq!(quote(&legacy, Mana, None), mana_wei(3_000));
    assert_eq!(quote(&legacy, Credits, Some(&rate())), credits(150));

    let discounted = order_with(usd(100), Some(coupon()));
    assert_eq!(
        quote(&discounted, Credits, None),
        credits(70),
        "the coupon lowers the signed price at settlement"
    );
    assert_eq!(quote(&discounted, Mana, Some(&rate())), mana_wei(1_400));

    let mint = derive(store_mint_facts(&b, web3(&b), ItemKind::Wearable));
    assert_eq!(quote(&mint, Credits, Some(&rate())), credits(115));
    assert_eq!(quote(&mint, Mana, None), mana_wei(2_300));
    let name = derive(name_registration_facts(&b, web3(&b), Lookup::Absent));
    assert_eq!(
        quote(&name, Credits, Some(&rate())),
        credits(5000),
        "100 MANA at $0.50"
    );

    let resale = derive(wearable_resale_facts(
        &b,
        web3(&b),
        b.reseller,
        ItemKind::Wearable,
    ));
    assert!(matches!(
        quote(&resale, Credits, Some(&rate())),
        Err(Refusal::MethodNotAllowed { .. })
    ));
    assert_eq!(
        priced(&[cart(mint.clone(), Credits), cart(pegged, Credits)]).unwrap()[1].quotes,
        vec![Quote::Credits {
            quoted: UsdCents::new(100)
        }],
        "the partition carries the quotes it computed"
    );
}

#[test]
fn a_cart_never_reroutes_a_line() {
    let b = book();
    let web2_primary = derive(store_mint_facts(&b, web2(&b), ItemKind::Wearable));
    let groups = priced(&[
        cart(web2_primary.clone(), Credits),
        cart(
            derive(wearable_resale_facts(
                &b,
                web2(&b),
                b.reseller,
                ItemKind::Wearable,
            )),
            Mana,
        ),
    ])
    .unwrap();
    assert!(
        !groups[0].mana_remainder_allowed,
        "no MANA leg without a web3 wallet"
    );
    let mut free = store_mint_facts(&b, web2(&b), ItemKind::Wearable);
    let Offer::StoreMint { price, .. } = &mut free.offer else {
        unreachable!()
    };
    *price = ManaWei::ZERO;
    let free = derive(free);
    let refused = priced(&[
        cart(web2_primary.clone(), Mana),
        cart(web2_primary.clone(), Credits),
        cart(free, Credits),
    ])
    .unwrap_err()
    .refused;
    let asset = web2_primary.asset();
    assert_eq!(
        refused,
        vec![
            LineRefusal {
                index: 0,
                refusal: Refusal::MethodNotAllowed {
                    asset,
                    method: Mana,
                    allowed: vec![Credits]
                }
            },
            LineRefusal {
                index: 2,
                refusal: Refusal::NonPositiveQuote { asset }
            },
        ]
    );
    assert_eq!(
        partition(&[cart(web2_primary.clone(), Credits)], None)
            .unwrap_err()
            .refused,
        vec![LineRefusal {
            index: 0,
            refusal: Refusal::Incomplete {
                missing: MissingFact::ManaUsdRate
            }
        }],
        "a MANA-priced mint has no credit price without the reference rate"
    );
    let many: Vec<CartLine> = (0..=MAX_CREDIT_LINES_PER_GROUP)
        .map(|_| cart(web2_primary.clone(), Credits))
        .collect();
    let refused = priced(&many).unwrap_err().refused;
    assert_eq!(refused.len(), MAX_CREDIT_LINES_PER_GROUP + 1);
    assert!(
        refused
            .iter()
            .all(|r| matches!(r.refusal, Refusal::GroupTooLarge { lines: 31, max: 30 }))
    );
}

fn u256(v: &Value) -> U256 {
    parse_decimal(v.as_str().unwrap()).unwrap()
}

fn cents(v: &Value) -> UsdCents {
    UsdCents::new(v.as_i64().unwrap())
}

#[test]
fn rounding_matches_the_pinned_upstream_vectors() {
    let v = fixture("rounding-vectors.json");
    let pairs = |name: &str| -> Vec<(i64, i64)> {
        v[name]["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| (c[0].as_i64().unwrap(), c[1].as_i64().unwrap()))
            .collect()
    };
    for (quoted, charged) in pairs("chargeForQuote") {
        assert_eq!(
            UsdCents::new(quoted).charge_for_quote(),
            Some(UsdCents::new(charged)),
            "{quoted}"
        );
    }
    for quoted in v["chargeForQuote"]["refused"].as_array().unwrap() {
        assert_eq!(cents(quoted).charge_for_quote(), None);
    }
    for (c, credits) in pairs("creditsCeil") {
        assert_eq!(UsdCents::new(c).credits_ceil(), credits, "ceil {c}");
    }
    for (c, credits) in pairs("creditsFloor") {
        assert_eq!(UsdCents::new(c).credits_floor(), credits, "floor {c}");
    }
    for (c, leg) in pairs("wholeCreditsFloor") {
        assert_eq!(
            UsdCents::new(c).whole_credits_floor(),
            UsdCents::new(leg),
            "leg {c}"
        );
    }
    for c in v["usdCentsToManaWei"]["cases"].as_array().unwrap() {
        let got = ManaWei::from_usd_cents(
            cents(&c["cents"]),
            u256(&c["rate"]),
            c["decimals"].as_u64().unwrap() as u8,
            c["bufferBps"].as_u64().unwrap() as u32,
        );
        assert_eq!(got, Some(ManaWei::new(u256(&c["manaWei"]))), "{c}");
    }
    for c in v["usdCentsToManaWei"]["refused"].as_array().unwrap() {
        assert_eq!(
            ManaWei::from_usd_cents(cents(&c["cents"]), u256(&c["rate"]), 8, 200),
            None,
            "{c}"
        );
    }
    for c in v["capHeadroom"]["cases"].as_array().unwrap() {
        let got = ManaWei::new(u256(&c["quotedWei"]))
            .with_cap_headroom(c["pct"].as_u64().unwrap() as u32);
        assert_eq!(got, Some(ManaWei::new(u256(&c["capWei"]))));
    }
    for c in v["manaForRemainder"]["cases"].as_array().unwrap() {
        let got = ManaWei::for_remainder(
            cents(&c["remainder"]),
            cents(&c["price"]),
            ManaWei::new(u256(&c["priceWei"])),
        );
        assert_eq!(got, Some(ManaWei::new(u256(&c["wei"]))), "{c}");
    }
    for c in v["usdWeiToCents"]["cases"].as_array().unwrap() {
        assert_eq!(
            UsdCents::from_usd_wei_ceil(u256(&c[0])),
            Some(cents(&c[1])),
            "{c}"
        );
    }
    let rate_of = |c: &Value| {
        ManaUsdRate::new(u256(&c["rate"]), c["decimals"].as_u64().unwrap() as u8).unwrap()
    };
    for c in v["manaWeiToUsdCents"]["cases"].as_array().unwrap() {
        assert_eq!(
            ManaWei::new(u256(&c["manaWei"])).usd_cents_ceil(rate_of(c)),
            Some(cents(&c["cents"])),
            "{c}"
        );
    }
    for c in v["usdWeiToManaWei"]["cases"].as_array().unwrap() {
        assert_eq!(
            ManaWei::from_usd_wei(u256(&c["usdWei"]), rate_of(c)),
            Some(ManaWei::new(u256(&c["manaWei"]))),
            "{c}"
        );
    }
    for c in v["discountedUsdCents"]["cases"].as_array().unwrap() {
        let ppm = c[1].as_u64().unwrap() as u32;
        assert_eq!(cents(&c[0]).discounted(ppm), Some(cents(&c[2])), "{c}");
    }
    assert_eq!(
        ManaUsdRate::new(n(0), 8),
        None,
        "a non-positive answer is no rate"
    );
    assert_eq!(UsdCents::from_credits(41), Some(UsdCents::new(410)));
}

#[test]
fn identities_and_the_chain_reads() {
    let reads = fixture("chain-reads.json");
    for (signature, selector) in reads["selectors"].as_object().unwrap() {
        let hash = keccak256(signature.as_bytes());
        assert_eq!(
            format!("0x{}", hex_of(&hash[..4])),
            selector.as_str().unwrap(),
            "{signature}"
        );
    }
    let expected = reads["ethereum"]["getTokenId"]["Nacho"].as_str().unwrap();
    assert_eq!(
        format!("0x{}", hex_of(&label().token_id().to_be_bytes::<32>())),
        expected
    );
    let manager = &reads["polygon"]["creditsManager"];
    assert_eq!(
        (
            manager["primarySalesAllowed"].as_bool(),
            manager["secondarySalesAllowed"].as_bool()
        ),
        (Some(true), Some(false)),
        "the shared manager agrees with the rule today; issuance still refuses a shared manager (docs/shop/CONTRACTS.md)"
    );

    let mixed = addr(
        "0x8B3A40CA1B6F5CAFC99D112A4D02E897D1FD8CC5"
            .to_lowercase()
            .as_str(),
    );
    assert_eq!(
        EvmAddress::parse("0x8B3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5").unwrap(),
        mixed
    );
    assert_eq!(
        mixed.to_string(),
        "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5"
    );
    assert!(EvmAddress::parse("8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5").is_err());
    assert!(EvmAddress::parse("0x8b3a").is_err());

    let item = AssetId::item(ChainId::POLYGON, mixed, n(ITEM));
    let token = AssetId::token(ChainId::POLYGON, mixed, n(ITEM));
    assert_ne!(
        item, token,
        "an item id and a token id with the same number are different assets"
    );
    assert_eq!(
        serde_json::to_value(item).unwrap(),
        json!({"chainId": 137, "contract": "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5", "token": {"kind": "item", "itemId": "7"}})
    );
    let back: AssetId = serde_json::from_value(serde_json::to_value(token).unwrap()).unwrap();
    assert_eq!(back, token);
    assert_eq!(
        serde_json::to_value(ManaWei::from_mana(2)).unwrap(),
        json!("2000000000000000000")
    );
    assert!(
        NameLabel::parse("a").is_err()
            && NameLabel::parse("dash-es").is_err()
            && NameLabel::parse("sixteencharacter").is_err()
    );
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
