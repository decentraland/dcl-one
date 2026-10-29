use crate::auth::AuthChain;
use crate::auth::verify_auth_chain;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use thiserror::Error;

use crate::archipelago::config::AuthConfig;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("challenge not found or expired")]
    UnknownChallenge,
    #[error("challenge does not match")]
    ChallengeMismatch,
    #[error("auth chain rejected: {0}")]
    InvalidChain(String),
    #[error("signature too old: {0}s")]
    SignatureTooOld(i64),
    #[error("address mismatch")]
    AddressMismatch,
}

impl AuthError {
    pub fn class(&self) -> &'static str {
        match self {
            Self::UnknownChallenge => "unknown_challenge",
            Self::ChallengeMismatch => "challenge_mismatch",
            Self::InvalidChain(_) => "invalid_chain",
            Self::SignatureTooOld(_) => "signature_too_old",
            Self::AddressMismatch => "address_mismatch",
        }
    }
}

pub const MAX_PENDING_CHALLENGES: usize = 65_536;

type ChallengeKey = (String, String);

#[derive(Clone, Debug)]
struct Issued {
    issued_at: DateTime<Utc>,
    sequence: u64,
}

/// Any socket that opens is handed a challenge, so the pending set is bounded: past
/// `MAX_PENDING_CHALLENGES` the oldest live challenge leaves first. `order` holds issuance
/// order; an entry whose sequence no longer matches the map is a redeemed or discarded
/// challenge and holds no capacity.
#[derive(Default)]
struct Pending {
    by_challenge: HashMap<ChallengeKey, Issued>,
    order: VecDeque<(ChallengeKey, u64)>,
    next_sequence: u64,
}

impl Pending {
    fn pop_oldest(&mut self, expired_before: Option<DateTime<Utc>>) -> bool {
        let Some((key, sequence)) = self.order.front() else {
            return false;
        };
        let live = self
            .by_challenge
            .get(key)
            .filter(|issued| issued.sequence == *sequence);
        if let (Some(issued), Some(cutoff)) = (live, expired_before)
            && issued.issued_at >= cutoff
        {
            return false;
        }
        if live.is_some() {
            self.by_challenge.remove(key);
        }
        self.order.pop_front();
        true
    }
}

/// The challenges the `/ws` sockets are waiting on, each stored under its handshake's
/// key until its signed answer redeems it, it expires or the socket discards it.
pub struct ChallengeStore {
    cfg: AuthConfig,
    pending: Mutex<Pending>,
}

impl ChallengeStore {
    pub fn new(cfg: AuthConfig) -> Arc<Self> {
        Arc::new(Self {
            cfg,
            pending: Mutex::new(Pending::default()),
        })
    }

    pub fn put_for_key(&self, key: &str, challenge: &str) {
        let mut guard = self.pending.lock();
        let pending = &mut *guard;
        let cutoff = self.cutoff();
        while pending.pop_oldest(Some(cutoff)) {}
        while pending.by_challenge.len() >= MAX_PENDING_CHALLENGES && pending.pop_oldest(None) {}
        if pending.order.len() > 2 * MAX_PENDING_CHALLENGES {
            let live = &pending.by_challenge;
            pending.order.retain(|(queued, sequence)| {
                live.get(queued)
                    .is_some_and(|issued| issued.sequence == *sequence)
            });
        }
        let stored = (key.to_ascii_lowercase(), challenge.to_string());
        let sequence = pending.next_sequence;
        pending.next_sequence += 1;
        pending.order.push_back((stored.clone(), sequence));
        pending.by_challenge.insert(
            stored,
            Issued {
                issued_at: Utc::now(),
                sequence,
            },
        );
    }

    pub fn discard_for_key(&self, key: &str, challenge: &str) {
        self.pending
            .lock()
            .by_challenge
            .remove(&(key.to_ascii_lowercase(), challenge.to_string()));
    }

    pub fn redeem_and_verify_payload(
        &self,
        storage_key: &str,
        address: &str,
        challenge: &str,
        expected_payload: &str,
        chain: &AuthChain,
    ) -> Result<(), AuthError> {
        let addr_lc = address.to_ascii_lowercase();
        let key = (storage_key.to_ascii_lowercase(), challenge.to_string());
        let issued = self
            .pending
            .lock()
            .by_challenge
            .get(&key)
            .filter(|issued| issued.issued_at >= self.cutoff())
            .cloned()
            .ok_or(AuthError::UnknownChallenge)?;
        let now = Utc::now();
        let age = now.signed_duration_since(issued.issued_at).num_seconds();
        if age > self.cfg.signature_max_age_secs as i64 {
            return Err(AuthError::SignatureTooOld(age));
        }
        if chain.is_empty() {
            return Err(AuthError::InvalidChain("empty chain".into()));
        }
        if let Some(first) = chain.first()
            && !first.payload.eq_ignore_ascii_case(&addr_lc)
        {
            return Err(AuthError::AddressMismatch);
        }
        let last_payload = chain.last().map(|l| l.payload.clone()).unwrap_or_default();
        if last_payload != expected_payload {
            return Err(AuthError::ChallengeMismatch);
        }

        verify_auth_chain(chain, expected_payload, Some(now.timestamp_millis()))
            .map_err(|e| AuthError::InvalidChain(format!("{:?}", e)))?;
        let mut guard = self.pending.lock();
        match guard.by_challenge.get(&key) {
            Some(current)
                if current.sequence == issued.sequence && current.issued_at >= self.cutoff() =>
            {
                guard.by_challenge.remove(&key);
                Ok(())
            }
            _ => Err(AuthError::UnknownChallenge),
        }
    }

    fn cutoff(&self) -> DateTime<Utc> {
        Utc::now() - chrono::Duration::seconds(self.cfg.challenge_ttl_secs as i64)
    }

    #[cfg(test)]
    fn pending(&self) -> usize {
        self.pending.lock().by_challenge.len()
    }

    #[cfg(test)]
    fn queued(&self) -> usize {
        self.pending.lock().order.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejection_class_never_retains_invalid_chain_details() {
        const CANARY: &str = "archipelago-invalid-chain-private-material-canary";
        let error = AuthError::InvalidChain(CANARY.to_string());
        assert_eq!(error.class(), "invalid_chain");
        assert!(!error.class().contains(CANARY));
    }

    fn cfg() -> AuthConfig {
        AuthConfig {
            challenge_ttl_secs: 120,
            signature_max_age_secs: 300,
            deny_list_url: None,
            ..AuthConfig::default()
        }
    }

    /// A challenge a socket was handed, stored under `key`.
    fn offered(store: &ChallengeStore, key: &str) -> String {
        let challenge = uuid::Uuid::new_v4().simple().to_string();
        store.put_for_key(key, &challenge);
        challenge
    }

    /// The socket stored under `address` redeems `challenge` as its own signed payload.
    fn redeem(
        store: &ChallengeStore,
        address: &str,
        challenge: &str,
        chain: &AuthChain,
    ) -> Result<(), AuthError> {
        store.redeem_and_verify_payload(address, address, challenge, challenge, chain)
    }

    #[test]
    fn redeem_with_unknown_address_fails() {
        let s = ChallengeStore::new(cfg());
        let err = redeem(
            &s,
            "0x0000000000000000000000000000000000000001",
            "deadbeef",
            &vec![],
        )
        .unwrap_err();
        assert!(
            matches!(err, AuthError::UnknownChallenge),
            "expected UnknownChallenge, got {err:?}"
        );
    }

    #[test]
    fn redeem_with_valid_signed_chain_succeeds() {
        use alloy::signers::{SignerSync, local::PrivateKeySigner};

        let s = ChallengeStore::new(cfg());
        let wallet = PrivateKeySigner::random();
        let address = format!("{:#x}", wallet.address());

        let challenge = offered(&s, &address);

        let hash = alloy::primitives::eip191_hash_message(challenge.as_bytes());
        let sig = wallet.sign_hash_sync(&hash).expect("sign");

        let chain: AuthChain = serde_json::from_value(serde_json::json!([
            { "type": "SIGNER", "payload": address, "signature": "" },
            {
                "type": "ECDSA_SIGNED_ENTITY",
                "payload": challenge,
                "signature": sig.to_string()
            }
        ]))
        .expect("chain json");

        redeem(&s, &address, &challenge, &chain).expect("valid signed challenge must verify");
    }

    #[test]
    fn redeem_with_wrong_signer_fails() {
        use alloy::signers::{SignerSync, local::PrivateKeySigner};

        let s = ChallengeStore::new(cfg());
        let wallet = PrivateKeySigner::random();
        let impostor = PrivateKeySigner::random();
        let address = format!("{:#x}", wallet.address());

        let challenge = offered(&s, &address);
        let hash = alloy::primitives::eip191_hash_message(challenge.as_bytes());
        let sig = impostor.sign_hash_sync(&hash).expect("sign");

        let chain: AuthChain = serde_json::from_value(serde_json::json!([
            { "type": "SIGNER", "payload": address, "signature": "" },
            {
                "type": "ECDSA_SIGNED_ENTITY",
                "payload": challenge,
                "signature": sig.to_string()
            }
        ]))
        .expect("chain json");

        redeem(&s, &address, &challenge, &chain).expect_err("impostor signature must be rejected");
    }

    fn signed(signer: &alloy::signers::local::PrivateKeySigner, challenge: &str) -> AuthChain {
        use alloy::signers::SignerSync;
        serde_json::from_value(serde_json::json!([
            { "type": "SIGNER", "payload": format!("{:#x}", signer.address()), "signature": "" },
            { "type": "ECDSA_SIGNED_ENTITY", "payload": challenge,
              "signature": signer.sign_message_sync(challenge.as_bytes()).unwrap().to_string() }
        ]))
        .unwrap()
    }

    #[test]
    fn same_wallet_challenges_survive_mismatch_and_invalid_signature() {
        use alloy::signers::local::PrivateKeySigner;
        let store = ChallengeStore::new(cfg());
        let signer = PrivateKeySigner::random();
        let address = format!("{:#x}", signer.address());
        let first = offered(&store, &address);
        let second = offered(&store, &address.to_uppercase());
        let chain = signed(&signer, &first);
        assert!(matches!(
            redeem(&store, &address, "absent", &chain),
            Err(AuthError::UnknownChallenge)
        ));
        assert!(matches!(
            redeem(&store, &address, &second, &chain),
            Err(AuthError::ChallengeMismatch)
        ));
        assert!(
            redeem(
                &store,
                &address,
                &first,
                &signed(&PrivateKeySigner::random(), &first)
            )
            .is_err()
        );
        redeem(&store, &address, &first, &chain).unwrap();
        redeem(&store, &address, &second, &signed(&signer, &second)).unwrap();
        assert!(matches!(
            redeem(&store, &address, &first, &chain),
            Err(AuthError::UnknownChallenge)
        ));
    }

    #[test]
    fn concurrent_redemptions_commit_exactly_once() {
        use alloy::signers::local::PrivateKeySigner;
        let store = ChallengeStore::new(cfg());
        let signer = PrivateKeySigner::random();
        let address = format!("{:#x}", signer.address());
        let challenge = offered(&store, &address);
        let chain = signed(&signer, &challenge);
        let barrier = std::sync::Barrier::new(8);
        let successes = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        redeem(&store, &address, &challenge, &chain).is_ok()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| usize::from(h.join().unwrap()))
                .sum::<usize>()
        });
        assert_eq!(successes, 1);
    }

    fn numbered(n: usize) -> String {
        format!("0x{n:040x}")
    }

    #[test]
    fn unredeemed_challenges_are_bounded_and_the_oldest_leave_first() {
        let store = ChallengeStore::new(cfg());
        let oldest = offered(&store, &numbered(0));
        let mut newest = String::new();
        for n in 1..=MAX_PENDING_CHALLENGES {
            newest = offered(&store, &numbered(n));
        }
        assert_eq!(store.pending(), MAX_PENDING_CHALLENGES);
        assert!(matches!(
            redeem(&store, &numbered(0), &oldest, &vec![]),
            Err(AuthError::UnknownChallenge)
        ));
        assert!(
            matches!(
                redeem(&store, &numbered(MAX_PENDING_CHALLENGES), &newest, &vec![]),
                Err(AuthError::InvalidChain(_))
            ),
            "the newest challenge must still be known"
        );
    }

    #[test]
    fn a_flood_of_sockets_keeps_the_newest_challenges_and_redeems_one_of_them() {
        use alloy::signers::local::PrivateKeySigner;
        let store = ChallengeStore::new(cfg());
        let signer = PrivateKeySigner::random();
        let address = format!("{:#x}", signer.address());
        for n in 0..=MAX_PENDING_CHALLENGES {
            store.put_for_key(&format!("early-socket-{n}"), "churn");
        }
        let waiting = offered(&store, &address);
        for n in 0..MAX_PENDING_CHALLENGES - 1 {
            store.put_for_key(&format!("later-socket-{n}"), "churn");
        }
        assert_eq!(store.pending(), MAX_PENDING_CHALLENGES);
        assert!(
            store
                .redeem_and_verify_payload(
                    &address,
                    &address,
                    &waiting,
                    "transcript",
                    &signed(&signer, "transcript"),
                )
                .is_ok(),
            "a challenge among the newest MAX_PENDING_CHALLENGES survives the flood"
        );
    }

    #[test]
    fn discarded_challenges_neither_hold_capacity_nor_evict_a_live_one() {
        let store = ChallengeStore::new(cfg());
        let live = offered(&store, &numbered(0));
        for n in 1..=3 * MAX_PENDING_CHALLENGES {
            let churned = offered(&store, &numbered(n));
            store.discard_for_key(&numbered(n), &churned);
        }
        assert_eq!(store.pending(), 1);
        assert!(store.queued() <= 2 * MAX_PENDING_CHALLENGES + 1);
        assert!(matches!(
            redeem(&store, &numbered(0), &live, &vec![]),
            Err(AuthError::InvalidChain(_))
        ));
    }

    #[test]
    fn an_expired_challenge_is_unknown_even_before_it_is_collected() {
        let store = ChallengeStore::new(AuthConfig {
            challenge_ttl_secs: 0,
            ..cfg()
        });
        let challenge = offered(&store, &numbered(1));
        // Issued a second ago rather than slept on: strictly before the zero-TTL cutoff.
        for issued in store.pending.lock().by_challenge.values_mut() {
            issued.issued_at -= chrono::Duration::seconds(1);
        }
        assert!(matches!(
            redeem(&store, &numbered(1), &challenge, &vec![]),
            Err(AuthError::UnknownChallenge)
        ));
    }

    #[test]
    fn discarding_one_socket_challenge_preserves_another() {
        use alloy::signers::local::PrivateKeySigner;
        let store = ChallengeStore::new(cfg());
        let signer = PrivateKeySigner::random();
        let address = format!("{:#x}", signer.address());
        let first = offered(&store, &address);
        let second = offered(&store, &address);
        store.discard_for_key(&address, &first);
        assert!(matches!(
            redeem(&store, &address, &first, &signed(&signer, &first)),
            Err(AuthError::UnknownChallenge)
        ));
        redeem(&store, &address, &second, &signed(&signer, &second)).unwrap();
    }

    #[test]
    fn a_socket_challenge_is_stored_and_redeemed_under_its_key_in_any_case() {
        use alloy::signers::local::PrivateKeySigner;
        let store = ChallengeStore::new(cfg());
        let signer = PrivateKeySigner::random();
        let address = format!("{:#x}", signer.address());
        let challenge = offered(&store, &address.to_uppercase());
        redeem(&store, &address, &challenge, &signed(&signer, &challenge)).unwrap();
        assert_eq!(store.pending(), 0);
    }
}
