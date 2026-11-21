//! A third-party publication's slot cheque: a provider manager's EIP-712
//! `ConsumeSlots(thirdPartyId, qty, salt)` signature under the Polygon Third
//! Party Registry's domain, which the committee's registry transaction
//! consumes. This reads one from a request, normalizes it and recovers its
//! signer; nothing here signs.

use alloy_primitives::{Address, B256, U256, address};
use dcl_auth::eip712::{
    domain_separator_salted, hash_dynamic, struct_hash, typed_data_digest, word_u256,
};
use serde::Serialize;
use serde_json::Value;

/// The Third Party Registry on Polygon mainnet, the cheque's verifying
/// contract.
pub const REGISTRY: Address = address!("1C436C1EFb4608dFfDC8bace99d2B03c314f3348");
/// Polygon mainnet, which the registry domain's `salt` carries.
pub const CHAIN_ID: u64 = 137;
const NAME: &str = "Decentraland Third Party Registry";
const VERSION: &str = "1";
const CONSUME_SLOTS: &str = "ConsumeSlots(string thirdPartyId,uint256 qty,bytes32 salt)";

/// A cheque as stored and answered: `salt` lowercase hex of 32 bytes,
/// `signature` lowercase hex of 65 bytes with `v` 27 or 28.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct Cheque {
    pub qty: i32,
    pub salt: String,
    pub signature: String,
}

/// A request's `cheque`, field by field: a quantity that is a positive
/// integer within `i32`, and a salt and signature of the right shape.
#[derive(Clone, Debug, Default)]
pub struct Offer {
    pub qty: Option<i32>,
    salt: Option<B256>,
    signature: Option<[u8; 65]>,
}

impl Offer {
    /// Reads `value`'s fields; anything of another shape is absent.
    pub fn parse(value: &Value) -> Self {
        Self {
            qty: value
                .get("qty")
                .and_then(Value::as_u64)
                .filter(|qty| *qty > 0)
                .and_then(|qty| i32::try_from(qty).ok()),
            salt: value.get("salt").and_then(Value::as_str).and_then(salt),
            signature: value
                .get("signature")
                .and_then(Value::as_str)
                .and_then(signature),
        }
    }

    /// The wallet that signed this cheque for the provider `third_party_id`,
    /// lowercased; why not when the salt or signature can't be read or
    /// recovered.
    pub fn signer(&self, third_party_id: &str) -> Result<String, &'static str> {
        let qty = self.qty.ok_or("invalid quantity")?;
        let salt = self.salt.ok_or("invalid salt")?;
        let signature = self.signature.ok_or("invalid signature")?;
        dcl_auth::recover_address_from_digest(
            &digest(third_party_id, qty, salt),
            &format!("0x{}", hex::encode(signature)),
        )
        .map(|address| address.to_ascii_lowercase())
        .map_err(|_| "invalid signature")
    }

    /// The cheque as stored, once every field reads.
    pub fn stored(&self) -> Option<Cheque> {
        Some(Cheque {
            qty: self.qty?,
            salt: format!("{:#x}", self.salt?),
            signature: format!("0x{}", hex::encode(self.signature?)),
        })
    }
}

fn salt(raw: &str) -> Option<B256> {
    let digits = raw.strip_prefix("0x")?;
    (digits.len() == 64 && digits.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| digits.parse().ok())
        .flatten()
}

/// A 65-byte signature with `v` as 27 or 28: from 65 bytes whose `v` is 0, 1,
/// 27 or 28, or from a 64-byte EIP-2098 compact signature.
fn signature(raw: &str) -> Option<[u8; 65]> {
    let digits = raw.strip_prefix("0x")?;
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let bytes = hex::decode(digits).ok()?;
    let mut out = [0u8; 65];
    match bytes.len() {
        65 => {
            out[..64].copy_from_slice(&bytes[..64]);
            out[64] = match bytes[64] {
                0 | 27 => 27,
                1 | 28 => 28,
                _ => return None,
            };
        }
        64 => {
            out[..64].copy_from_slice(&bytes);
            out[64] = 27 + (bytes[32] >> 7);
            out[32] &= 0x7f;
        }
        _ => return None,
    }
    Some(out)
}

/// The registry domain's separator.
pub fn domain() -> [u8; 32] {
    domain_separator_salted(
        NAME,
        VERSION,
        REGISTRY,
        U256::from(CHAIN_ID).to_be_bytes::<32>(),
    )
}

/// The EIP-712 digest a manager signs: also the id the registry gives the
/// receipt of a consumed cheque.
pub fn digest(third_party_id: &str, qty: i32, salt: B256) -> [u8; 32] {
    typed_data_digest(
        domain(),
        struct_hash(
            hash_dynamic(CONSUME_SLOTS.as_bytes()),
            &[
                hash_dynamic(third_party_id.as_bytes()),
                word_u256(U256::from(qty.max(0) as u64)),
                salt.0,
            ],
        ),
    )
}

/// The receipt id of the stored cheque `cheque` of `third_party_id`:
/// lowercase `0x` hex; none when the stored salt doesn't read.
pub fn receipt_id(third_party_id: &str, cheque: &Cheque) -> Option<String> {
    let salt = salt(&cheque.salt)?;
    Some(format!(
        "0x{}",
        hex::encode(digest(third_party_id, cheque.qty, salt))
    ))
}

#[cfg(test)]
mod tests;
