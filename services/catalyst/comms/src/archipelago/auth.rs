use crate::auth::AuthChain;
use crate::auth::{eip1654::Eip1654Validator, verify::verify_auth_chain_at_verification_time};
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

/// A server-issued socket challenge and its exact generation. Clients never select this handle.
#[derive(Clone, Debug)]
pub struct Issuance {
    key: ChallengeKey,
    sequence: u64,
}

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

    pub fn issue(&self, challenge: &str) -> Issuance {
        self.put_for_key(&uuid::Uuid::new_v4().to_string(), challenge)
    }

    fn put_for_key(&self, key: &str, challenge: &str) -> Issuance {
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
            stored.clone(),
            Issued {
                issued_at: Utc::now(),
                sequence,
            },
        );
        Issuance {
            key: stored,
            sequence,
        }
    }

    pub fn discard(&self, issuance: &Issuance) {
        let mut pending = self.pending.lock();
        if pending
            .by_challenge
            .get(&issuance.key)
            .is_some_and(|issued| issued.sequence == issuance.sequence)
        {
            pending.by_challenge.remove(&issuance.key);
        }
    }

    pub async fn redeem_and_verify_payload(
        &self,
        issuance: &Issuance,
        expected_payload: &str,
        chain: &AuthChain,
        validator: Option<&dyn Eip1654Validator>,
    ) -> Result<String, AuthError> {
        let key = &issuance.key;
        let issued = self
            .pending
            .lock()
            .by_challenge
            .get(key)
            .filter(|issued| {
                issued.sequence == issuance.sequence && issued.issued_at >= self.cutoff()
            })
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
        let last_payload = chain.last().map(|l| l.payload.clone()).unwrap_or_default();
        if last_payload != expected_payload {
            return Err(AuthError::ChallengeMismatch);
        }

        verify_auth_chain_at_verification_time(
            chain,
            expected_payload,
            now.timestamp_millis(),
            validator.and_then(Eip1654Validator::fresh_validator),
        )
        .await
        .map_err(|e| AuthError::InvalidChain(format!("{:?}", e)))?;
        let signer = chain[0].payload.to_ascii_lowercase();
        if !crate::archipelago::session::is_address(&signer) {
            return Err(AuthError::AddressMismatch);
        }
        let mut guard = self.pending.lock();
        match guard.by_challenge.get(key) {
            Some(current)
                if current.sequence == issued.sequence && current.issued_at >= self.cutoff() =>
            {
                guard.by_challenge.remove(key);
                Ok(signer)
            }
            _ => Err(AuthError::UnknownChallenge),
        }
    }

    fn cutoff(&self) -> DateTime<Utc> {
        Utc::now() - chrono::Duration::seconds(self.cfg.challenge_ttl_secs as i64)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn pending(&self) -> usize {
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
    use crate::auth::verify_auth_chain_async;

    impl ChallengeStore {
        // Existing chain fixtures use named storage keys; production sockets retain the issued handle.
        async fn verify_stored(
            &self,
            key: &str,
            _claim: &str,
            challenge: &str,
            payload: &str,
            chain: &AuthChain,
            validator: Option<&dyn Eip1654Validator>,
        ) -> Result<(), AuthError> {
            let key = (key.to_ascii_lowercase(), challenge.to_owned());
            let sequence = self
                .pending
                .lock()
                .by_challenge
                .get(&key)
                .ok_or(AuthError::UnknownChallenge)?
                .sequence;
            self.redeem_and_verify_payload(&Issuance { key, sequence }, payload, chain, validator)
                .await
                .map(|_| ())
        }

        fn discard_for_key(&self, key: &str, challenge: &str) {
            let key = (key.to_ascii_lowercase(), challenge.to_owned());
            let sequence = self
                .pending
                .lock()
                .by_challenge
                .get(&key)
                .map(|issued| issued.sequence);
            if let Some(sequence) = sequence {
                self.discard(&Issuance { key, sequence });
            }
        }
    }

    #[tokio::test]
    async fn opaque_issuance_isolates_equal_nonces_and_stale_cleanup() {
        let store = ChallengeStore::new(cfg());
        let nonce = "forced-equal-nonce";
        let first = store.issue(nonce);
        let other = store.issue(nonce);
        let signer = alloy::signers::local::PrivateKeySigner::random();
        let chain = signed(&signer, nonce);
        store.discard(&first);
        let replacement = store.put_for_key(&first.key.0, nonce);
        store.discard(&first);
        assert!(matches!(
            store
                .redeem_and_verify_payload(&first, nonce, &chain, None)
                .await,
            Err(AuthError::UnknownChallenge)
        ));
        assert_eq!(
            store
                .redeem_and_verify_payload(&replacement, nonce, &chain, None)
                .await
                .unwrap(),
            format!("{:#x}", signer.address())
        );
        assert!(
            store
                .redeem_and_verify_payload(&other, nonce, &chain, None)
                .await
                .is_ok()
        );
        assert_eq!(store.pending(), 0);
    }

    struct Answer(bool);
    #[async_trait::async_trait]
    impl Eip1654Validator for Answer {
        fn fresh_validator(&self) -> Option<&dyn Eip1654Validator> {
            Some(self)
        }
        async fn validate_signature(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
        ) -> Result<bool, crate::auth::AuthError> {
            Ok(self.0)
        }
    }

    struct RpcError;
    #[async_trait::async_trait]
    impl Eip1654Validator for RpcError {
        fn fresh_validator(&self) -> Option<&dyn Eip1654Validator> {
            Some(self)
        }
        async fn validate_signature(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
        ) -> Result<bool, crate::auth::AuthError> {
            Err(crate::auth::AuthError::Eip1654ValidationFailed(
                "unavailable".into(),
            ))
        }
    }

    fn contract_chain(address: &str, challenge: &str) -> AuthChain {
        serde_json::from_value(serde_json::json!([
            {"type":"SIGNER", "payload":address, "signature":""},
            {"type":"ECDSA_EIP_1654_SIGNED_ENTITY", "payload":challenge, "signature":"0x1234"}
        ]))
        .unwrap()
    }

    struct Held(tokio::sync::Semaphore);

    struct HistoricalHeld {
        hold_lookup: bool,
        release: tokio::sync::Semaphore,
    }

    #[async_trait::async_trait]
    impl Eip1654Validator for HistoricalHeld {
        fn fresh_validator(&self) -> Option<&dyn Eip1654Validator> {
            Some(self)
        }
        async fn validate_signature(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
        ) -> Result<bool, crate::auth::AuthError> {
            Ok(false)
        }
        async fn verification_time_block(&self, _: i64) -> Result<u64, crate::auth::AuthError> {
            if self.hold_lookup {
                self.release.acquire().await.unwrap().forget();
            }
            Ok(4)
        }
        async fn validate_signature_at_block(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
            block: u64,
        ) -> Result<bool, crate::auth::AuthError> {
            assert_eq!(block, 4);
            if !self.hold_lookup {
                self.release.acquire().await.unwrap().forget();
            }
            Ok(true)
        }
    }

    #[tokio::test]
    async fn historical_lookup_and_call_preserve_opaque_issuance_consumption_reissue_and_expiry() {
        for hold_lookup in [false, true] {
            for invalidation in ["none", "reissue", "expiry", "cancel"] {
                let store = ChallengeStore::new(cfg());
                let principal = "0x0000000000000000000000000000000000000001";
                let challenge = "same nonce";
                let issuance = store.put_for_key("socket", challenge);
                let chain = contract_chain(principal, challenge);
                let validator = HistoricalHeld {
                    hold_lookup,
                    release: tokio::sync::Semaphore::new(0),
                };
                let mut first = Box::pin(store.redeem_and_verify_payload(
                    &issuance,
                    challenge,
                    &chain,
                    Some(&validator),
                ));
                let mut second = Box::pin(store.redeem_and_verify_payload(
                    &issuance,
                    challenge,
                    &chain,
                    Some(&validator),
                ));
                assert!(futures::poll!(&mut first).is_pending());
                assert!(futures::poll!(&mut second).is_pending());
                assert!(store.pending.try_lock().is_some());
                let replacement = if invalidation == "reissue" {
                    store.discard(&issuance);
                    Some(store.put_for_key("socket", challenge))
                } else {
                    None
                };
                if invalidation == "expiry" {
                    store
                        .pending
                        .lock()
                        .by_challenge
                        .get_mut(&issuance.key)
                        .unwrap()
                        .issued_at -= chrono::Duration::seconds(121);
                }
                if invalidation == "cancel" {
                    drop(first);
                    drop(second);
                    store.discard(&issuance);
                    validator.release.add_permits(2);
                    assert_eq!(store.pending(), 0);
                    continue;
                }
                validator.release.add_permits(2);
                let (a, b) = tokio::join!(first, second);
                if invalidation == "none" {
                    assert_eq!(
                        usize::from(a.as_ref().is_ok_and(|s| s == principal))
                            + usize::from(b.as_ref().is_ok_and(|s| s == principal)),
                        1
                    );
                    assert_eq!(store.pending(), 0);
                } else {
                    assert!(matches!(a, Err(AuthError::UnknownChallenge)));
                    assert!(matches!(b, Err(AuthError::UnknownChallenge)));
                    if let Some(replacement) = replacement {
                        store.discard(&issuance);
                        assert_eq!(store.pending(), 1);
                        assert_eq!(
                            store
                                .redeem_and_verify_payload(
                                    &replacement,
                                    challenge,
                                    &chain,
                                    Some(&Answer(true))
                                )
                                .await
                                .unwrap(),
                            principal
                        );
                    }
                }
            }
        }
    }
    #[async_trait::async_trait]
    impl Eip1654Validator for Held {
        fn fresh_validator(&self) -> Option<&dyn Eip1654Validator> {
            Some(self)
        }
        async fn validate_signature(
            &self,
            _: &str,
            _: &[u8],
            _: &[u8],
        ) -> Result<bool, crate::auth::AuthError> {
            self.0.acquire().await.unwrap().forget();
            Ok(true)
        }
    }

    #[tokio::test]
    async fn pending_contract_verifications_release_lock_and_commit_only_current_issuance_once() {
        for replace in [false, true] {
            let store = ChallengeStore::new(cfg());
            let address = "0x0000000000000000000000000000000000000001";
            let challenge = offered(&store, address);
            let chain = contract_chain(address, &challenge);
            let validator = Held(tokio::sync::Semaphore::new(0));
            let mut first = Box::pin(store.verify_stored(
                address,
                address,
                &challenge,
                &challenge,
                &chain,
                Some(&validator),
            ));
            let mut second = Box::pin(store.verify_stored(
                address,
                address,
                &challenge,
                &challenge,
                &chain,
                Some(&validator),
            ));
            assert!(futures::poll!(&mut first).is_pending());
            assert!(futures::poll!(&mut second).is_pending());
            assert!(
                store.pending.try_lock().is_some(),
                "RPC must not hold challenge lock"
            );
            if replace {
                store.discard_for_key(address, &challenge);
                store.put_for_key(address, &challenge);
            }
            validator.0.add_permits(2);
            let (a, b) = tokio::join!(first, second);
            assert_eq!(
                usize::from(a.is_ok()) + usize::from(b.is_ok()),
                usize::from(!replace)
            );
            assert_eq!(store.pending(), usize::from(replace));
            if replace {
                store
                    .verify_stored(
                        address,
                        address,
                        &challenge,
                        &challenge,
                        &chain,
                        Some(&Answer(true)),
                    )
                    .await
                    .unwrap();
            }
        }
    }

    #[tokio::test]
    async fn discarded_or_cancelled_contract_verification_cannot_consume_a_challenge() {
        let store = ChallengeStore::new(cfg());
        let address = "0x0000000000000000000000000000000000000001";
        let challenge = offered(&store, address);
        let chain = contract_chain(address, &challenge);
        let validator = Held(tokio::sync::Semaphore::new(0));
        let mut pending = Box::pin(store.verify_stored(
            address,
            address,
            &challenge,
            &challenge,
            &chain,
            Some(&validator),
        ));
        assert!(futures::poll!(&mut pending).is_pending());
        drop(pending);
        assert_eq!(store.pending(), 1);
        let mut pending = Box::pin(store.verify_stored(
            address,
            address,
            &challenge,
            &challenge,
            &chain,
            Some(&validator),
        ));
        assert!(futures::poll!(&mut pending).is_pending());
        store.discard_for_key(address, &challenge);
        validator.0.add_permits(1);
        assert!(matches!(pending.await, Err(AuthError::UnknownChallenge)));
        assert_eq!(store.pending(), 0);
    }

    #[tokio::test]
    async fn challenge_expiring_during_rpc_is_not_redeemed() {
        let store = ChallengeStore::new(cfg());
        let address = "0x0000000000000000000000000000000000000001";
        let challenge = offered(&store, address);
        let chain = contract_chain(address, &challenge);
        let validator = Held(tokio::sync::Semaphore::new(0));
        let mut pending = Box::pin(store.verify_stored(
            address,
            address,
            &challenge,
            &challenge,
            &chain,
            Some(&validator),
        ));
        assert!(futures::poll!(&mut pending).is_pending());
        for issued in store.pending.lock().by_challenge.values_mut() {
            issued.issued_at -= chrono::Duration::seconds(121);
        }
        validator.0.add_permits(1);
        assert!(matches!(pending.await, Err(AuthError::UnknownChallenge)));
    }

    #[tokio::test]
    async fn contract_refusal_missing_provider_and_peer_trust_fail_closed() {
        let store = ChallengeStore::new(cfg());
        let address = "0x0000000000000000000000000000000000000001";
        let challenge = offered(&store, address);
        let chain = contract_chain(address, &challenge);
        for validator in [
            None,
            Some(&Answer(false) as &dyn Eip1654Validator),
            Some(&RpcError),
            Some(&crate::auth::eip1654::PeerValidated),
        ] {
            assert!(matches!(
                store
                    .verify_stored(address, address, &challenge, &challenge, &chain, validator)
                    .await,
                Err(AuthError::InvalidChain(_))
            ));
            assert_eq!(store.pending(), 1);
        }
    }

    fn mandate(address: &str, expiration: &str) -> String {
        format!("Decentraland Login\nEphemeral address: {address}\nExpiration: {expiration}")
    }

    #[tokio::test]
    async fn delegated_and_mixed_contract_chains_preserve_binding_expiry_and_ten_link_limit() {
        use alloy::signers::{SignerSync, local::PrivateKeySigner};
        let root = PrivateKeySigner::random();
        let device = PrivateKeySigner::random();
        let contract = "0x0000000000000000000000000000000000000001";
        for (links, mixed) in [(3, false), (4, true), (10, false), (11, false)] {
            let store = ChallengeStore::new(cfg());
            let address = if mixed {
                format!("{:#x}", root.address())
            } else {
                contract.to_string()
            };
            let challenge = offered(&store, &address);
            let mut json =
                vec![serde_json::json!({"type":"SIGNER", "payload":address, "signature":""})];
            if mixed {
                let payload = mandate(contract, "2099-12-31T00:00:00.000Z");
                json.push(
                    serde_json::json!({"type":"ECDSA_EPHEMERAL", "payload":payload,
                    "signature":root.sign_message_sync(payload.as_bytes()).unwrap().to_string()}),
                );
            }
            while json.len() < links - 1 {
                let delegated = if json.len() == links - 2 {
                    format!("{:#x}", device.address())
                } else {
                    contract.into()
                };
                json.push(serde_json::json!({"type":"ECDSA_EIP_1654_EPHEMERAL", "payload":mandate(&delegated, "2099-12-31T00:00:00.000Z"), "signature":"0x1234"}));
            }
            json.push(
                serde_json::json!({"type":"ECDSA_SIGNED_ENTITY", "payload":challenge,
                "signature":device.sign_message_sync(challenge.as_bytes()).unwrap().to_string()}),
            );
            let chain: AuthChain = serde_json::from_value(json.into()).unwrap();
            if links == 11 {
                assert!(matches!(
                    store
                        .verify_stored(
                            &address,
                            &address,
                            &challenge,
                            &challenge,
                            &chain,
                            Some(&Answer(true))
                        )
                        .await,
                    Err(AuthError::InvalidChain(_))
                ));
                continue;
            }
            let mut invalid = chain.clone();
            invalid.last_mut().unwrap().payload = "other payload".into();
            assert!(matches!(
                store
                    .verify_stored(
                        &address,
                        &address,
                        &challenge,
                        &challenge,
                        &invalid,
                        Some(&Answer(true))
                    )
                    .await,
                Err(AuthError::ChallengeMismatch)
            ));
            invalid = chain.clone();
            invalid[0].payload = "not an address".into();
            assert!(matches!(
                store
                    .verify_stored(
                        &address,
                        &address,
                        &challenge,
                        &challenge,
                        &invalid,
                        Some(&Answer(true))
                    )
                    .await,
                Err(AuthError::InvalidChain(_) | AuthError::AddressMismatch)
            ));
            invalid = chain.clone();
            invalid[links - 2].payload = mandate(
                &format!("{:#x}", device.address()),
                "2000-01-01T00:00:00.000Z",
            );
            assert!(matches!(
                store
                    .verify_stored(
                        &address,
                        &address,
                        &challenge,
                        &challenge,
                        &invalid,
                        Some(&Answer(true))
                    )
                    .await,
                Err(AuthError::InvalidChain(_))
            ));
            invalid = chain.clone();
            invalid[links - 2].signature = Some("not hex".into());
            assert!(matches!(
                store
                    .verify_stored(
                        &address,
                        &address,
                        &challenge,
                        &challenge,
                        &invalid,
                        Some(&Answer(true))
                    )
                    .await,
                Err(AuthError::InvalidChain(_))
            ));
            invalid = chain.clone();
            invalid.remove(0);
            assert!(matches!(
                store
                    .verify_stored(
                        &address,
                        &address,
                        &challenge,
                        &challenge,
                        &invalid,
                        Some(&Answer(true))
                    )
                    .await,
                Err(AuthError::InvalidChain(_))
            ));
            assert_eq!(store.pending(), 1);
            store
                .verify_stored(
                    &address,
                    &address,
                    &challenge,
                    &challenge,
                    &chain,
                    Some(&Answer(true)),
                )
                .await
                .unwrap();
            assert_eq!(store.pending(), 0);
        }
    }

    #[tokio::test]
    async fn a_cached_delegation_cannot_authorize_a_new_challenge_after_revocation() {
        use alloy::signers::local::PrivateKeySigner;
        use std::sync::atomic::{AtomicBool, Ordering};
        struct Revocable(AtomicBool);
        #[async_trait::async_trait]
        impl Eip1654Validator for Revocable {
            fn fresh_validator(&self) -> Option<&dyn Eip1654Validator> {
                Some(self)
            }
            async fn validate_signature(
                &self,
                _: &str,
                _: &[u8],
                _: &[u8],
            ) -> Result<bool, crate::auth::AuthError> {
                Ok(self.0.load(Ordering::SeqCst))
            }
        }
        let validator = Arc::new(Revocable(AtomicBool::new(true)));
        let cache = crate::auth::validation_cache::ValidationCache::new(Arc::new(
            crate::auth::validation_cache::ValidationCache::new(validator.clone()),
        ));
        let store = ChallengeStore::new(cfg());
        let address = "0x0000000000000000000000000000000000000001";
        let device = PrivateKeySigner::random();
        let delegation: crate::auth::AuthLink = serde_json::from_value(serde_json::json!({"type":"ECDSA_EIP_1654_EPHEMERAL",
            "payload":mandate(&format!("{:#x}", device.address()), "2099-12-31T00:00:00.000Z"), "signature":"0x1234"})).unwrap();
        let make_chain = |payload: &str| {
            let mut chain = signed(&device, payload);
            chain[0].payload = address.into();
            chain.insert(1, delegation.clone());
            chain
        };
        // An unrelated domain may still retain its successful delegation verdict.
        verify_auth_chain_async(&make_chain("earlier"), "earlier", None, Some(&cache))
            .await
            .unwrap();
        validator.0.store(false, Ordering::SeqCst);
        let challenge = offered(&store, address);
        assert!(matches!(
            store
                .verify_stored(
                    address,
                    address,
                    &challenge,
                    &challenge,
                    &make_chain(&challenge),
                    Some(&cache)
                )
                .await,
            Err(AuthError::InvalidChain(_))
        ));
        assert_eq!(store.pending(), 1);
    }

    #[test]
    fn contract_challenge_is_verified_by_configured_validator() {
        let store = ChallengeStore::new(cfg());
        let address = "0x0000000000000000000000000000000000000001";
        let challenge = offered(&store, address);
        let chain = serde_json::from_value(serde_json::json!([
            {"type":"SIGNER", "payload":address, "signature":""},
            {"type":"ECDSA_EIP_1654_SIGNED_ENTITY", "payload":challenge, "signature":"0x1234"}
        ]))
        .unwrap();
        futures::executor::block_on(store.verify_stored(
            address,
            address,
            &challenge,
            &challenge,
            &chain,
            Some(&Answer(true)),
        ))
        .unwrap();
    }

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
        futures::executor::block_on(
            store.verify_stored(address, address, challenge, challenge, chain, None),
        )
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
        let mut invalid = chain.clone();
        invalid.last_mut().unwrap().signature = signed(&PrivateKeySigner::random(), &first)
            .last()
            .unwrap()
            .signature
            .clone();
        assert!(redeem(&store, &address, &first, &invalid).is_err());
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
            futures::executor::block_on(store.verify_stored(
                &address,
                &address,
                &waiting,
                "transcript",
                &signed(&signer, "transcript"),
                None,
            ))
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
