//! The wire types of the rentals listings API (decentraland/signatures-server):
//! the creation body, the listing document and the EIP-712 `Listing` struct
//! the signature is checked over.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RentalListingPeriodInput {
    #[serde(rename = "minDays")]
    pub min_days: i64,
    #[serde(rename = "maxDays")]
    pub max_days: i64,
    #[serde(rename = "pricePerDay")]
    pub price_per_day: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RentalListingCreation {
    pub network: String,
    #[serde(rename = "chainId")]
    pub chain_id: i64,
    /// Milliseconds since the epoch.
    pub expiration: i64,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    pub nonces: Vec<String>,
    pub periods: Vec<RentalListingPeriodInput>,
    #[serde(rename = "rentalContractAddress")]
    pub rental_contract_address: String,
    pub signature: String,
    #[serde(default = "address_zero")]
    pub target: String,
}

pub const ADDRESS_ZERO: &str = "0x0000000000000000000000000000000000000000";

fn address_zero() -> String {
    ADDRESS_ZERO.to_string()
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct RentalListingPeriod {
    #[serde(rename = "minDays")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub min_days: i64,
    #[serde(rename = "maxDays")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub max_days: i64,
    #[serde(rename = "pricePerDay")]
    pub price_per_day: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct RentalListing {
    pub id: String,
    #[serde(rename = "nftId")]
    pub nft_id: String,
    pub category: String,
    #[serde(rename = "searchText")]
    pub search_text: String,
    pub network: String,
    #[serde(rename = "chainId")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub chain_id: i64,
    /// Milliseconds since the epoch.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub expiration: i64,
    pub signature: String,
    pub nonces: Vec<String>,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "rentalContractAddress")]
    pub rental_contract_address: String,
    pub lessor: Option<String>,
    pub tenant: Option<String>,
    pub status: String,
    #[serde(rename = "createdAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub updated_at: i64,
    #[serde(rename = "startedAt")]
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub started_at: Option<i64>,
    pub periods: Vec<RentalListingPeriod>,
    pub target: String,
    #[serde(rename = "rentedDays")]
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub rented_days: Option<i64>,
}

/// The `Listing` struct of the Rentals contract, every number as its decimal
/// string (the signature is checked over these, not over the wire body).
#[derive(Debug, Clone)]
pub struct ContractRentalListing {
    pub signer: String,
    pub contract_address: String,
    pub token_id: String,
    /// Seconds since the epoch (the wire's milliseconds / 1000).
    pub expiration: String,
    pub indexes: Vec<String>,
    pub price_per_day: Vec<String>,
    pub max_days: Vec<String>,
    pub min_days: Vec<String>,
    pub signature: String,
    pub target: String,
}

impl ContractRentalListing {
    pub fn from_creation(lessor: &str, r: &RentalListingCreation) -> Self {
        Self {
            signer: lessor.to_string(),
            contract_address: r.contract_address.clone(),
            token_id: r.token_id.clone(),
            expiration: (r.expiration / 1000).to_string(),
            indexes: r.nonces.clone(),
            price_per_day: r.periods.iter().map(|p| p.price_per_day.clone()).collect(),
            max_days: r.periods.iter().map(|p| p.max_days.to_string()).collect(),
            min_days: r.periods.iter().map(|p| p.min_days.to_string()).collect(),
            signature: r.signature.clone(),
            target: r.target.clone(),
        }
    }
}
