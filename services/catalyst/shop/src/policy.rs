//! The payment-eligibility rule (SHOP.md, "Purchase experience and payment
//! rules", as confirmed 2026-09-25), pure and without I/O:
//!
//! - eligible primary wearables and emotes (CollectionStore mints and creator
//!   item orders) and primary NAME registration: credits first; a web3 wallet
//!   also sees MANA as the second option;
//! - LAND, estates, rentals and every secondary sale, NAME resales and a
//!   creator reselling a token already minted included: MANA only, with
//!   wallet/network guidance for a session without a web3 wallet.
//!
//! The input is [`Authoritative`], which only [`Authoritative::derive`] builds,
//! from indexed or mirrored records and a decoded offer. There is no field
//! through which a client names a sale "primary". Every entry calls one of
//! three functions: [`guard_method`] (quote, cart line), [`partition`] (cart
//! checkout) and [`guard_group_credit_spend`] (single or batch authorization,
//! listing import, the legacy broker, a relayed `useCredits` and execution).
//! A refusal never turns into another payment method: a restricted line is not
//! moved to credits, an eligible one is not moved to MANA, and missing facts
//! answer [`Refusal::Incomplete`] rather than a narrower choice.

use std::collections::HashMap;

use serde::Serialize;

use super::types::{
    AssetId, AssetKind, ChainId, ContractRef, ContractRole, CreditSizing, CustomEffect, EvmAddress,
    ExternalCall, ItemKind, ItemRecord, Lookup, ManaUsdRate, ManaWei, Offer, PaymentMethod,
    SaleKind, Session, SettlementCall, SettlementGroupKey, SettlementOperation, SignedTrade,
    TokenRecord, TokenRef, TradeAsset, TradeAssetType, TradeCoupon, UsdCents,
};

/// Upstream `MAX_LINES_PER_GROUP` (credits-server `authorize-usd-credit-batch.ts`).
pub const MAX_CREDIT_LINES_PER_GROUP: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MissingFact {
    ContractRegistry,
    CollectionItem,
    NameRegistry,
    /// The MANA/USD reference rate (the marketplace's aggregator).
    ManaUsdRate,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the {missing:?} source cannot answer yet; the purchase is unavailable, not re-priced")]
    Incomplete { missing: MissingFact },
    #[error("{asset} is not an item of its collection")]
    UnknownItem { asset: AssetId },
    #[error("the indexed records do not describe {asset}")]
    RecordMismatch { asset: AssetId },
    #[error("{asset} is not on a collection contract and cannot be minted")]
    NotMintable { asset: AssetId },
    #[error("unsupported trade: {detail}")]
    UnsupportedTrade { detail: &'static str },
    #[error("the NAME {asset} is already registered")]
    NameTaken { asset: AssetId },
    #[error("{asset} is not on the NAME registrar")]
    NotARegistrar { asset: AssetId },
    #[error("{asset} cannot be paid with {method:?}; {}", allowed_methods(.allowed))]
    MethodNotAllowed {
        asset: AssetId,
        method: PaymentMethod,
        allowed: Vec<PaymentMethod>,
    },
    #[error("the quote for {asset} is not positive")]
    NonPositiveQuote { asset: AssetId },
    #[error("the group total overflows at {asset}")]
    AmountOverflow { asset: AssetId },
    #[error("spending credits needs a signed-in account")]
    NotSignedIn,
    #[error("{asset} ({sale:?}, {kind:?}) is MANA only")]
    CreditsNotAllowed {
        asset: AssetId,
        sale: SaleKind,
        kind: AssetKind,
    },
    #[error("the operation would spend credits on the existing token {asset}")]
    SecondaryCreditSpend { asset: AssetId },
    #[error("the custom call {target} {selector:?} does not decode to a credit-eligible effect")]
    UndecodedCustomCall {
        target: EvmAddress,
        selector: [u8; 4],
    },
    #[error(
        "the custom call {target} {selector:?} forwards to another call; credits settle only on the manager's typed routes"
    )]
    ForwardedCustomCall {
        target: EvmAddress,
        selector: [u8; 4],
    },
    #[error("a MANA remainder on {asset} needs a web3 wallet session")]
    ManaRemainderNotAllowed { asset: AssetId },
    #[error("the operation settles {asset}, which is not a line of the group")]
    AssetNotInGroup { asset: AssetId },
    #[error("the operation does not settle the line {asset}")]
    LineNotSettled { asset: AssetId },
    #[error("the operation settles {asset} through another call than its group's")]
    WrongCall { asset: AssetId },
    #[error("the lines of one credit do not settle in one call")]
    MixedGroup,
    #[error("credits do not settle on chain {chain_id}")]
    CreditsChainUnsupported { chain_id: ChainId },
    #[error("the call targets chain {got}, the credit chain {expected}")]
    WrongChain { expected: ChainId, got: ChainId },
    #[error("a credits group holds at most {max} lines, not {lines}")]
    GroupTooLarge { lines: usize, max: usize },
    #[error("a credit spend needs at least one line")]
    EmptyGroup,
    #[error("a broker debit of {debit} would credit the buyer's ledger")]
    NegativeDebit { debit: UsdCents },
    #[error("a credit of {value} exceeds its group's bound of {cap}")]
    CreditValueTooLarge { value: ManaWei, cap: ManaWei },
    #[error(
        "at the marketplace's feed the group needs {need}, above the {reference} its quote allows; not signed"
    )]
    SettlementNeedAboveReference { need: ManaWei, reference: ManaWei },
    #[error("{venue:?} is not a known marketplace of its chain")]
    UnlistedMarketplace { venue: ContractRef },
}

/// Everything one purchase line is judged on. Each field comes from an
/// authority: the session from the verified auth chain, the offer from a
/// decoded signed order, a CollectionStore listing, a registrar or a rentals
/// listing, the rest from the contract registry, the collections mirror and
/// the NFT/NAME index.
#[derive(Debug, Clone)]
pub struct Facts {
    pub session: Session,
    pub offer: Offer,
    /// The registry's answer for the line's asset contract.
    pub contract: Lookup<ContractRole>,
    pub item: Lookup<ItemRecord>,
    pub token: Lookup<TokenRecord>,
}

/// One purchase line as the rule sees it. Built only by [`Authoritative::derive`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authoritative {
    session: Session,
    offer: Offer,
    asset: AssetId,
    kind: AssetKind,
    sale: SaleKind,
    bundle: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    Mint,
    Registration,
    Existing,
}

fn trade_asset_id(chain_id: ChainId, asset: &TradeAsset) -> Result<(AssetId, Form), Refusal> {
    match asset.asset_type {
        TradeAssetType::CollectionItem => Ok((
            AssetId::item(chain_id, asset.contract, asset.value),
            Form::Mint,
        )),
        TradeAssetType::Erc721 => Ok((
            AssetId::token(chain_id, asset.contract, asset.value),
            Form::Existing,
        )),
        _ => Err(Refusal::UnsupportedTrade {
            detail: "a trade sends only ERC-721 tokens or collection items",
        }),
    }
}

fn check_trade_payment(trade: &SignedTrade) -> Result<(), Refusal> {
    if trade.received.len() != 1 || !trade.received[0].asset_type.is_mana() {
        return Err(Refusal::UnsupportedTrade {
            detail: "a Shop trade receives exactly one MANA asset",
        });
    }
    if trade.sent.is_empty() {
        return Err(Refusal::UnsupportedTrade {
            detail: "a trade sends at least one asset",
        });
    }
    Ok(())
}

/// A coupon reverts on anything but collection items, and only a rate
/// discount strictly between 0 and 100% prices sensibly (upstream shop
/// `couponForTrade`).
fn check_coupon(coupon: &TradeCoupon, form: Form) -> Result<(), Refusal> {
    if form != Form::Mint {
        return Err(Refusal::UnsupportedTrade {
            detail: "a coupon discounts collection items only",
        });
    }
    if coupon.discount_ppm == 0 || coupon.discount_ppm >= TradeCoupon::PPM {
        return Err(Refusal::UnsupportedTrade {
            detail: "a coupon discount is between 0 and 100%",
        });
    }
    Ok(())
}

fn line_asset(offer: &Offer) -> Result<(AssetId, Form, bool), Refusal> {
    match offer {
        Offer::StoreMint {
            venue,
            collection,
            item_id,
            ..
        } => Ok((
            AssetId::item(venue.chain_id, *collection, *item_id),
            Form::Mint,
            false,
        )),
        Offer::Trade { trade, coupon } => {
            check_trade_payment(trade)?;
            let mut form = Form::Mint;
            let mut first = None;
            for sent in &trade.sent {
                let (asset, asset_form) = trade_asset_id(trade.venue.chain_id, sent)?;
                if asset_form == Form::Existing {
                    form = Form::Existing;
                }
                first.get_or_insert(asset);
            }
            let asset = first.ok_or(Refusal::UnsupportedTrade {
                detail: "a trade sends at least one asset",
            })?;
            if let Some(coupon) = coupon {
                check_coupon(coupon, form)?;
            }
            Ok((asset, form, trade.sent.len() > 1))
        }
        Offer::LegacyOrder {
            venue,
            nft,
            token_id,
            ..
        } => Ok((
            AssetId::token(venue.chain_id, *nft, *token_id),
            Form::Existing,
            false,
        )),
        Offer::NameRegistration {
            label, registrar, ..
        } => Ok((AssetId::name(*registrar, label), Form::Registration, false)),
        Offer::Rental {
            venue,
            contract,
            token_id,
            ..
        } => Ok((
            AssetId::token(venue.chain_id, *contract, *token_id),
            Form::Existing,
            false,
        )),
    }
}

fn item_matches(record: &ItemRecord, asset: &AssetId) -> bool {
    record.chain_id == asset.chain_id
        && record.collection == asset.contract
        && asset.token
            == TokenRef::Item {
                item_id: record.item_id,
            }
}

fn item_kind(kind: ItemKind) -> AssetKind {
    match kind {
        ItemKind::Wearable => AssetKind::Wearable,
        ItemKind::Emote => AssetKind::Emote,
    }
}

fn mint_kind(facts: &Facts, asset: &AssetId) -> Result<AssetKind, Refusal> {
    match &facts.contract {
        Lookup::Unknown => Err(Refusal::Incomplete {
            missing: MissingFact::ContractRegistry,
        }),
        Lookup::Absent => Ok(AssetKind::Other),
        Lookup::Found(ContractRole::Collection) => match &facts.item {
            Lookup::Found(record) if item_matches(record, asset) => Ok(item_kind(record.kind)),
            Lookup::Found(_) => Err(Refusal::RecordMismatch { asset: *asset }),
            Lookup::Absent => Err(Refusal::UnknownItem { asset: *asset }),
            Lookup::Unknown => Err(Refusal::Incomplete {
                missing: MissingFact::CollectionItem,
            }),
        },
        Lookup::Found(_) => Err(Refusal::NotMintable { asset: *asset }),
    }
}

/// The kind of an existing token is informational: every existing-token sale
/// is MANA only whatever it is, so a record the index lacks leaves it `Other`
/// instead of blocking a MANA purchase.
fn existing_kind(facts: &Facts, asset: &AssetId) -> Result<AssetKind, Refusal> {
    match &facts.contract {
        Lookup::Found(ContractRole::Land) => Ok(AssetKind::Parcel),
        Lookup::Found(ContractRole::Estate) => Ok(AssetKind::Estate),
        Lookup::Found(ContractRole::NameRegistrar) => Ok(AssetKind::Name),
        Lookup::Found(ContractRole::Collection) => match (&facts.token, &facts.item) {
            (Lookup::Found(token), Lookup::Found(record)) => {
                let describes = token.asset == *asset
                    && token.item_id == Some(record.item_id)
                    && record.chain_id == asset.chain_id
                    && record.collection == asset.contract;
                if describes {
                    Ok(item_kind(record.kind))
                } else {
                    Err(Refusal::RecordMismatch { asset: *asset })
                }
            }
            _ => Ok(AssetKind::Other),
        },
        Lookup::Absent | Lookup::Unknown => Ok(AssetKind::Other),
    }
}

impl Authoritative {
    pub fn derive(facts: Facts) -> Result<Self, Refusal> {
        let (asset, form, bundle) = line_asset(&facts.offer)?;
        let sale = match form {
            Form::Mint => SaleKind::PrimaryMint,
            Form::Existing => SaleKind::Secondary,
            Form::Registration => {
                match &facts.contract {
                    Lookup::Found(ContractRole::NameRegistrar) => {}
                    Lookup::Unknown => {
                        return Err(Refusal::Incomplete {
                            missing: MissingFact::ContractRegistry,
                        });
                    }
                    Lookup::Found(_) | Lookup::Absent => {
                        return Err(Refusal::NotARegistrar { asset });
                    }
                }
                match &facts.token {
                    Lookup::Absent => SaleKind::PrimaryNameRegistration,
                    Lookup::Found(_) => return Err(Refusal::NameTaken { asset }),
                    Lookup::Unknown => {
                        return Err(Refusal::Incomplete {
                            missing: MissingFact::NameRegistry,
                        });
                    }
                }
            }
        };
        let kind = match (&facts.offer, form) {
            (Offer::Rental { .. }, _) => AssetKind::Rental,
            (Offer::NameRegistration { .. }, _) => AssetKind::Name,
            (_, Form::Mint) => mint_kind(&facts, &asset)?,
            _ => existing_kind(&facts, &asset)?,
        };
        Ok(Self {
            session: facts.session,
            offer: facts.offer,
            asset,
            kind,
            sale,
            bundle,
        })
    }

    pub fn session(&self) -> Session {
        self.session
    }

    pub fn offer(&self) -> &Offer {
        &self.offer
    }

    pub fn asset(&self) -> AssetId {
        self.asset
    }

    pub fn kind(&self) -> AssetKind {
        self.kind
    }

    pub fn sale(&self) -> SaleKind {
        self.sale
    }

    pub fn is_bundle(&self) -> bool {
        self.bundle
    }

    /// The contract the line executes on when paid with `method`: the
    /// `useCredits` external-call target for credits, the direct target for
    /// MANA. None where the offer has no such route.
    pub fn venue(&self, method: PaymentMethod) -> Option<ContractRef> {
        match (&self.offer, method) {
            (Offer::NameRegistration { credits_venue, .. }, PaymentMethod::Credits) => {
                Some(*credits_venue)
            }
            (Offer::NameRegistration { mana_venue, .. }, PaymentMethod::Mana) => Some(*mana_venue),
            (Offer::Rental { .. } | Offer::LegacyOrder { .. }, PaymentMethod::Credits) => None,
            (Offer::StoreMint { venue, .. }, _)
            | (Offer::LegacyOrder { venue, .. }, _)
            | (Offer::Rental { venue, .. }, _)
            | (
                Offer::Trade {
                    trade: SignedTrade { venue, .. },
                    ..
                },
                _,
            ) => Some(*venue),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    /// Eligible primary: credits are the displayed price and the default.
    CreditsFirst,
    /// A web3 wallet also sees MANA, second.
    ManaSecond,
    /// Signed out: sign in (and buy credits as needed) to pay.
    SignInForCredits,
    SecondarySale,
    LandOrRental,
    /// Not an item of a Decentraland collection.
    NotDecentralandItem,
    /// Several assets in one signed trade.
    Bundle,
    /// A mint on a chain without a CreditsManager.
    CreditsChainUnsupported,
    /// A sale/asset pairing no primary purchase has.
    UnsupportedCombination,
}

/// For a session without a web3 wallet on a MANA-only purchase: which network
/// to fund and whether native gas is needed. It never suggests credits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletGuidance {
    pub chain_id: ChainId,
    pub native_gas_required: bool,
    /// The signed-in account whose wallet to set up; None when signed out.
    pub account: Option<EvmAddress>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Eligibility {
    /// In presentation order.
    pub allowed: Vec<PaymentMethod>,
    pub default: Option<PaymentMethod>,
    pub reasons: Vec<Reason>,
    pub guidance: Option<WalletGuidance>,
}

impl Eligibility {
    pub fn allows(&self, method: PaymentMethod) -> bool {
        self.allowed.contains(&method)
    }
}

/// The restrictions that make a line MANA only; empty means credits-eligible.
fn restrictions(kind: AssetKind, sale: SaleKind, chain_id: ChainId, bundle: bool) -> Vec<Reason> {
    let mut reasons = Vec::new();
    if matches!(
        kind,
        AssetKind::Parcel | AssetKind::Estate | AssetKind::Rental
    ) {
        reasons.push(Reason::LandOrRental);
    }
    if sale == SaleKind::Secondary {
        reasons.push(Reason::SecondarySale);
    }
    if kind == AssetKind::Other {
        reasons.push(Reason::NotDecentralandItem);
    }
    if bundle {
        reasons.push(Reason::Bundle);
    }
    let primary_pair = matches!(
        (sale, kind),
        (
            SaleKind::PrimaryMint,
            AssetKind::Wearable | AssetKind::Emote
        ) | (SaleKind::PrimaryNameRegistration, AssetKind::Name)
    );
    if reasons.is_empty() && !primary_pair {
        reasons.push(Reason::UnsupportedCombination);
    }
    if reasons.is_empty() && sale == SaleKind::PrimaryMint && !chain_id.is_polygon() {
        reasons.push(Reason::CreditsChainUnsupported);
    }
    reasons
}

fn present(restricted: Vec<Reason>, session: Session, mana_chain: ChainId) -> Eligibility {
    use PaymentMethod::{Credits, Mana};
    if restricted.is_empty() {
        let (allowed, reasons) = match session {
            Session::Web3 { .. } => (
                vec![Credits, Mana],
                vec![Reason::CreditsFirst, Reason::ManaSecond],
            ),
            Session::Web2 { .. } => (vec![Credits], vec![Reason::CreditsFirst]),
            Session::SignedOut => (
                vec![Credits],
                vec![Reason::CreditsFirst, Reason::SignInForCredits],
            ),
        };
        return Eligibility {
            allowed,
            default: Some(Credits),
            reasons,
            guidance: None,
        };
    }
    let guidance = match session {
        Session::Web3 { .. } => None,
        Session::Web2 { account } => Some(WalletGuidance {
            chain_id: mana_chain,
            native_gas_required: mana_chain.native_gas_required(),
            account: Some(account),
        }),
        Session::SignedOut => Some(WalletGuidance {
            chain_id: mana_chain,
            native_gas_required: mana_chain.native_gas_required(),
            account: None,
        }),
    };
    Eligibility {
        allowed: vec![Mana],
        default: Some(Mana),
        reasons: restricted,
        guidance,
    }
}

/// The buyer-facing half of [`Refusal::MethodNotAllowed`]: a line only MANA
/// can pay says so in those words.
fn allowed_methods(allowed: &[PaymentMethod]) -> String {
    if allowed == [PaymentMethod::Mana] {
        "it is MANA only".to_string()
    } else {
        format!("allowed: {allowed:?}")
    }
}

/// THE rule: which payment methods a line offers, in presentation order.
pub fn eligibility(line: &Authoritative) -> Eligibility {
    let mana_chain = line
        .venue(PaymentMethod::Mana)
        .map_or(line.asset.chain_id, |venue| venue.chain_id);
    present(
        restrictions(line.kind, line.sale, line.asset.chain_id, line.bundle),
        line.session,
        mana_chain,
    )
}

/// For a quote or a cart line: refuses a method the rule does not offer.
pub fn guard_method(line: &Authoritative, method: PaymentMethod) -> Result<(), Refusal> {
    let offered = eligibility(line);
    if offered.allows(method) {
        Ok(())
    } else {
        Err(Refusal::MethodNotAllowed {
            asset: line.asset,
            method,
            allowed: offered.allowed,
        })
    }
}

/// What one line costs in its chosen method. Only [`quote`] produces one,
/// from the offer's authoritative price, so no caller-supplied amount ever
/// reaches a group total or sizes a credit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum Quote {
    Credits { quoted: UsdCents },
    Mana { amount: ManaWei },
}

/// The price an offer's source sets, in the unit it sets it.
enum OfferPrice {
    /// USD wei (1e18 = $1): a USD-pegged trade.
    Usd(alloy_primitives::U256),
    Mana(ManaWei),
}

fn offer_price(offer: &Offer) -> Result<(OfferPrice, Option<u32>), Refusal> {
    match offer {
        Offer::Trade { trade, coupon } => {
            check_trade_payment(trade)?;
            let received = &trade.received[0];
            let price = match received.asset_type {
                TradeAssetType::UsdPeggedMana => OfferPrice::Usd(received.value),
                _ => OfferPrice::Mana(ManaWei::new(received.value)),
            };
            Ok((price, coupon.map(|coupon| coupon.discount_ppm)))
        }
        Offer::StoreMint { price, .. }
        | Offer::LegacyOrder { price, .. }
        | Offer::NameRegistration { price, .. }
        | Offer::Rental { price, .. } => Ok((OfferPrice::Mana(*price), None)),
    }
}

/// THE pricing function: a line's cost in `method`, from its offer alone. A
/// USD-pegged trade is priced in credits from its signed USD amount and in
/// MANA at the reference rate; a MANA price (a legacy trade, a store mint, a
/// NAME, a legacy order, a rental) is itself in MANA and priced in credits at
/// that rate; a coupon then lowers either (upstream shop `lineUsdCents`,
/// `readTradeManaPriceWei`, `discountedUsdCents`, `discountedManaWei`). A
/// missing rate makes the quote unavailable, never a guess.
pub fn quote(
    line: &Authoritative,
    method: PaymentMethod,
    rate: Option<&ManaUsdRate>,
) -> Result<Quote, Refusal> {
    guard_method(line, method)?;
    match method {
        PaymentMethod::Credits => credits_quote(line, rate).map(|quoted| Quote::Credits { quoted }),
        PaymentMethod::Mana => mana_quote(line, rate).map(|amount| Quote::Mana { amount }),
    }
}

fn need_rate(rate: Option<&ManaUsdRate>) -> Result<ManaUsdRate, Refusal> {
    rate.copied().ok_or(Refusal::Incomplete {
        missing: MissingFact::ManaUsdRate,
    })
}

/// A line's price in cents, whatever methods the rule offers it; callers go
/// through [`quote`] or a guard that has checked credits eligibility first.
fn credits_quote(line: &Authoritative, rate: Option<&ManaUsdRate>) -> Result<UsdCents, Refusal> {
    let asset = line.asset;
    let (price, discount) = offer_price(&line.offer)?;
    let cents = match price {
        OfferPrice::Usd(usd_wei) => UsdCents::from_usd_wei_ceil(usd_wei),
        OfferPrice::Mana(wei) => wei.usd_cents_ceil(need_rate(rate)?),
    };
    let cents = match discount {
        Some(ppm) => cents.and_then(|cents| cents.discounted(ppm)),
        None => cents,
    }
    .ok_or(Refusal::AmountOverflow { asset })?;
    if cents.get() > 0 {
        Ok(cents)
    } else {
        Err(Refusal::NonPositiveQuote { asset })
    }
}

/// Whether the line's price is set in USD (a USD-pegged trade), which the
/// marketplace converts to MANA at its own feed when the trade settles.
pub fn prices_in_usd(line: &Authoritative) -> bool {
    matches!(offer_price(&line.offer), Ok((OfferPrice::Usd(_), _)))
}

/// The most a credit for `lines` may sign, from the lines alone: the one
/// function both the sizing and [`guard_group_credit_spend`] use.
///
/// The reference cap is the group's quoted cents at the quote rate, buffered
/// and with headroom: the bound before a settlement rate existed. Without a
/// settlement rate it is the cap.
///
/// With one (the group holds a USD-pegged line), the group's need is summed
/// in MANA, so no cents total is converted back at a single rate: each
/// USD-pegged line is its USD amount after its coupon (the contract's
/// `price - floor(price * discount / 1e6)`) converted at the settlement
/// rate, flooring, which is the order and rounding the marketplace uses; the
/// MANA-priced lines keep their quoted cents at the quote rate, converted
/// together. A need above the reference cap is refused
/// ([`Refusal::SettlementNeedAboveReference`]): no credit is signed that is
/// already short when the feed is read, and none is signed above the
/// reference. Otherwise the cap is the need buffered and with headroom, no
/// higher than the reference cap. The reference cap never uses the
/// settlement rate, so the guard bounds the signed value independently of
/// the feed. Neither bound promises the later spend fits: a feed that moves
/// during the credit's life is covered by what is left of the margin, if
/// anything.
pub fn credit_cap(lines: &[Authoritative], sizing: &CreditSizing) -> Result<ManaWei, Refusal> {
    let Some(asset) = lines.first().map(|line| line.asset) else {
        return Err(Refusal::EmptyGroup);
    };
    let overflow = Refusal::AmountOverflow { asset };
    let quote_rate = sizing.rate();
    let mut quoted = UsdCents::ZERO;
    for line in lines {
        let cents = credits_quote(line, Some(&quote_rate))?;
        quoted = quoted
            .checked_add(cents)
            .ok_or(Refusal::AmountOverflow { asset: line.asset })?;
    }
    let reference = sizing.cap(quoted).ok_or(overflow.clone())?;
    let Some(settlement_rate) = sizing.settlement_rate() else {
        return Ok(reference);
    };
    let mut usd_pegged = ManaWei::ZERO;
    let mut mana_priced_cents = UsdCents::ZERO;
    for line in lines {
        match offer_price(&line.offer)? {
            (OfferPrice::Usd(usd_wei), discount) => {
                let mana = usd_pegged_settlement(usd_wei, discount, settlement_rate)
                    .ok_or(Refusal::AmountOverflow { asset: line.asset })?;
                if mana.is_zero() {
                    return Err(Refusal::NonPositiveQuote { asset: line.asset });
                }
                usd_pegged = usd_pegged
                    .checked_add(mana)
                    .ok_or(Refusal::AmountOverflow { asset: line.asset })?;
            }
            (OfferPrice::Mana(_), _) => {
                let cents = credits_quote(line, Some(&quote_rate))?;
                mana_priced_cents = mana_priced_cents
                    .checked_add(cents)
                    .ok_or(Refusal::AmountOverflow { asset: line.asset })?;
            }
        }
    }
    let mana_priced = if mana_priced_cents.get() > 0 {
        ManaWei::from_usd_cents(
            mana_priced_cents,
            quote_rate.rate(),
            quote_rate.decimals(),
            0,
        )
        .ok_or(overflow.clone())?
    } else {
        ManaWei::ZERO
    };
    let need = usd_pegged
        .checked_add(mana_priced)
        .ok_or(overflow.clone())?;
    if need > reference {
        return Err(Refusal::SettlementNeedAboveReference { need, reference });
    }
    let sized = sizing.cap_for_need(need).ok_or(overflow)?;
    Ok(sized.min(reference))
}

/// The MANA a USD-pegged amount settles for: the coupon applied to the USD
/// amount first (`price - floor(price * discount / 1e6)`, so the kept part
/// rounds up), then converted at `rate`, flooring (`CollectionDiscountCoupon`
/// before `DecentralandMarketplacePolygon`'s conversion).
fn usd_pegged_settlement(
    usd_wei: alloy_primitives::U256,
    discount_ppm: Option<u32>,
    rate: ManaUsdRate,
) -> Option<ManaWei> {
    let usd_wei = match discount_ppm {
        Some(ppm) => {
            let ppm = alloy_primitives::U256::from(ppm.min(1_000_000));
            let off = usd_wei.checked_mul(ppm)? / alloy_primitives::U256::from(1_000_000u64);
            usd_wei - off
        }
        None => usd_wei,
    };
    ManaWei::from_usd_wei(usd_wei, rate)
}

fn mana_quote(line: &Authoritative, rate: Option<&ManaUsdRate>) -> Result<ManaWei, Refusal> {
    let asset = line.asset;
    let (price, discount) = offer_price(&line.offer)?;
    let amount = match price {
        OfferPrice::Usd(usd_wei) => ManaWei::from_usd_wei(usd_wei, need_rate(rate)?),
        OfferPrice::Mana(amount) => Some(amount),
    };
    let amount = match discount {
        Some(ppm) => amount.and_then(|amount| amount.discounted(ppm)),
        None => amount,
    }
    .ok_or(Refusal::AmountOverflow { asset })?;
    if amount.is_zero() {
        Err(Refusal::NonPositiveQuote { asset })
    } else {
        Ok(amount)
    }
}

/// One cart line: the authoritative line and the method the buyer chose.
/// It carries no price; [`partition`] quotes it.
#[derive(Debug, Clone)]
pub struct CartLine {
    pub line: Authoritative,
    pub method: PaymentMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum GroupTotal {
    /// `charged` sums each line's whole-credit charge, as upstream's batch does.
    Credits {
        quoted: UsdCents,
        charged: UsdCents,
    },
    Mana {
        amount: ManaWei,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementGroup {
    pub key: SettlementGroupKey,
    /// Indexes into the cart, in cart order.
    pub lines: Vec<usize>,
    /// Each line's quote, aligned with `lines`.
    pub quotes: Vec<Quote>,
    pub total: GroupTotal,
    /// A credits group whose every line also offers MANA (a web3 wallet) may
    /// top up a short credit with a MANA remainder.
    pub mana_remainder_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRefusal {
    pub index: usize,
    pub refusal: Refusal,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{} cart line(s) cannot settle as chosen", refused.len())]
pub struct PartitionError {
    pub refused: Vec<LineRefusal>,
}

/// The group a line settles in when paid with `method`: the contract its
/// call targets and the call's shape, so that one group is one transaction.
fn settlement_key(
    line: &Authoritative,
    method: PaymentMethod,
) -> Result<SettlementGroupKey, Refusal> {
    let venue = line.venue(method).ok_or(Refusal::MethodNotAllowed {
        asset: line.asset,
        method,
        allowed: eligibility(line).allowed,
    })?;
    if method == PaymentMethod::Credits && !venue.chain_id.is_polygon() {
        return Err(Refusal::CreditsChainUnsupported {
            chain_id: venue.chain_id,
        });
    }
    let call = match &line.offer {
        Offer::Trade { coupon: None, .. } => SettlementCall::Accept,
        Offer::Trade {
            coupon: Some(_), ..
        } => SettlementCall::AcceptWithCoupon,
        Offer::StoreMint { .. } => SettlementCall::StoreBuy,
        Offer::LegacyOrder { .. } | Offer::Rental { .. } | Offer::NameRegistration { .. } => {
            SettlementCall::One { asset: line.asset }
        }
    };
    Ok(SettlementGroupKey {
        method,
        chain_id: venue.chain_id,
        contract: venue.contract,
        call,
    })
}

fn line_key(
    cart: &CartLine,
    rate: Option<&ManaUsdRate>,
) -> Result<(SettlementGroupKey, Quote), Refusal> {
    let quote = quote(&cart.line, cart.method, rate)?;
    Ok((settlement_key(&cart.line, cart.method)?, quote))
}

fn add_quote(total: GroupTotal, quote: Quote) -> Option<GroupTotal> {
    match (total, quote) {
        (GroupTotal::Credits { quoted, charged }, Quote::Credits { quoted: line }) => {
            Some(GroupTotal::Credits {
                quoted: quoted.checked_add(line)?,
                charged: charged.checked_add(line.charge_for_quote()?)?,
            })
        }
        (GroupTotal::Mana { amount }, Quote::Mana { amount: line }) => Some(GroupTotal::Mana {
            amount: amount.checked_add(line)?,
        }),
        _ => None,
    }
}

/// Splits a cart into confirmation groups by payment method, chain,
/// settlement contract and call, in order of first appearance, so that each
/// group is one transaction (a NAME registration is always its own), each
/// line quoted by [`quote`] at `rate`. All or nothing: any line whose chosen
/// method the rule does not offer, or that cannot be priced, refuses the
/// partition, with every such line named, so nothing is silently re-routed.
pub fn partition(
    cart: &[CartLine],
    rate: Option<&ManaUsdRate>,
) -> Result<Vec<SettlementGroup>, PartitionError> {
    let mut groups: Vec<SettlementGroup> = Vec::new();
    let mut by_key: HashMap<SettlementGroupKey, usize> = HashMap::new();
    let mut refused = Vec::new();
    for (index, line) in cart.iter().enumerate() {
        let (key, quote) = match line_key(line, rate) {
            Ok(keyed) => keyed,
            Err(refusal) => {
                refused.push(LineRefusal { index, refusal });
                continue;
            }
        };
        let slot = *by_key.entry(key).or_insert_with(|| {
            groups.push(SettlementGroup {
                key,
                lines: Vec::new(),
                quotes: Vec::new(),
                total: match key.method {
                    PaymentMethod::Credits => GroupTotal::Credits {
                        quoted: UsdCents::ZERO,
                        charged: UsdCents::ZERO,
                    },
                    PaymentMethod::Mana => GroupTotal::Mana {
                        amount: ManaWei::ZERO,
                    },
                },
                mana_remainder_allowed: key.method == PaymentMethod::Credits,
            });
            groups.len() - 1
        });
        let group = &mut groups[slot];
        let Some(total) = add_quote(group.total, quote) else {
            refused.push(LineRefusal {
                index,
                refusal: Refusal::AmountOverflow {
                    asset: line.line.asset,
                },
            });
            continue;
        };
        group.total = total;
        group.lines.push(index);
        group.quotes.push(quote);
        if key.method == PaymentMethod::Credits
            && !eligibility(&line.line).allows(PaymentMethod::Mana)
        {
            group.mana_remainder_allowed = false;
        }
    }
    for group in &groups {
        if group.key.method == PaymentMethod::Credits
            && group.lines.len() > MAX_CREDIT_LINES_PER_GROUP
        {
            for &index in &group.lines {
                refused.push(LineRefusal {
                    index,
                    refusal: Refusal::GroupTooLarge {
                        lines: group.lines.len(),
                        max: MAX_CREDIT_LINES_PER_GROUP,
                    },
                });
            }
        }
    }
    if refused.is_empty() {
        Ok(groups)
    } else {
        refused.sort_by_key(|line| line.index);
        Err(PartitionError { refused })
    }
}

/// The assets a decoded call would deliver, each with the contract it
/// executes on. Any existing token, any custom call nobody decoded and any
/// custom call that forwards to another call refuses: credits pay for mints
/// on the manager's typed routes and for NAME registrations only.
fn credit_settled(call: &ExternalCall) -> Result<Vec<(AssetId, ContractRef)>, Refusal> {
    match call {
        ExternalCall::MarketplaceAccept {
            venue,
            trades,
            coupons,
        } => {
            if !coupons.is_empty() && coupons.len() != trades.len() {
                return Err(Refusal::UnsupportedTrade {
                    detail: "acceptWithCoupon takes one coupon per trade",
                });
            }
            let mut settled = Vec::new();
            for trade in trades {
                check_trade_payment(trade)?;
                for sent in &trade.sent {
                    let (asset, form) = trade_asset_id(venue.chain_id, sent)?;
                    if form == Form::Existing {
                        return Err(Refusal::SecondaryCreditSpend { asset });
                    }
                    settled.push((asset, *venue));
                }
            }
            Ok(settled)
        }
        ExternalCall::LegacyExecuteOrder {
            venue,
            nft,
            token_id,
            ..
        } => Err(Refusal::SecondaryCreditSpend {
            asset: AssetId::token(venue.chain_id, *nft, *token_id),
        }),
        ExternalCall::CollectionStoreBuy { venue, items } => Ok(items
            .iter()
            .map(|item| {
                (
                    AssetId::item(venue.chain_id, item.collection, item.item_id),
                    *venue,
                )
            })
            .collect()),
        ExternalCall::NameRegister {
            venue,
            registrar,
            label,
        } => Ok(vec![(AssetId::name(*registrar, label), *venue)]),
        ExternalCall::Custom {
            venue,
            selector,
            effect,
        } => match effect {
            CustomEffect::NameRegistration { registrar, label } => {
                Ok(vec![(AssetId::name(*registrar, label), *venue)])
            }
            CustomEffect::Call(_) => Err(Refusal::ForwardedCustomCall {
                target: venue.contract,
                selector: *selector,
            }),
            CustomEffect::Opaque => Err(Refusal::UndecodedCustomCall {
                target: venue.contract,
                selector: *selector,
            }),
        },
    }
}

/// The shape of a decoded call, as a settlement key names it.
fn call_shape(call: &ExternalCall) -> Option<SettlementCall> {
    match call {
        ExternalCall::MarketplaceAccept { coupons, .. } if coupons.is_empty() => {
            Some(SettlementCall::Accept)
        }
        ExternalCall::MarketplaceAccept { .. } => Some(SettlementCall::AcceptWithCoupon),
        ExternalCall::CollectionStoreBuy { .. } => Some(SettlementCall::StoreBuy),
        ExternalCall::LegacyExecuteOrder {
            venue,
            nft,
            token_id,
            ..
        } => Some(SettlementCall::One {
            asset: AssetId::token(venue.chain_id, *nft, *token_id),
        }),
        ExternalCall::NameRegister {
            registrar, label, ..
        }
        | ExternalCall::Custom {
            effect: CustomEffect::NameRegistration { registrar, label },
            ..
        } => Some(SettlementCall::One {
            asset: AssetId::name(*registrar, label),
        }),
        ExternalCall::Custom { .. } => None,
    }
}

/// The call settles exactly the group's lines, on their contracts, through
/// the call their settlement key names (a plain accept never settles a
/// discounted line, nor the reverse).
fn settles(
    lines: &[Authoritative],
    call: &ExternalCall,
    method: PaymentMethod,
) -> Result<(), Refusal> {
    same_lines(credit_settled(call)?, expected_lines(lines, method)?)?;
    let shape = call_shape(call);
    for line in lines {
        if Some(settlement_key(line, method)?.call) != shape {
            return Err(Refusal::WrongCall { asset: line.asset });
        }
    }
    Ok(())
}

fn same_lines(
    settled: Vec<(AssetId, ContractRef)>,
    expected: Vec<(AssetId, ContractRef)>,
) -> Result<(), Refusal> {
    let mut remaining = expected;
    for entry in settled {
        match remaining.iter().position(|line| *line == entry) {
            Some(at) => {
                remaining.swap_remove(at);
            }
            None => return Err(Refusal::AssetNotInGroup { asset: entry.0 }),
        }
    }
    match remaining.first() {
        Some((asset, _)) => Err(Refusal::LineNotSettled { asset: *asset }),
        None => Ok(()),
    }
}

fn expected_lines(
    lines: &[Authoritative],
    method: PaymentMethod,
) -> Result<Vec<(AssetId, ContractRef)>, Refusal> {
    lines
        .iter()
        .map(|line| {
            let venue = line.venue(method).ok_or(Refusal::CreditsNotAllowed {
                asset: line.asset,
                sale: line.sale,
                kind: line.kind,
            })?;
            Ok((line.asset, venue))
        })
        .collect()
}

pub fn guard_credit_spend(line: &Authoritative, op: &SettlementOperation) -> Result<(), Refusal> {
    guard_group_credit_spend(std::slice::from_ref(line), op)
}

/// Refuses any operation that would spend credits on something other than
/// exactly the group's credit-eligible lines. An operation that spends no
/// credit (a direct MANA call, a zero broker debit) passes untouched; a
/// negative broker debit, which would credit the ledger, is refused.
pub fn guard_group_credit_spend(
    lines: &[Authoritative],
    op: &SettlementOperation,
) -> Result<(), Refusal> {
    match op {
        SettlementOperation::Mana { .. } => return Ok(()),
        SettlementOperation::LedgerBroker { debit, .. } if debit.get() < 0 => {
            return Err(Refusal::NegativeDebit { debit: *debit });
        }
        SettlementOperation::LedgerBroker { debit, .. } if debit.get() == 0 => return Ok(()),
        _ => {}
    }
    if lines.is_empty() {
        return Err(Refusal::EmptyGroup);
    }
    if lines.len() > MAX_CREDIT_LINES_PER_GROUP {
        return Err(Refusal::GroupTooLarge {
            lines: lines.len(),
            max: MAX_CREDIT_LINES_PER_GROUP,
        });
    }
    let listing = matches!(op, SettlementOperation::CreditListing { .. });
    for line in lines {
        if !listing && line.session == Session::SignedOut {
            return Err(Refusal::NotSignedIn);
        }
        if !eligibility(line).allows(PaymentMethod::Credits) {
            return Err(Refusal::CreditsNotAllowed {
                asset: line.asset,
                sale: line.sale,
                kind: line.kind,
            });
        }
    }
    match op {
        SettlementOperation::AuthorizeCredit {
            chain_id,
            value,
            sizing,
        } => {
            if !chain_id.is_polygon() {
                return Err(Refusal::CreditsChainUnsupported {
                    chain_id: *chain_id,
                });
            }
            let key = settlement_key(&lines[0], PaymentMethod::Credits)?;
            for line in &lines[1..] {
                if settlement_key(line, PaymentMethod::Credits)? != key {
                    return Err(Refusal::MixedGroup);
                }
            }
            if key.chain_id != *chain_id {
                return Err(Refusal::WrongChain {
                    expected: *chain_id,
                    got: key.chain_id,
                });
            }
            let cap = credit_cap(lines, sizing)?;
            if *value > cap {
                return Err(Refusal::CreditValueTooLarge { value: *value, cap });
            }
            Ok(())
        }
        SettlementOperation::UseCredits {
            chain_id,
            max_uncredited,
            call,
        } => {
            if !chain_id.is_polygon() {
                return Err(Refusal::CreditsChainUnsupported {
                    chain_id: *chain_id,
                });
            }
            let got = call.venue().chain_id;
            if got != *chain_id {
                return Err(Refusal::WrongChain {
                    expected: *chain_id,
                    got,
                });
            }
            if !max_uncredited.is_zero()
                && let Some(line) = lines
                    .iter()
                    .find(|line| !eligibility(line).allows(PaymentMethod::Mana))
            {
                return Err(Refusal::ManaRemainderNotAllowed { asset: line.asset });
            }
            settles(lines, call, PaymentMethod::Credits)
        }
        SettlementOperation::LedgerBroker { call, .. } => settles(lines, call, PaymentMethod::Mana),
        SettlementOperation::CreditListing { trade } => {
            let call = ExternalCall::MarketplaceAccept {
                venue: trade.venue,
                trades: vec![trade.clone()],
                coupons: Vec::new(),
            };
            same_lines(
                credit_settled(&call)?,
                expected_lines(lines, PaymentMethod::Credits)?,
            )
        }
        SettlementOperation::Mana { .. } => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EvmAddress;

    fn account() -> EvmAddress {
        EvmAddress::parse("0x00000000000000000000000000000000000000a1").unwrap()
    }

    fn sessions() -> [Session; 3] {
        [
            Session::SignedOut,
            Session::Web2 { account: account() },
            Session::Web3 { wallet: account() },
        ]
    }

    /// Every AssetKind x SaleKind x session, on Polygon and on Ethereum, single
    /// and bundled, against the table in SHOP.md.
    #[test]
    fn the_rule_table_is_exhaustive() {
        use PaymentMethod::{Credits, Mana};
        for kind in AssetKind::ALL {
            for sale in SaleKind::ALL {
                for chain_id in [ChainId::POLYGON, ChainId::ETHEREUM] {
                    for bundle in [false, true] {
                        for session in sessions() {
                            let eligible = !bundle
                                && match (sale, kind) {
                                    (
                                        SaleKind::PrimaryMint,
                                        AssetKind::Wearable | AssetKind::Emote,
                                    ) => chain_id == ChainId::POLYGON,
                                    (SaleKind::PrimaryNameRegistration, AssetKind::Name) => true,
                                    _ => false,
                                };
                            let got = present(
                                restrictions(kind, sale, chain_id, bundle),
                                session,
                                chain_id,
                            );
                            let case =
                                format!("{kind:?} {sale:?} {chain_id} bundle={bundle} {session:?}");
                            if eligible {
                                assert_eq!(got.default, Some(Credits), "{case}");
                                assert_eq!(got.guidance, None, "{case}");
                                let want = if session.has_wallet() {
                                    vec![Credits, Mana]
                                } else {
                                    vec![Credits]
                                };
                                assert_eq!(got.allowed, want, "{case}");
                            } else {
                                assert_eq!(got.allowed, vec![Mana], "{case}");
                                assert_eq!(got.default, Some(Mana), "{case}");
                                assert!(!got.reasons.is_empty(), "{case}");
                                assert!(
                                    !got.reasons.iter().any(|r| matches!(
                                        r,
                                        Reason::CreditsFirst
                                            | Reason::ManaSecond
                                            | Reason::SignInForCredits
                                    )),
                                    "{case}: a MANA-only line never mentions credits"
                                );
                                assert_eq!(got.guidance.is_some(), !session.has_wallet(), "{case}");
                                if matches!(
                                    kind,
                                    AssetKind::Parcel | AssetKind::Estate | AssetKind::Rental
                                ) {
                                    assert!(got.reasons.contains(&Reason::LandOrRental), "{case}");
                                }
                                if sale == SaleKind::Secondary {
                                    assert!(got.reasons.contains(&Reason::SecondarySale), "{case}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn web2_guidance_names_the_network_gas_and_account() {
        let got = present(
            vec![Reason::SecondarySale],
            Session::Web2 { account: account() },
            ChainId::ETHEREUM,
        );
        assert_eq!(
            got.guidance,
            Some(WalletGuidance {
                chain_id: ChainId::ETHEREUM,
                native_gas_required: true,
                account: Some(account())
            })
        );
        let got = present(
            vec![Reason::LandOrRental],
            Session::SignedOut,
            ChainId::POLYGON,
        );
        assert_eq!(
            got.guidance,
            Some(WalletGuidance {
                chain_id: ChainId::POLYGON,
                native_gas_required: false,
                account: None
            })
        );
    }
}
