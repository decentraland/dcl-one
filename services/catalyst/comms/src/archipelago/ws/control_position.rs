use crate::archipelago::control_v4::V4Owner;
use crate::archipelago::state::AppState;
use crate::protocol::bus::control_position::ControlPosition;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) fn publish(
    state: &AppState,
    owner: &V4Owner,
    realm: &str,
    position: Option<[f32; 3]>,
) -> bool {
    publish_numbered(state, owner, realm, position, &SEQUENCE)
}

/// [`publish`], numbering the record from `sequence`: the next value after the one it holds.
fn publish_numbered(
    state: &AppState,
    owner: &V4Owner,
    realm: &str,
    position: Option<[f32; 3]>,
    sequence: &AtomicU64,
) -> bool {
    let close = position.is_none();
    let Some(permit) =
        state
            .registry
            .control_position_permit(&owner.address, &owner.session, owner.epoch, close)
    else {
        return false;
    };
    let Ok(previous) = sequence.try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        value.checked_add(1)
    }) else {
        return false;
    };
    let record = ControlPosition {
        audience: state
            .gatekeeper
            .cluster
            .control_v4_audience
            .clone()
            .unwrap_or_default(),
        wallet: owner.address.clone(),
        session: owner.session.clone(),
        epoch: owner.epoch,
        sequence: previous + 1,
        realm: realm.to_owned(),
        position,
    };
    crate::archipelago::feed::ControlPositionAnnouncement::new(record, permit)
        .is_some_and(|announcement| state.outbox.publish_control_position(announcement))
}

#[derive(Default)]
pub(crate) struct LegacyProducer {
    published: std::collections::HashMap<String, PublishedCandidate>,
    cursor: usize,
}
struct PublishedCandidate {
    link: crate::archipelago::registry::PeerLink,
    epoch: u64,
    sequence: u64,
    updated: Option<tokio::time::Instant>,
}
impl LegacyProducer {
    pub(crate) async fn publish_current(&mut self, state: &AppState) {
        if state.restart.is_cancelled() {
            return;
        }
        let Some(audience) = state.gatekeeper.cluster.control_v4_audience.as_ref() else {
            return;
        };
        let mut candidates = state.registry.legacy_candidates();
        candidates.sort_by(|a, b| a.address.cmp(&b.address));
        self.published.retain(|address, old| {
            if candidates
                .iter()
                .any(|c| &c.address == address && c.link.id == old.link.id)
            {
                return true;
            }
            if let Some(sequence) = old.sequence.checked_add(1) {
                let record = crate::protocol::bus::control_position::LegacyRoomIntent {
                    audience: audience.clone(),
                    wallet: address.clone(),
                    session: old.link.session.clone(),
                    candidate_epoch: old.epoch,
                    sequence,
                    realm: String::new(),
                    position: None,
                };
                if let Some(announcement) =
                    crate::archipelago::feed::ControlPositionAnnouncement::legacy(
                        record,
                        old.link.id,
                    )
                {
                    state.outbox.publish_control_position(announcement);
                }
            }
            false
        });
        // One bounded batch belongs to the feed job; no allocation spawns a task.
        // Lost nextval responses consume ordering metadata, never room authority.
        let missing: Vec<_> = candidates
            .iter()
            .filter(|candidate| !self.published.contains_key(&candidate.address))
            .collect();
        if state.outbox.is_linked() && !missing.is_empty() {
            use futures::StreamExt;
            self.cursor %= missing.len();
            let count = missing.len().min(
                crate::gatekeeper::admission::cluster::subscriber::wallet_queue::MAX_RUNNING_WORK,
            );
            let mut allocations = futures::stream::FuturesUnordered::new();
            for offset in 0..count {
                let candidate = (*missing[(self.cursor + offset) % missing.len()]).clone();
                let authority = state.control_v4.clone();
                allocations.push(async move {
                    let epoch = tokio::time::timeout(
                        super::AUTHORITY_OPERATION_TIMEOUT,
                        authority.next_connection_epoch(),
                    )
                    .await;
                    (candidate, epoch)
                });
            }
            self.cursor = (self.cursor + count) % missing.len();
            while let Some((candidate, result)) = allocations.next().await {
                if let Ok(Ok(epoch)) = result {
                    if !state.restart.is_cancelled()
                        && state.registry.bind_legacy_candidate(&candidate, epoch)
                    {
                        self.published.insert(
                            candidate.address.clone(),
                            PublishedCandidate {
                                link: candidate.link.clone(),
                                epoch,
                                sequence: 0,
                                updated: None,
                            },
                        );
                    }
                }
            }
        }
        // Allocation may have awaited while a newer heartbeat replaced the retained sample.
        for candidate in state.registry.legacy_candidates() {
            let Some(current) = self.published.get_mut(&candidate.address) else {
                continue;
            };
            if current.link.id != candidate.link.id
                || current.updated == Some(candidate.updated)
                || !state.registry.legacy_candidate_is_current(
                    &candidate.address,
                    &candidate.link.session,
                    candidate.link.id,
                    current.epoch,
                )
            {
                continue;
            }
            let Some(sequence) = current.sequence.checked_add(1) else {
                continue;
            };
            current.sequence = sequence;
            let record = crate::protocol::bus::control_position::LegacyRoomIntent {
                audience: audience.clone(),
                wallet: candidate.address,
                session: candidate.link.session,
                candidate_epoch: current.epoch,
                sequence,
                realm: candidate.realm,
                position: Some(candidate.position),
            };
            if let Some(announcement) =
                crate::archipelago::feed::ControlPositionAnnouncement::legacy(
                    record,
                    current.link.id,
                )
            {
                if state.outbox.publish_control_position(announcement) {
                    current.updated = Some(candidate.updated);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archipelago::config::Settings;
    use crate::archipelago::test_support::gatekeeper;
    use crate::gatekeeper::bus::InProcessBus;
    use crate::protocol::bus::control_position::SUBJECT;
    use std::sync::Arc;
    use std::time::Duration;

    /// A fenced heartbeat reaches Pulse over the gatekeeper's link, as one record whose bytes are
    /// the ones the connector's own client published.
    #[tokio::test]
    async fn control_positions_publish_on_the_gatekeeper_link() {
        let bus = Arc::new(InProcessBus::new());
        let mut shared = gatekeeper();
        let comms = Arc::get_mut(&mut shared).expect("unshared test gatekeeper");
        comms.cluster.control_v4_audience = Some("realm-a".into());
        comms.bus = bus.clone();
        let state = crate::archipelago::build_state(
            Settings::for_tests(),
            shared,
            &crate::jobs::Jobs::new(),
        )
        .await
        .unwrap();
        let owner = V4Owner {
            address: "0x000000000000000000000000000000000000000a".into(),
            session: "0x000000000000000000000000000000000000000b".into(),
            epoch: 7,
        };
        let (_link, _events) = state.registry.on_v4_peer_connected_with_realm_epoch(
            &owner.address,
            &owner.session,
            "connection-7",
            true,
            owner.epoch,
        );
        let sequence = AtomicU64::new(41);
        assert!(publish_numbered(
            &state,
            &owner,
            "main",
            Some([1.0, 2.0, 3.0]),
            &sequence
        ));

        let published = || {
            bus.published()
                .into_iter()
                .filter(|(subject, _)| subject == SUBJECT)
                .map(|(_, payload)| payload)
                .collect::<Vec<_>>()
        };
        tokio::time::timeout(Duration::from_secs(3), async {
            while published().is_empty() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("the position never reached the link");
        let published = published();
        assert_eq!(published.len(), 1);
        let expected = ControlPosition {
            audience: "realm-a".into(),
            wallet: owner.address.clone(),
            session: owner.session.clone(),
            epoch: 7,
            sequence: 42,
            realm: "main".into(),
            position: Some([1.0, 2.0, 3.0]),
        };
        assert_eq!(published[0], expected.encode().unwrap());
        assert_eq!(sequence.load(Ordering::Relaxed), 42);
        assert_eq!(state.outbox.dropped(), 0);
    }
}
