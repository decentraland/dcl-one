//! The federation's signed actions: a typed message a wallet signs for a
//! domain (`DecentralandPlaces`, `DecentralandEvents`, ...), with a 16-byte
//! nonce and the signing second, verified by recovering the signer from the
//! digest (`\x19\x01 || sha256(domain) || encode_struct || nonce ||
//! signed_at`, EIP-191 personal-sign over it, as the old crate did: the
//! "EIP-712" name is historical, the digest is sha256). A signed body is
//! accepted inside the old skew window (30 s ahead, 5 min behind) and at
//! most once per signer and nonce (the domain's `seen_nonces` table, kept
//! for the window plus a minute and swept by a periodic job).
//!
//! The domains keep their own logs (`explore.signed_actions` for places and
//! events) and serve them as the old `/federation/{scope}/{snapshot,changes}`
//! feeds. The old crate's NATS gossip transport and peer consumer were off
//! unless `FED_GOSSIP=nats` and a peer file with pinned roots were
//! configured, which no deployment did; peers reconcile by pulling the feeds.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

pub const MAX_SKEW_FUTURE_SECS: i64 = 30;
pub const MAX_SKEW_PAST_SECS: i64 = 5 * 60;
const REPLAY_RETENTION_SLACK_SECS: i64 = 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Eip712Domain {
    pub name: String,
    pub version: String,
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    #[serde(rename = "verifyingContract")]
    pub verifying_contract: String,
}

pub trait TypedMessage: Serialize + Clone + std::fmt::Debug {
    const PRIMARY_TYPE: &'static str;
    fn encode_struct(&self) -> Vec<u8>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signed<T: TypedMessage> {
    pub domain: Eip712Domain,
    pub message: T,
    pub nonce: [u8; 16],
    pub signed_at: i64,
    pub signature: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FedError {
    #[error("invalid signature: {0}")]
    InvalidSignature(String),
    #[error("signer mismatch: expected {expected}, recovered {recovered}")]
    SignerMismatch { expected: String, recovered: String },
    #[error("nonce already seen for signer {signer}")]
    DuplicateNonce { signer: String },
    #[error("action timestamp out of window: signed_at={signed_at}, now={now}, skew={skew}s")]
    SkewedTimestamp { signed_at: i64, now: i64, skew: i64 },
    #[error("transport error: {0}")]
    Transport(String),
}

impl<T: TypedMessage> Signed<T> {
    pub fn hash(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"\x19\x01");
        h.update(domain_separator(&self.domain));
        h.update(self.message.encode_struct());
        h.update(self.nonce);
        h.update(self.signed_at.to_be_bytes());
        h.finalize().into()
    }

    pub fn signer(&self) -> Result<String, FedError> {
        decode_signature(&self.signature)?;
        dcl_auth::recover::recover_address(&self.hash(), &self.signature)
            .map_err(|e| FedError::InvalidSignature(e.to_string()))
    }

    pub fn verify(&self, expected_signer: &str, now: i64) -> Result<(), FedError> {
        let skew = now - self.signed_at;
        if !(-MAX_SKEW_FUTURE_SECS..=MAX_SKEW_PAST_SECS).contains(&skew) {
            return Err(FedError::SkewedTimestamp {
                signed_at: self.signed_at,
                now,
                skew,
            });
        }
        let recovered = self.signer()?;
        if !recovered.eq_ignore_ascii_case(expected_signer) {
            return Err(FedError::SignerMismatch {
                expected: expected_signer.to_string(),
                recovered,
            });
        }
        Ok(())
    }

    /// The action's key in a log: the digest in hex.
    pub fn signature_hash(&self) -> String {
        hex::encode(self.hash())
    }
}

fn decode_signature(sig: &str) -> Result<Vec<u8>, FedError> {
    let s = sig.strip_prefix("0x").unwrap_or(sig);
    if s.len() != 130 {
        return Err(FedError::InvalidSignature(format!(
            "expected 132 hex chars (with 0x), got {}",
            sig.len()
        )));
    }
    hex::decode(s).map_err(|e| FedError::InvalidSignature(e.to_string()))
}

fn domain_separator(d: &Eip712Domain) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(d.name.as_bytes());
    h.update(d.version.as_bytes());
    h.update(d.chain_id.to_be_bytes());
    h.update(d.verifying_contract.as_bytes());
    h.finalize().into()
}

pub mod domains {
    use super::Eip712Domain;

    fn domain(name: &str) -> Eip712Domain {
        Eip712Domain {
            name: name.into(),
            version: "1".into(),
            chain_id: 137,
            verifying_contract: "0x0000000000000000000000000000000000000000".into(),
        }
    }

    pub fn places() -> Eip712Domain {
        domain("DecentralandPlaces")
    }

    pub fn events() -> Eip712Domain {
        domain("DecentralandEvents")
    }

    pub fn communities() -> Eip712Domain {
        domain("DecentralandCommunities")
    }

    pub fn market() -> Eip712Domain {
        domain("DecentralandMarket")
    }
}

/// A body is a federation envelope when it carries the three keys of one.
pub fn is_envelope(body: &serde_json::Value) -> bool {
    body.as_object().is_some_and(|o| {
        o.contains_key("domain") && o.contains_key("message") && o.contains_key("signature")
    })
}

/// How long a nonce stays recorded: past the whole window a replay could
/// still pass `verify` in.
pub fn retention_expiry(signed_at: i64) -> i64 {
    signed_at + MAX_SKEW_PAST_SECS + REPLAY_RETENTION_SLACK_SECS
}

/// Records `(signer, nonce)` in `table` (a domain's `seen_nonces`), or
/// refuses a nonce the signer already used.
pub async fn check_and_record(
    pool: &PgPool,
    table: &'static str,
    signer: &str,
    nonce: &[u8; 16],
    signed_at: i64,
) -> Result<(), FedError> {
    let signer = signer.to_ascii_lowercase();
    let sql = format!(
        "INSERT INTO {table} (signer, nonce, expires_at) VALUES ($1, $2, $3) ON CONFLICT (signer, nonce) DO NOTHING"
    );
    let res = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(&signer)
        .bind(hex::encode(nonce))
        .bind(retention_expiry(signed_at))
        .execute(pool)
        .await
        .map_err(|e| FedError::Transport(e.to_string()))?;
    if res.rows_affected() == 0 {
        return Err(FedError::DuplicateNonce { signer });
    }
    Ok(())
}

/// Deletes the nonces whose window closed (`expires_at < now`, unix seconds).
pub async fn sweep_nonces(pool: &PgPool, table: &'static str, now: i64) -> sqlx::Result<u64> {
    let sql = format!("DELETE FROM {table} WHERE expires_at < $1");
    Ok(sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(now)
        .execute(pool)
        .await?
        .rows_affected())
}

pub type Cursor = i64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub seq: i64,
    pub signature_hash: String,
}

pub fn caught_up(cursor: Cursor, watermark: i64) -> bool {
    cursor >= watermark
}

pub fn next_cursor(prev: Cursor, applied: &[Change]) -> Cursor {
    applied
        .iter()
        .map(|c| c.seq)
        .max()
        .unwrap_or(prev)
        .max(prev)
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use super::*;
    use alloy::signers::SignerSync;
    use alloy::signers::local::PrivateKeySigner;

    /// Signs `message` for `domain` with `key` now, the way a client does.
    pub fn sign<T: TypedMessage>(
        key: &PrivateKeySigner,
        domain: Eip712Domain,
        message: T,
        signed_at: i64,
    ) -> Signed<T> {
        let mut signed = Signed {
            domain,
            message,
            nonce: rand::random(),
            signed_at,
            signature: String::new(),
        };
        let sig = key.sign_message_sync(&signed.hash()).unwrap();
        signed.signature = format!("0x{}", hex::encode(sig.as_bytes()));
        signed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Ping {
        id: String,
    }

    impl TypedMessage for Ping {
        const PRIMARY_TYPE: &'static str = "Ping";
        fn encode_struct(&self) -> Vec<u8> {
            let mut out = Self::PRIMARY_TYPE.as_bytes().to_vec();
            out.push(0);
            out.extend_from_slice(self.id.as_bytes());
            out
        }
    }

    #[test]
    fn a_signed_action_verifies_inside_the_window_only() {
        let key = alloy::signers::local::PrivateKeySigner::random();
        let now = 1_700_000_000;
        let signed = test_support::sign(&key, domains::places(), Ping { id: "a".into() }, now);
        let signer = format!("{:#x}", key.address());
        assert_eq!(signed.signer().unwrap(), signer);
        assert!(signed.verify(&signer, now + 10).is_ok());
        assert!(matches!(
            signed.verify(&signer, now + MAX_SKEW_PAST_SECS + 1),
            Err(FedError::SkewedTimestamp { .. })
        ));
        assert!(matches!(
            signed.verify(&signer, now - MAX_SKEW_FUTURE_SECS - 1),
            Err(FedError::SkewedTimestamp { .. })
        ));
        assert!(matches!(
            signed.verify("0x0000000000000000000000000000000000000001", now),
            Err(FedError::SignerMismatch { .. })
        ));
        let mut tampered = signed.clone();
        tampered.message.id = "b".into();
        assert_ne!(
            tampered.signer().unwrap(),
            signer,
            "the message is in the digest"
        );
        let mut short = signed;
        short.signature = "0x00".into();
        assert!(matches!(short.signer(), Err(FedError::InvalidSignature(_))));
    }

    #[test]
    fn retention_outlives_the_verify_acceptance_window() {
        let signed_at = 1_000_000;
        let latest_verify_now = signed_at + MAX_SKEW_PAST_SECS;
        assert!(retention_expiry(signed_at) - latest_verify_now >= MAX_SKEW_FUTURE_SECS);
    }

    #[test]
    fn envelopes_are_recognised_by_their_keys() {
        assert!(is_envelope(
            &serde_json::json!({ "domain": {}, "message": {}, "signature": "0x", "nonce": [], "signed_at": 1 })
        ));
        assert!(!is_envelope(&serde_json::json!({ "favorites": true })));
        assert!(!is_envelope(
            &serde_json::json!({ "domain": {}, "message": {} })
        ));
        assert!(!is_envelope(&serde_json::json!("nope")));
    }

    #[test]
    fn cursor_advances_to_last_seq_and_never_regresses() {
        assert_eq!(next_cursor(0, &[]), 0);
        let page = vec![
            Change {
                seq: 3,
                signature_hash: "a".into(),
            },
            Change {
                seq: 7,
                signature_hash: "b".into(),
            },
        ];
        assert_eq!(next_cursor(0, &page), 7);
        assert_eq!(
            next_cursor(
                7,
                &[Change {
                    seq: 2,
                    signature_hash: "c".into()
                }]
            ),
            7
        );
        assert!(!caught_up(4, 5) && caught_up(5, 5) && caught_up(6, 5) && caught_up(0, 0));
    }
}
