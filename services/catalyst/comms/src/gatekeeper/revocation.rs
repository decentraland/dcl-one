//! The one revocation entry point: a permission or block edit made outside
//! the gatekeeper (worlds' core maps its `Revocation` onto
//! [`RoomRevocation`]) evicts the affected participants from the running
//! LiveKit rooms through [`CommsState::revoke`], detached from the caller (an
//! SFU outage is logged under `comms-revocation`, never surfaced to the
//! edit). Untested against a real SFU until one is dialled; the dev
//! placeholders skip it.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::Address;
use crate::access::Access;
use crate::interfaces::AccessPolicy;
use crate::shared::identity::AuthorityNotEstablished;
use crate::world_identity::WorldName;

use super::CommsState;
use super::admission::scene::{kick, live_world_rooms, readmit_world_rooms};

/// What an edit revoked, in the rooms' terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoomRevocation {
    /// One wallet lost its admission to the world: it leaves the world's
    /// live rooms.
    WorldAddress { world: WorldName, address: Address },
    /// The world's access setting changed: every participant it no longer
    /// admits leaves the world's live rooms (the owner stays).
    WorldAccess { world: WorldName },
    /// The wallet is blocked everywhere: it leaves every live room.
    Everywhere { address: Address },
}

/// Whether an access edit removes `address` from `world`'s rooms: only on a
/// decided refusal (the old access-change handler), never the owner or a
/// deployer unless blocked or platform-banned (a ban beats the privilege).
/// After an access-setting change a shared-secret participant must mint
/// again with the new secret.
pub(crate) async fn evictable(
    policy: &dyn AccessPolicy,
    world: &WorldName,
    privileged: &BTreeSet<Address>,
    address: &Address,
    access_change: bool,
) -> Result<bool, AuthorityNotEstablished> {
    let now = chrono::Utc::now().naive_utc();
    let access = policy
        .can_access(world, address, None, None, None, now)
        .await?;
    if matches!(access, Access::Banned(_)) {
        return Ok(true);
    }
    if privileged.contains(address) {
        return Ok(false);
    }
    Ok(match access {
        Access::Allowed(_) => false,
        Access::InvalidSecret => access_change,
        Access::Banned(_) | Access::NotAllowed => true,
    })
}

impl CommsState {
    /// Enforces `revocation` in the running LiveKit rooms on a spawned task;
    /// returns at once. LiveKit not configured evicts nothing.
    pub fn revoke(self: &Arc<Self>, revocation: RoomRevocation) {
        if !self.livekit.configured {
            tracing::debug!(
                ?revocation,
                "LiveKit not configured; no participant evicted"
            );
            return;
        }
        let state = self.clone();
        tokio::spawn(async move {
            match &revocation {
                RoomRevocation::WorldAddress { world, address } => {
                    let decision = match state.policy.privileged_addresses(world).await {
                        Ok(privileged) => {
                            evictable(state.policy.as_ref(), world, &privileged, address, false)
                                .await
                        }
                        Err(error) => Err(error),
                    };
                    match decision {
                        Ok(true) => {}
                        Ok(false) => return,
                        Err(error) => {
                            tracing::warn!(%error, %world, %address, "revocation undecided; nobody evicted");
                            return;
                        }
                    }
                    let rooms = match live_world_rooms(&state, world).await {
                        Ok(rooms) => rooms,
                        Err(error) => {
                            tracing::warn!(%error, %world, "revocation inventory incomplete; no completion claimed");
                            return;
                        }
                    };
                    kick(&state, &rooms, address).await;
                }
                RoomRevocation::WorldAccess { world } => {
                    if let Err(error) = readmit_world_rooms(&state, world).await {
                        tracing::warn!(%error, %world, "revocation inventory incomplete; no completion claimed");
                        return;
                    }
                }
                RoomRevocation::Everywhere { address } => {
                    if let Err(error) = super::removal::operator_block(&state.pool, address).await {
                        tracing::error!(%error,%address,"operator-block removal intent was not accepted; worlds write is already committed");
                        return;
                    }
                }
            }
            tracing::info!(
                job = "comms-revocation",
                ?revocation,
                "LiveKit revocation dispatched"
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::{AccessGrant, Ban};
    use crate::interfaces::test_support::StubPolicy;

    fn wallet(byte: char) -> Address {
        Address::parse(&format!("0x{}", byte.to_string().repeat(40))).unwrap()
    }

    async fn decide(
        access: Result<Access, AuthorityNotEstablished>,
        privileged: &[Address],
        access_change: bool,
    ) -> Result<bool, AuthorityNotEstablished> {
        let policy = StubPolicy {
            access,
            ..StubPolicy::default()
        };
        evictable(
            &policy,
            &WorldName::parse("alice.dcl.eth").unwrap(),
            &privileged.iter().cloned().collect(),
            &wallet('a'),
            access_change,
        )
        .await
    }

    #[tokio::test]
    async fn owner_and_deployers_stay_unless_banned() {
        for access_change in [false, true] {
            for access in [Access::NotAllowed, Access::InvalidSecret] {
                assert!(
                    !decide(Ok(access), &[wallet('a')], access_change)
                        .await
                        .unwrap()
                );
            }
            for ban in [Ban::Platform, Ban::Blocked] {
                assert!(
                    decide(Ok(Access::Banned(ban)), &[wallet('a')], access_change)
                        .await
                        .unwrap(),
                    "a {ban:?} privileged wallet is evicted"
                );
            }
        }
    }

    #[tokio::test]
    async fn only_a_decided_refusal_evicts() {
        let cases = [
            (Access::Allowed(AccessGrant::AllowListed), false, false),
            (Access::NotAllowed, false, true),
            (Access::Banned(Ban::Platform), false, true),
            (Access::InvalidSecret, false, false),
            (Access::InvalidSecret, true, true),
        ];
        for (access, access_change, expected) in cases {
            assert_eq!(
                decide(Ok(access.clone()), &[wallet('b')], access_change)
                    .await
                    .unwrap(),
                expected,
                "{access:?} access_change={access_change}"
            );
        }
        assert!(
            decide(
                Err(AuthorityNotEstablished::store_unavailable(
                    "worlds.worlds",
                    &sqlx::Error::PoolTimedOut,
                )),
                &[],
                true,
            )
            .await
            .is_err(),
            "an undecided answer is never an eviction"
        );
    }
}
