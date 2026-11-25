//! The fee of a standard collection's publication, for a payer that co-signs
//! the collection manager's `createCollection` call instead of the creator
//! paying it in MANA (the shop's credits). [`for_call`] takes the call's
//! arguments and answers the collection they publish and what it costs, or
//! refuses: the arguments are the canonical encoding of a call through the
//! node's chain's forwarder and factory ([`crate::network`]) that names the
//! signer as creator and holds 1 to [`MAX_ITEMS`] items; the address those
//! arguments deploy at ([`contract::address`], which commits to the salt,
//! name, symbol, token metadata base, creator and items) is the one a
//! collection row of the signer's holds; [`access::publisher`] grants the
//! signer its publication; and the collection is neither published,
//! deployed nor being published by the node. Its owner's lock does not refuse
//! it: a first transaction may have reverted.
//!
//! The fee is what the collection manager charges: each item's rarity price
//! in USD ([`crate::rarities`]), summed and rounded up to a cent.

use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use uuid::Uuid;

use crate::SharedState;
use crate::access;
use crate::chain_state;
use crate::collections::contract::{self, InitializeItem, initializeCall};
use crate::collections::lifecycle::deployed;
use crate::http::{ApiError, Subject};
use crate::publication::{MAX_ITEMS, refuse_before_snapshot};
use crate::publication_chain::createCollectionCall;
use crate::shared::signed_fetch::Signer;

/// A collection's publication and its fee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationFee {
    pub collection: Uuid,
    /// The address the collection deploys at, lowercase.
    pub contract: String,
    pub items: usize,
    pub usd_cents: i64,
}

const WEI_PER_USD_CENT: u64 = 10_000_000_000_000_000;

fn not_a_publication() -> ApiError {
    ApiError::bad_request("The call is not a publication of a collection on this node's chain")
}

/// The publication `data` (the `createCollection` arguments, no selector)
/// is for `signer`, and its fee.
pub async fn for_call(
    state: &SharedState,
    signer: &Signer,
    data: &[u8],
) -> Result<PublicationFee, ApiError> {
    let network = &state.settings.network;
    let call = createCollectionCall::abi_decode_raw(data)
        .ok()
        .filter(|call| call.abi_encode().get(4..) == Some(data))
        .ok_or_else(not_a_publication)?;
    let address = |text: &str| text.parse::<Address>().map_err(|_| not_a_publication());
    if call.forwarder != address(network.forwarder)?
        || call.factory != address(network.collection_factory)?
    {
        return Err(not_a_publication());
    }
    if call.creator != address(signer.as_str())? {
        return Err(ApiError::forbidden(
            "The publication names another wallet as its creator",
        ));
    }
    if !(1..=MAX_ITEMS).contains(&call.items.len()) {
        return Err(ApiError::bad_request(
            "Publish a collection containing 1 to 50 items",
        ));
    }
    let initialize = initializeCall {
        name: call.name.clone(),
        symbol: call.symbol.clone(),
        baseURI: call.baseURI.clone(),
        creator: call.creator,
        shouldComplete: true,
        isApproved: false,
        rarities: address(network.rarities_with_oracle)?,
        items: call
            .items
            .iter()
            .map(|item| InitializeItem {
                rarity: item.rarity.clone(),
                price: item.price,
                beneficiary: item.beneficiary,
                metadata: item.metadata.clone(),
            })
            .collect(),
    };
    let contract = contract::address(network, call.salt, &initialize.abi_encode());
    let id: Uuid = sqlx::query_scalar(
        "SELECT id FROM builder.collections WHERE contract_address = $1 AND eth_address = $2",
    )
    .bind(&contract)
    .bind(signer.as_str())
    .fetch_optional(state.items.pool())
    .await?
    .ok_or_else(|| ApiError::not_found("No collection of yours publishes with this call"))?;
    let publisher = access::publisher(state, signer, id).await?;
    let manage = access::manage_collection(state, signer, &id.to_string()).await?;
    if deployed(state, &contract).await? {
        return Err(ApiError::bad_request(
            "Collection has already been published",
        ));
    }
    let mut conn = state.items.pool().acquire().await?;
    let view = chain_state::for_decision(&mut conn).await?;
    refuse_before_snapshot(&mut conn, &view, publisher.id(), publisher.signer()).await?;
    let stored = access::confirm_collection_manage(&mut conn, &view, &manage).await?;
    drop(conn);
    if stored.pending {
        return Err(ApiError::locked(
            Subject::Collection,
            &id.to_string(),
            "published",
        ));
    }
    let rarities = state.rarities.all(state).await?;
    let mut usd = U256::ZERO;
    for item in &call.items {
        let price = rarities
            .iter()
            .find(|rarity| rarity.name == item.rarity)
            .and_then(|rarity| U256::from_str_radix(&rarity.prices.usd, 10).ok())
            .ok_or_else(|| ApiError::bad_request(format!("Unknown rarity {}", item.rarity)))?;
        usd = usd
            .checked_add(price)
            .ok_or_else(|| ApiError::bad_request("The publication fee is out of range"))?;
    }
    let usd_cents = i64::try_from(usd.div_ceil(U256::from(WEI_PER_USD_CENT)))
        .map_err(|_| ApiError::bad_request("The publication fee is out of range"))?;
    Ok(PublicationFee {
        collection: id,
        contract,
        items: call.items.len(),
        usd_cents,
    })
}
