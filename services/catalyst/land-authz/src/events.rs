//! The log decoder: one `eth_getLogs` row of the LAND or Estate registry into
//! an [`AuthzEvent`]. Seven topics: the five the old crate indexed (the
//! operator, update-operator, update-manager and approved-for-all legs, and
//! `Transfer`, which clears the per-token legs and now also settles the
//! owner) plus the Estate registry's `AddLand`/`RemoveLand`, which settle
//! estate membership so the fold needs no squid `parcel.estate_id`.

use alloy_primitives::keccak256;
use std::sync::LazyLock;

use super::{
    KIND_ADD_LAND, KIND_APPROVAL, KIND_APPROVED_FOR_ALL, KIND_REMOVE_LAND, KIND_TRANSFER,
    KIND_UPDATE_MANAGER, KIND_UPDATE_OPERATOR,
};

pub const TOPIC_UPDATE_OPERATOR: &str =
    "0x9d9dd80a56a16f715df6eb40b771e24ff8cbea6eed9de28473ce0f28fe5602a9";
pub const TOPIC_UPDATE_MANAGER: &str =
    "0xd79fbfe1644c022b9150727d871532bfcc3e27ffee86fc596a062770ac97b042";
pub const TOPIC_APPROVAL_FOR_ALL: &str =
    "0x17307eab39ab6107e8899845ad3d59bd9653f200f220920489ca2b5937696c31";
pub const TOPIC_APPROVAL: &str =
    "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925";

/// `Transfer(address indexed, address indexed, uint256 indexed)`: the ERC-721
/// form the Estate registry emits.
pub const TOPIC_TRANSFER_3: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
/// The LAND registry's two pre-standard `Transfer` overloads (five and six
/// parameters); topics 1..3 are still `from`, `to`, `assetId`.
pub const TOPIC_TRANSFER_5: &str =
    "0xd5c97f2e041b2046be3b4337472f05720760a198f4d7d84980b7155eec7cca6f";
pub const TOPIC_TRANSFER_6: &str =
    "0x8988d59efc2c4547ef86c88f6543963bab0cea94f8e486e619c7c3a790db93be";

/// `AddLand(uint256 indexed _estateId, uint256 indexed _landId)` (Estate registry).
pub const TOPIC_ADD_LAND: &str =
    "0xff0e52667d53255667dc777a00af81038a4646367b0d73d8ee8540ca5b0c9a2e";
/// `RemoveLand(uint256 indexed _estateId, uint256 indexed _landId, address indexed _destinatary)`.
pub const TOPIC_REMOVE_LAND: &str =
    "0x7932eb5ab0d4d4d172776074ee15d13d708465ff5476902ed15a4965434fcab1";

pub const SIGNATURE_ADD_LAND: &str = "AddLand(uint256,uint256)";
pub const SIGNATURE_REMOVE_LAND: &str = "RemoveLand(uint256,uint256,address)";

pub const ALL_TOPICS: [&str; 9] = [
    TOPIC_UPDATE_OPERATOR,
    TOPIC_UPDATE_MANAGER,
    TOPIC_APPROVAL_FOR_ALL,
    TOPIC_APPROVAL,
    TOPIC_TRANSFER_3,
    TOPIC_TRANSFER_5,
    TOPIC_TRANSFER_6,
    TOPIC_ADD_LAND,
    TOPIC_REMOVE_LAND,
];

/// The two Estate topics computed from their signatures at first use; the
/// constants above are what the decoder matches, and a unit test pins the two
/// to each other.
pub static ESTATE_TOPICS: LazyLock<(String, String)> = LazyLock::new(|| {
    (
        format!("{:#x}", keccak256(SIGNATURE_ADD_LAND)),
        format!("{:#x}", keccak256(SIGNATURE_REMOVE_LAND)),
    )
});

/// One row of `land_authz.authz_event`. `token_id` is the decimal text of the
/// asset (the LAND id for `add_land`/`remove_land`, whose `estate_id` is the
/// estate); `account` is the owner for `approval`/`update_manager`/
/// `approved_for_all`, `to` for `transfer`, the destinatary for
/// `remove_land`; `operator` the grantee; `approved` the flag of the two
/// account-wide legs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthzEvent {
    pub block_number: i64,
    pub log_index: i32,
    pub block_time: i64,
    pub token_address: String,
    pub kind: &'static str,
    pub token_id: Option<String>,
    pub estate_id: Option<String>,
    pub account: Option<String>,
    pub operator: Option<String>,
    pub approved: Option<bool>,
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("log has no topic0")]
    NoTopic,
    #[error("unhandled topic {0}")]
    UnhandledTopic(String),
    #[error("topic {topic} needs {want} topics, log has {got}")]
    TopicArity {
        topic: &'static str,
        want: usize,
        got: usize,
    },
    #[error("could not read a {field} from the log")]
    Field { field: &'static str },
}

fn addr_from_topic(topic: &str) -> Option<String> {
    let hex = topic.strip_prefix("0x")?;
    if hex.len() != 64 {
        return None;
    }
    Some(format!("0x{}", &hex[24..]).to_lowercase())
}

fn u256_topic_to_decimal(topic: &str) -> Option<String> {
    let hex = topic.strip_prefix("0x")?;
    if hex.len() != 64 {
        return None;
    }
    let mut digits: Vec<u8> = vec![0];
    for c in hex.chars() {
        let nib = c.to_digit(16)?;
        let mut carry = nib;
        for d in digits.iter_mut() {
            let v = (*d as u32) * 16 + carry;
            *d = (v % 10) as u8;
            carry = v / 10;
        }
        while carry > 0 {
            digits.push((carry % 10) as u8);
            carry /= 10;
        }
    }
    while digits.len() > 1 && *digits.last().unwrap() == 0 {
        digits.pop();
    }
    Some(digits.iter().rev().map(|d| (b'0' + d) as char).collect())
}

fn first_word_bool(data: &str) -> Option<bool> {
    let hex = data.strip_prefix("0x")?;
    if hex.len() < 64 {
        return None;
    }
    Some(hex[..64].chars().any(|c| c != '0'))
}

pub fn decode_log(
    address: &str,
    topics: &[String],
    data: &str,
    block_number: i64,
    log_index: i32,
    block_time: i64,
) -> Result<AuthzEvent, DecodeError> {
    let topic0 = topics.first().ok_or(DecodeError::NoTopic)?.to_lowercase();
    let token_address = address.to_lowercase();
    let need = |want: usize, topic: &'static str| -> Result<(), DecodeError> {
        if topics.len() < want {
            return Err(DecodeError::TopicArity {
                topic,
                want,
                got: topics.len(),
            });
        }
        Ok(())
    };
    let base = |kind: &'static str| AuthzEvent {
        block_number,
        log_index,
        block_time,
        token_address: token_address.clone(),
        kind,
        token_id: None,
        estate_id: None,
        account: None,
        operator: None,
        approved: None,
    };
    let uint = |i: usize, field: &'static str| {
        u256_topic_to_decimal(&topics[i]).ok_or(DecodeError::Field { field })
    };
    let addr = |i: usize, field: &'static str| {
        addr_from_topic(&topics[i]).ok_or(DecodeError::Field { field })
    };

    match topic0.as_str() {
        TOPIC_UPDATE_OPERATOR => {
            need(3, "UpdateOperator")?;
            Ok(AuthzEvent {
                token_id: Some(uint(1, "assetId")?),
                operator: Some(addr(2, "operator")?),
                ..base(KIND_UPDATE_OPERATOR)
            })
        }
        TOPIC_APPROVAL => {
            need(4, "Approval")?;
            Ok(AuthzEvent {
                token_id: Some(uint(3, "assetId")?),
                account: addr_from_topic(&topics[1]),
                operator: Some(addr(2, "operator")?),
                ..base(KIND_APPROVAL)
            })
        }
        TOPIC_TRANSFER_3 | TOPIC_TRANSFER_5 | TOPIC_TRANSFER_6 => {
            need(4, "Transfer")?;
            Ok(AuthzEvent {
                token_id: Some(uint(3, "assetId")?),
                account: addr_from_topic(&topics[2]),
                ..base(KIND_TRANSFER)
            })
        }
        TOPIC_UPDATE_MANAGER => {
            need(4, "UpdateManager")?;
            Ok(AuthzEvent {
                account: Some(addr(1, "_owner")?),
                operator: Some(addr(2, "_operator")?),
                approved: Some(
                    first_word_bool(data).ok_or(DecodeError::Field { field: "_approved" })?,
                ),
                ..base(KIND_UPDATE_MANAGER)
            })
        }
        TOPIC_APPROVAL_FOR_ALL => {
            need(3, "ApprovalForAll")?;
            Ok(AuthzEvent {
                account: Some(addr(1, "holder")?),
                operator: Some(addr(2, "operator")?),
                approved: Some(first_word_bool(data).ok_or(DecodeError::Field {
                    field: "authorized",
                })?),
                ..base(KIND_APPROVED_FOR_ALL)
            })
        }
        TOPIC_ADD_LAND => {
            need(3, "AddLand")?;
            Ok(AuthzEvent {
                estate_id: Some(uint(1, "_estateId")?),
                token_id: Some(uint(2, "_landId")?),
                ..base(KIND_ADD_LAND)
            })
        }
        TOPIC_REMOVE_LAND => {
            need(4, "RemoveLand")?;
            Ok(AuthzEvent {
                estate_id: Some(uint(1, "_estateId")?),
                token_id: Some(uint(2, "_landId")?),
                account: addr_from_topic(&topics[3]),
                ..base(KIND_REMOVE_LAND)
            })
        }
        other => Err(DecodeError::UnhandledTopic(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ESTATE_REGISTRY_MAINNET, LAND_REGISTRY_MAINNET};

    const ZERO: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";
    const ONE: &str = "0x0000000000000000000000000000000000000000000000000000000000000001";

    fn topic_addr(a: &str) -> String {
        format!("0x{:0>64}", a.trim_start_matches("0x"))
    }

    #[test]
    fn the_estate_topics_are_the_keccak_of_their_signatures() {
        let (add, remove) = &*ESTATE_TOPICS;
        assert_eq!(
            (add.as_str(), remove.as_str()),
            (TOPIC_ADD_LAND, TOPIC_REMOVE_LAND),
            "the constants must be keccak256({SIGNATURE_ADD_LAND:?}) and keccak256({SIGNATURE_REMOVE_LAND:?})"
        );
        assert_eq!(
            format!("{:#x}", keccak256("Transfer(address,address,uint256)")),
            TOPIC_TRANSFER_3
        );
        assert_eq!(
            format!("{:#x}", keccak256("ApprovalForAll(address,address,bool)")),
            TOPIC_APPROVAL_FOR_ALL
        );
    }

    #[test]
    fn address_is_the_low_twenty_bytes() {
        assert_eq!(
            addr_from_topic(&topic_addr("0x4959f54f7b30224047ac3cae6bf147bbaee8b61c")).unwrap(),
            "0x4959f54f7b30224047ac3cae6bf147bbaee8b61c"
        );
    }

    #[test]
    fn u256_topic_decimal_conversion() {
        assert_eq!(u256_topic_to_decimal(ZERO).unwrap(), "0");
        assert_eq!(u256_topic_to_decimal(ONE).unwrap(), "1");
        let big = format!("0x{:0>64}", "de0b6b3a7640000");
        assert_eq!(u256_topic_to_decimal(&big).unwrap(), "1000000000000000000");
    }

    #[test]
    fn bool_word_is_any_nonzero_nibble() {
        assert!(!first_word_bool(ZERO).unwrap());
        assert!(first_word_bool(ONE).unwrap());
    }

    #[test]
    fn update_operator_decodes_asset_and_operator() {
        let ev = decode_log(
            LAND_REGISTRY_MAINNET,
            &[
                TOPIC_UPDATE_OPERATOR.to_string(),
                ONE.to_string(),
                topic_addr("0xabc0000000000000000000000000000000000001"),
            ],
            "0x",
            100,
            2,
            1700,
        )
        .unwrap();
        assert_eq!(ev.kind, KIND_UPDATE_OPERATOR);
        assert_eq!(ev.token_id.as_deref(), Some("1"));
        assert_eq!(
            ev.operator.as_deref(),
            Some("0xabc0000000000000000000000000000000000001")
        );
        assert_eq!(ev.estate_id, None);
    }

    #[test]
    fn approval_for_all_carries_the_flag_from_data() {
        let revoked = decode_log(
            LAND_REGISTRY_MAINNET,
            &[
                TOPIC_APPROVAL_FOR_ALL.to_string(),
                topic_addr("0x1111111111111111111111111111111111111111"),
                topic_addr("0x2222222222222222222222222222222222222222"),
            ],
            ZERO,
            10,
            0,
            5,
        )
        .unwrap();
        assert_eq!(revoked.kind, KIND_APPROVED_FOR_ALL);
        assert_eq!(revoked.approved, Some(false));
    }

    #[test]
    fn estate_update_manager_grant_decodes_from_four_topic_log() {
        let owner = "0x1e4dabf2fc73b6d76956a528de8ffaa2796641f0";
        let manager = "0xe7f78d2c9a9375153476834d2db32632384b01e1";
        let grant = decode_log(
            ESTATE_REGISTRY_MAINNET,
            &[
                TOPIC_UPDATE_MANAGER.to_string(),
                topic_addr(owner),
                topic_addr(manager),
                topic_addr(owner),
            ],
            ONE,
            100,
            2,
            1700,
        )
        .unwrap();
        assert_eq!(grant.token_address, ESTATE_REGISTRY_MAINNET);
        assert_eq!(grant.kind, KIND_UPDATE_MANAGER);
        assert_eq!(grant.account.as_deref(), Some(owner));
        assert_eq!(grant.operator.as_deref(), Some(manager));
        assert_eq!(grant.approved, Some(true));
    }

    #[test]
    fn a_transfer_settles_the_new_owner_in_account() {
        let to = "0x4959f54f7b30224047ac3cae6bf147bbaee8b61c";
        let ev = decode_log(
            LAND_REGISTRY_MAINNET,
            &[
                TOPIC_TRANSFER_5.to_string(),
                ZERO.to_string(),
                topic_addr(to),
                ONE.to_string(),
            ],
            "0x",
            7,
            1,
            0,
        )
        .unwrap();
        assert_eq!(
            (ev.kind, ev.token_id.as_deref(), ev.account.as_deref()),
            (KIND_TRANSFER, Some("1"), Some(to))
        );
    }

    #[test]
    fn add_and_remove_land_carry_the_estate_and_the_land() {
        let estate = format!("0x{:0>64x}", 500);
        let land = format!("0x{:0>64x}", 12u128);
        let added = decode_log(
            ESTATE_REGISTRY_MAINNET,
            &[TOPIC_ADD_LAND.to_string(), estate.clone(), land.clone()],
            "0x",
            50,
            0,
            0,
        )
        .unwrap();
        assert_eq!(added.kind, KIND_ADD_LAND);
        assert_eq!(
            (added.estate_id.as_deref(), added.token_id.as_deref()),
            (Some("500"), Some("12"))
        );
        let dest = "0xbbbb000000000000000000000000000000000002";
        let removed = decode_log(
            ESTATE_REGISTRY_MAINNET,
            &[
                TOPIC_REMOVE_LAND.to_string(),
                estate,
                land,
                topic_addr(dest),
            ],
            "0x",
            51,
            0,
            0,
        )
        .unwrap();
        assert_eq!(removed.kind, KIND_REMOVE_LAND);
        assert_eq!(removed.account.as_deref(), Some(dest));
        assert_eq!(removed.estate_id.as_deref(), Some("500"));
    }

    #[test]
    fn a_truncated_log_is_an_error_not_a_silent_grant() {
        let err = decode_log(
            LAND_REGISTRY_MAINNET,
            &[TOPIC_UPDATE_OPERATOR.to_string()],
            "0x",
            1,
            0,
            0,
        )
        .unwrap_err();
        assert!(matches!(err, DecodeError::TopicArity { .. }));
        let err = decode_log(
            ESTATE_REGISTRY_MAINNET,
            &[TOPIC_ADD_LAND.to_string(), ONE.to_string()],
            "0x",
            1,
            0,
            0,
        )
        .unwrap_err();
        assert!(matches!(err, DecodeError::TopicArity { .. }));
    }
}
