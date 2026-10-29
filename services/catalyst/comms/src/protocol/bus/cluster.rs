//! Pulse cluster-change hints, authoritative wallet/session lookups, and retained
//! takeover completions from the gatekeeper.

use prost::Message as _;

use super::{BusError, MAX_PAYLOAD_BYTES, peer_subject_wallet, wallet_token};
use crate::protocol::pulse::decentraland::pulse::TakeoverCompleted;
use crate::protocol::pulse::decentraland::pulse::{PeerClusterChange, PeerClusterSnapshot};

pub const MAX_SAFE_SECONDS: u64 = 9_007_199_254_740_991;
pub const MAX_CONSUMER_SECONDS: u64 = MAX_SAFE_SECONDS / 1000;

/// Completion retains its operation identity across transport loss and retries.
pub struct Completion {
    pub wallet: String,
    pub completion: TakeoverCompleted,
}

impl Completion {
    pub const SUBJECTS: &str = "peer.*.takeover_completed";
    pub fn subject(&self) -> String {
        format!(
            "peer.{}.takeover_completed",
            self.wallet.to_ascii_lowercase()
        )
    }
    pub fn decode(subject: &str, payload: &[u8]) -> Option<Self> {
        if payload.is_empty() || payload.len() > 4096 {
            return None;
        }
        let wallet = peer_subject_wallet(subject, "takeover_completed")?;
        let completion = TakeoverCompleted::decode(payload).ok()?;
        if completion.takeover_id.is_empty()
            || completion.takeover_id.len() > 64
            || completion.revoke_before == 0
            || completion.revoke_before > MAX_CONSUMER_SECONDS
        {
            return None;
        }
        Some(Self { wallet, completion })
    }
}

/// The most a lookup reply carries; Pulse drops a larger one rather than publish it.
pub const MAX_REPLY_BYTES: usize = 64 * 1024;

/// A peer's published (post-debounce) cluster assignment changed. Pulse publishes it, one per
/// wallet whose cluster changed in a pass; the gatekeeper's island subscriber mints from it.
/// A hint: a lost change is recovered by the lookup on the wallet's next connect.
///
/// The wallet is the subject's second token, not part of the payload.
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterChange {
    pub wallet: String,
    pub change: PeerClusterChange,
}

impl ClusterChange {
    pub const SUBJECTS: &str = "peer.*.cluster_change";

    /// Lower-cased once at the publish boundary, so one wallet always maps to one subject
    /// whatever checksum casing the auth chain carried. The subject is also the outbox's
    /// coalescing key, so per-subject latest-wins is exactly per-peer latest-wins.
    pub fn subject(&self) -> String {
        format!("peer.{}.cluster_change", self.wallet.to_ascii_lowercase())
    }

    pub fn encode(&self) -> Vec<u8> {
        self.change.encode_to_vec()
    }

    /// The lower-cased wallet a `peer.{wallet}.cluster_change` subject names.
    pub fn subject_wallet(subject: &str) -> Option<String> {
        peer_subject_wallet(subject, "cluster_change")
    }

    /// Lower-cases `session` and `displaced_session` once at decode time, so minting, the mirror
    /// and the session-addressed subject all agree on their casing.
    ///
    /// The subject is read before the payload: a [`BusError::Payload`] comes from a subject
    /// that names a wallet ([`Self::subject_wallet`]).
    pub fn decode(subject: &str, payload: &[u8]) -> Result<Self, BusError> {
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(BusError::TooLarge);
        }
        let wallet = Self::subject_wallet(subject).ok_or(BusError::Subject)?;
        let mut change = PeerClusterChange::decode(payload)?;
        change.session = change.session.to_lowercase();
        change.displaced_session = change.displaced_session.to_lowercase();
        Ok(Self { wallet, change })
    }
}

/// The gatekeeper's request for Pulse's current view of one wallet and session. The reply is
/// a [`PeerClusterSnapshot`] of at most [`MAX_REPLY_BYTES`] on a private `_INBOX.` subject, and
/// Pulse replies for a fresh live registration or an exact retained inactive session
/// with pending cleanup. The
/// requester allows [`REQUEST_DEADLINE`](super::REQUEST_DEADLINE); silence or a reply that is
/// not [`valid_reply`] means unknown, and nothing is minted.
///
/// The wallet is the subject's second token and the payload is the session address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterLookup {
    pub wallet: String,
    pub session: String,
}

impl ClusterLookup {
    pub const SUBJECTS: &str = "peer.*.cluster_lookup";

    /// Lower-cased, as [`ClusterChange::subject`] is.
    pub fn subject(&self) -> String {
        format!("peer.{}.cluster_lookup", self.wallet.to_ascii_lowercase())
    }

    /// The session's 42 bytes, lower-cased like the subject's wallet.
    pub fn encode(&self) -> Vec<u8> {
        self.session.to_ascii_lowercase().into_bytes()
    }

    pub fn decode(subject: &str, payload: &[u8]) -> Result<Self, BusError> {
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(BusError::TooLarge);
        }
        let wallet = peer_subject_wallet(subject, "cluster_lookup").ok_or(BusError::Subject)?;
        let session = std::str::from_utf8(payload)
            .ok()
            .and_then(wallet_token)
            .ok_or(BusError::Subject)?;
        Ok(Self { wallet, session })
    }
}

/// Whether `reply` answers `lookup` and names the Pulse that gave it: the wallet and session
/// the lookup put on the wire (lower-cased), a cluster, a source incarnation of at most 256
/// bytes, and a pass that has run. Anything else is read as no answer.
pub fn valid_reply(lookup: &ClusterLookup, reply: &PeerClusterSnapshot) -> bool {
    reply.wallet == lookup.wallet.to_ascii_lowercase()
        && reply.session == lookup.session.to_ascii_lowercase()
        && !reply.cluster_id.is_empty()
        && !reply.source_incarnation.is_empty()
        && reply.source_incarnation.len() <= 256
        && reply.pass > 0
        && reply.assignment_version > 0
        && reply.encoded_len() <= MAX_REPLY_BYTES
        && reply
            .token_not_before
            .is_none_or(|floor| floor <= MAX_CONSUMER_SECONDS)
        && reply.pending_takeovers.len() <= 32
        && reply
            .pending_takeovers
            .iter()
            .enumerate()
            .all(|(i, pending)| {
                !pending.takeover_id.is_empty()
                    && pending.takeover_id.len() <= 64
                    && wallet_token(&pending.displaced_session).as_deref()
                        == Some(pending.displaced_session.as_str())
                    && !pending.displaced_cluster_id.is_empty()
                    && pending.displaced_cluster_id.len() <= MAX_REPLY_BYTES
                    && pending.displaced_realm.len() <= MAX_REPLY_BYTES
                    && !reply.pending_takeovers[..i]
                        .iter()
                        .any(|p| p.takeover_id == pending.takeover_id)
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::bus::{BusError, MAX_PAYLOAD_BYTES};

    const WALLET: &str = "0xAbCdEf0123456789abcdef0123456789ABCDEF01";
    const WALLET_LOWER: &str = "0xabcdef0123456789abcdef0123456789abcdef01";

    #[test]
    fn retained_recovery_fixture_pins_additive_schema_and_completion_bounds() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/pulse_retained_recovery.json"
        )))
        .unwrap();
        let snapshot = PeerClusterSnapshot {
            wallet: "A".into(),
            session: "B".into(),
            cluster_id: "C".into(),
            realm: "R".into(),
            pass: 1,
            source_incarnation: "I".into(),
            token_not_before: Some(0),
            assignment_version: 1,
            pending_takeovers: vec![],
        };
        assert_eq!(
            hex::encode(snapshot.encode_to_vec()),
            fixture["wire"]["snapshot_hex"].as_str().unwrap()
        );
        let completion = TakeoverCompleted {
            takeover_id: "T".into(),
            revoke_before: 105,
        };
        assert_eq!(
            hex::encode(completion.encode_to_vec()),
            fixture["wire"]["completion_hex"].as_str().unwrap()
        );
        let subject = format!("peer.{WALLET_LOWER}.takeover_completed");
        assert!(Completion::decode(&subject, &completion.encode_to_vec()).is_some());
        for id in ["".to_owned(), "x".repeat(65)] {
            assert!(
                Completion::decode(
                    &subject,
                    &TakeoverCompleted {
                        takeover_id: id,
                        revoke_before: 105
                    }
                    .encode_to_vec()
                )
                .is_none()
            );
        }
        assert!(
            Completion::decode(
                &subject,
                &TakeoverCompleted {
                    takeover_id: "T".into(),
                    revoke_before: MAX_CONSUMER_SECONDS,
                }
                .encode_to_vec()
            )
            .is_some()
        );
        for cutoff in [0, MAX_CONSUMER_SECONDS + 1, MAX_SAFE_SECONDS + 1] {
            assert!(
                Completion::decode(
                    &subject,
                    &TakeoverCompleted {
                        takeover_id: "T".into(),
                        revoke_before: cutoff
                    }
                    .encode_to_vec()
                )
                .is_none()
            );
        }
        assert!(Completion::decode(&subject, &[]).is_none());
        assert!(Completion::decode(&subject, &vec![0; 4097]).is_none());
        assert!(
            Completion::decode("peer.*.takeover_completed", &completion.encode_to_vec()).is_none()
        );
    }

    fn address(byte: &str) -> String {
        format!("0x{}", byte.repeat(20))
    }

    fn change_message() -> PeerClusterChange {
        PeerClusterChange {
            cluster_id: "c1".into(),
            realm: "main".into(),
            session: address("02"),
            displaced_session: address("03"),
            displaced_cluster_id: "c0".into(),
            pending_takeovers: Vec::new(),
            token_not_before: None,
            assignment_version: 0,
            source_incarnation: String::new(),
        }
    }

    /// The subject and bytes Pulse's feed produced for this change before they moved here.
    #[test]
    fn cluster_change_bytes_are_pinned() {
        let change = ClusterChange {
            wallet: WALLET.into(),
            change: change_message(),
        };
        assert_eq!(
            change.subject(),
            "peer.0xabcdef0123456789abcdef0123456789abcdef01.cluster_change"
        );
        assert_eq!(
            hex::encode(change.encode()),
            concat!(
                "0a02633112046d61696e1a2a30783032303230323032303230323032303230323032",
                "3032303230323032303230323032303230323032222a307830333033303330333033",
                "3033303330333033303330333033303330333033303330333033303330332a026330"
            )
        );
        let decoded = ClusterChange::decode(&change.subject(), &change.encode()).unwrap();
        assert_eq!(decoded.wallet, WALLET_LOWER);
        assert_eq!(decoded.change, change.change);
    }

    /// The request is the session's 42 ASCII bytes; the reply is the plain encoding of Pulse's
    /// `PeerClusterSnapshot`. Both recorded from the code that served and consumed them.
    #[test]
    fn lookup_bytes_are_pinned() {
        let lookup = ClusterLookup {
            wallet: WALLET_LOWER.into(),
            session: address("02"),
        };
        assert_eq!(
            lookup.subject(),
            "peer.0xabcdef0123456789abcdef0123456789abcdef01.cluster_lookup"
        );
        assert_eq!(
            hex::encode(lookup.encode()),
            "307830323032303230323032303230323032303230323032303230323032303230323032303230323032"
        );
        let reply = PeerClusterSnapshot {
            wallet: WALLET_LOWER.into(),
            session: address("02"),
            cluster_id: "c1".into(),
            realm: "main".into(),
            pass: 7,
            source_incarnation: "_INBOX.x".into(),
            pending_takeovers: Vec::new(),
            token_not_before: Some(0),
            assignment_version: 1,
        };
        let bytes = reply.encode_to_vec();
        assert_eq!(
            hex::encode(&bytes),
            concat!(
                "0a2a3078616263646566303132333435363738396162636465663031323334353637",
                "38396162636465663031122a30783032303230323032303230323032303230323032",
                "30323032303230323032303230323032303230321a02633122046d61696e28073208",
                "5f494e424f582e7840004801"
            )
        );
        assert!(bytes.len() <= MAX_REPLY_BYTES);
        let decoded = PeerClusterSnapshot::decode(bytes.as_slice()).unwrap();
        assert!(valid_reply(&lookup, &decoded));
    }

    #[test]
    fn a_reply_must_answer_the_lookup_and_name_its_source() {
        let lookup = ClusterLookup {
            wallet: WALLET_LOWER.into(),
            session: address("02"),
        };
        let reply = PeerClusterSnapshot {
            wallet: lookup.wallet.clone(),
            session: lookup.session.clone(),
            cluster_id: "c1".into(),
            realm: "main".into(),
            pass: 7,
            source_incarnation: "_INBOX.x".into(),
            pending_takeovers: Vec::new(),
            token_not_before: Some(0),
            assignment_version: 1,
        };
        assert!(valid_reply(&lookup, &reply));
        let refused = [
            PeerClusterSnapshot {
                wallet: address("04"),
                ..reply.clone()
            },
            PeerClusterSnapshot {
                session: address("03"),
                ..reply.clone()
            },
            PeerClusterSnapshot {
                cluster_id: String::new(),
                ..reply.clone()
            },
            PeerClusterSnapshot {
                source_incarnation: String::new(),
                ..reply.clone()
            },
            PeerClusterSnapshot {
                source_incarnation: "x".repeat(257),
                ..reply.clone()
            },
            PeerClusterSnapshot {
                pass: 0,
                ..reply.clone()
            },
        ];
        for reply in &refused {
            assert!(!valid_reply(&lookup, reply), "{reply:?}");
        }
        let longest = PeerClusterSnapshot {
            source_incarnation: "x".repeat(256),
            ..reply
        };
        assert!(valid_reply(&lookup, &longest));
    }

    /// A lookup built from a mixed-case wallet and session goes on the wire lower-cased, subject
    /// and payload alike, and the lower-cased reply Pulse sends answers it.
    #[test]
    fn a_mixed_case_lookup_is_answered_by_its_lower_cased_reply() {
        let lookup = ClusterLookup {
            wallet: WALLET.into(),
            session: address("AB"),
        };
        assert_eq!(lookup.encode(), address("ab").into_bytes());
        let reply = PeerClusterSnapshot {
            wallet: WALLET_LOWER.into(),
            session: address("ab"),
            cluster_id: "c1".into(),
            realm: "main".into(),
            pass: 1,
            source_incarnation: "pulse".into(),
            pending_takeovers: Vec::new(),
            token_not_before: Some(0),
            assignment_version: 1,
        };
        assert!(valid_reply(&lookup, &reply));
        assert!(!valid_reply(
            &lookup,
            &PeerClusterSnapshot {
                session: address("AB"),
                ..reply
            }
        ));
    }

    #[test]
    fn subjects_are_lower_cased() {
        let change = ClusterChange {
            wallet: WALLET.into(),
            change: PeerClusterChange::default(),
        };
        assert_eq!(
            change.subject(),
            format!("peer.{WALLET_LOWER}.cluster_change")
        );
        let lookup = ClusterLookup {
            wallet: WALLET.into(),
            session: address("02"),
        };
        assert_eq!(
            lookup.subject(),
            format!("peer.{WALLET_LOWER}.cluster_lookup")
        );
    }

    /// Every case the parsers this replaced took or refused: the gatekeeper's subject, wallet and
    /// session handling, and Pulse's lookup parser (which refused a `0X` prefix).
    #[test]
    fn parsers_accept_what_todays_consumers_accept() {
        let payload = PeerClusterChange {
            session: address("AB"),
            displaced_session: address("CD"),
            ..change_message()
        }
        .encode_to_vec();
        for wallet in [
            WALLET.to_string(),
            WALLET.to_uppercase().replacen("0X", "0x", 1),
            WALLET.replacen("0x", "0X", 1),
            WALLET_LOWER.to_string(),
        ] {
            let decoded =
                ClusterChange::decode(&format!("peer.{wallet}.cluster_change"), &payload).unwrap();
            assert_eq!(decoded.wallet, WALLET_LOWER, "{wallet}");
            assert_eq!(decoded.change.session, address("ab"));
            assert_eq!(decoded.change.displaced_session, address("cd"));
            assert_eq!(decoded.change.cluster_id, "c1");

            let lookup = ClusterLookup::decode(
                &format!("peer.{wallet}.cluster_lookup"),
                address("AB").replacen("0x", "0X", 1).as_bytes(),
            )
            .unwrap();
            assert_eq!(lookup.wallet, WALLET_LOWER, "{wallet}");
            assert_eq!(lookup.session, address("ab"));
        }

        let hex40 = &WALLET_LOWER[2..];
        let refused_wallets = [
            format!("peer.{WALLET_LOWER}0.cluster_change"),
            format!("peer.{}.cluster_change", &WALLET_LOWER[..41]),
            format!("peer.{hex40}.cluster_change"),
            format!("peer.1x{hex40}.cluster_change"),
            format!("peer.0x{}g.cluster_change", &hex40[..39]),
            format!("peer.{WALLET_LOWER}.extra.cluster_change"),
            format!("engine.{WALLET_LOWER}.cluster_change"),
            format!("peer.{WALLET_LOWER}"),
            "peer..cluster_change".to_string(),
            "peer.*.cluster_change".to_string(),
            String::new(),
        ];
        for subject in &refused_wallets {
            assert!(
                matches!(
                    ClusterChange::decode(subject, &payload),
                    Err(BusError::Subject)
                ),
                "{subject}"
            );
            let lookup_subject = subject.replace("cluster_change", "cluster_lookup");
            assert!(
                matches!(
                    ClusterLookup::decode(&lookup_subject, address("ab").as_bytes()),
                    Err(BusError::Subject)
                ),
                "{lookup_subject}"
            );
        }

        // Each parser takes only its own event.
        let subject = format!("peer.{WALLET_LOWER}.cluster_lookup");
        let session = address("ab");
        assert!(matches!(
            ClusterChange::decode(&subject, &payload),
            Err(BusError::Subject)
        ));
        assert!(matches!(
            ClusterLookup::decode(
                &format!("peer.{WALLET_LOWER}.cluster_change"),
                session.as_bytes()
            ),
            Err(BusError::Subject)
        ));
        for invalid in [
            format!("{session} "),
            format!(" {session}"),
            format!("{session}.extra"),
            format!("{session}0"),
            session[..41].to_string(),
            session[2..].to_string(),
            format!("0x{}g", &session[2..41]),
            "*".to_string(),
            String::new(),
        ] {
            assert!(
                ClusterLookup::decode(&subject, invalid.as_bytes()).is_err(),
                "{invalid:?}"
            );
        }
        assert!(ClusterLookup::decode(&subject, &[0xff; 42]).is_err());
    }

    /// The subject is read first, so a payload error's subject names the wallet.
    #[test]
    fn a_payload_that_is_not_a_cluster_change_is_a_payload_error() {
        let subject = format!("peer.{WALLET}.cluster_change");
        assert!(matches!(
            ClusterChange::decode(&subject, &[0xff]),
            Err(BusError::Payload(_))
        ));
        assert_eq!(
            ClusterChange::subject_wallet(&subject).as_deref(),
            Some(WALLET_LOWER)
        );
        assert_eq!(
            ClusterChange::subject_wallet(&format!("peer.{WALLET}.cluster_lookup")),
            None
        );
    }

    #[test]
    fn oversized_payloads_are_refused() {
        let subject = format!("peer.{WALLET_LOWER}.cluster_change");
        // A tag byte, a three-byte length and the text: exactly the limit, then one byte over.
        let at_limit = PeerClusterChange {
            realm: "x".repeat(MAX_PAYLOAD_BYTES - 4),
            ..PeerClusterChange::default()
        };
        let bytes = at_limit.encode_to_vec();
        assert_eq!(bytes.len(), MAX_PAYLOAD_BYTES);
        assert_eq!(
            ClusterChange::decode(&subject, &bytes).unwrap().change,
            at_limit
        );

        let over = PeerClusterChange {
            realm: "x".repeat(MAX_PAYLOAD_BYTES - 3),
            ..PeerClusterChange::default()
        };
        let bytes = over.encode_to_vec();
        assert_eq!(bytes.len(), MAX_PAYLOAD_BYTES + 1);
        assert!(matches!(
            ClusterChange::decode(&subject, &bytes),
            Err(BusError::TooLarge)
        ));
        assert!(matches!(
            ClusterLookup::decode(
                &format!("peer.{WALLET_LOWER}.cluster_lookup"),
                &vec![b'0'; MAX_PAYLOAD_BYTES + 1]
            ),
            Err(BusError::TooLarge)
        ));
    }
}
