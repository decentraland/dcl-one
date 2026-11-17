//! The local player's saved outfits: the catalyst `outfits` entity on `<address>:outfits`, signed
//! and deployed here for the bridge scene.

use std::sync::Arc;

use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use common::{
    rpc::RpcResultSender,
    util::{TaskCompat, TaskExt},
};
use ipfs::IpfsAssetServer;
use system_bridge::{OutfitsData, SystemApi};
use wallet::Wallet;

use super::{deploy_entity, CurrentUserProfile};

/// The catalyst accepts slots 0 to 9.
pub const MAX_OUTFIT_SLOT: u32 = 9;
/// Slots past this one need a name in `namesForExtraSlots`.
const MAX_FREE_OUTFIT_SLOT: u32 = 4;

/// A deploy in flight, and who waits on its result.
type PendingOutfitsDeploy = (
    Task<Result<(), String>>,
    RpcResultSender<Result<(), String>>,
);

#[derive(Resource, Default)]
pub struct OutfitsState {
    deploys: Vec<PendingOutfitsDeploy>,
    /// The outcome of the last deploy that settled, for the debug panel.
    pub last_deploy: Option<Result<(), String>>,
}

impl OutfitsState {
    pub fn deploys_in_flight(&self) -> usize {
        self.deploys.len()
    }
}

/// The signed-in web3 account: guests have no catalyst outfits and keep nothing on disk.
fn web3_account(wallet: &Wallet, current: &CurrentUserProfile) -> Option<String> {
    let web3 = current
        .profile
        .as_ref()
        .is_some_and(|p| p.content.has_connected_web3.unwrap_or_default());
    web3.then(|| wallet.address())
        .flatten()
        .map(|a| format!("{a:#x}"))
}

/// The signed-in web3 account, when it is the one the caller expects.
fn expected_account(
    expected: &str,
    wallet: &Wallet,
    current: &CurrentUserProfile,
) -> Result<String, String> {
    let account = web3_account(wallet, current)
        .ok_or_else(|| "only a web3 account has outfits".to_owned())?;
    if !account.eq_ignore_ascii_case(expected) {
        return Err("another account is signed in".to_owned());
    }
    Ok(account)
}

/// Rejects what the catalyst would without a lookup: slots out of 0..=9, repeated, or past 4 with
/// no name. The catalyst still checks that the deployer owns the names.
pub fn check_outfits(outfits: &OutfitsData) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    let has_name = outfits
        .names_for_extra_slots
        .iter()
        .any(|n| !n.trim().is_empty());
    for slot in outfits.outfits.iter().map(|o| o.slot) {
        if slot > MAX_OUTFIT_SLOT {
            return Err(format!("outfit slot {slot} is out of range"));
        }
        if !seen.insert(slot) {
            return Err(format!("outfit slot {slot} is repeated"));
        }
        if slot > MAX_FREE_OUTFIT_SLOT && !has_name {
            return Err(format!("outfit slot {slot} needs an owned name"));
        }
    }
    Ok(())
}

pub fn process_outfits(
    mut requests: EventReader<SystemApi>,
    mut state: ResMut<OutfitsState>,
    current: Res<CurrentUserProfile>,
    wallet: Res<Wallet>,
    ipfas: IpfsAssetServer,
) {
    for request in requests.read() {
        if let SystemApi::DeployOutfits(expected, outfits, sender) = request {
            let account = match expected_account(expected, &wallet, &current) {
                Ok(account) => account,
                Err(e) => {
                    sender.send(Err(e));
                    continue;
                }
            };
            if let Err(e) = check_outfits(outfits) {
                sender.send(Err(e));
                continue;
            }
            let metadata = match serde_json::to_value(outfits) {
                Ok(metadata) => metadata,
                Err(e) => {
                    sender.send(Err(e.to_string()));
                    continue;
                }
            };
            let ipfs = Arc::clone(ipfas.ipfs());
            let wallet = wallet.clone();
            let task = IoTaskPool::get().spawn_compat(async move {
                deploy_entity(
                    ipfs,
                    wallet,
                    "outfits",
                    format!("{account}:outfits"),
                    metadata,
                )
                .await
                .map_err(|e| e.to_string())
            });
            state.deploys.push((task, sender.clone()));
        }
    }

    let mut index = 0;
    while index < state.deploys.len() {
        let Some(result) = state.deploys[index].0.complete() else {
            index += 1;
            continue;
        };
        let (_, sender) = state.deploys.remove(index);
        match &result {
            Ok(()) => info!("deployed outfits"),
            Err(e) => error!("failed to deploy outfits: {e}"),
        }
        state.last_deploy = Some(result.clone());
        sender.send(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use system_bridge::{OutfitColor, OutfitColored, OutfitData, OutfitSlotData};

    fn slot(slot: u32) -> OutfitSlotData {
        let grey = OutfitColored {
            color: OutfitColor {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 1.0,
            },
        };
        OutfitSlotData {
            slot,
            outfit: OutfitData {
                body_shape: "urn:decentraland:off-chain:base-avatars:BaseFemale".to_owned(),
                eyes: grey,
                hair: grey,
                skin: grey,
                wearables: vec![],
                force_render: vec![],
            },
        }
    }

    fn outfits(slots: &[u32]) -> OutfitsData {
        OutfitsData {
            outfits: slots.iter().copied().map(slot).collect(),
            names_for_extra_slots: vec![],
        }
    }

    #[test]
    fn free_slots_within_range_and_distinct_pass() {
        assert_eq!(check_outfits(&outfits(&[0, 2, 4])), Ok(()));
    }

    #[test]
    fn an_extra_slot_with_a_name_passes() {
        let mut named = outfits(&[0, 9]);
        named.names_for_extra_slots = vec!["somename".to_owned()];
        assert_eq!(check_outfits(&named), Ok(()));
    }

    #[test]
    fn an_extra_slot_without_a_name_is_refused() {
        assert!(check_outfits(&outfits(&[5])).is_err());
        let mut blank = outfits(&[5]);
        blank.names_for_extra_slots = vec!["  ".to_owned()];
        assert!(check_outfits(&blank).is_err());
    }

    #[test]
    fn a_slot_past_nine_is_refused() {
        assert!(check_outfits(&outfits(&[10])).is_err());
    }

    #[test]
    fn a_repeated_slot_is_refused() {
        assert!(check_outfits(&outfits(&[2, 2])).is_err());
    }

    #[test]
    fn the_metadata_has_the_catalyst_entity_shape() {
        let json = serde_json::to_value(outfits(&[1])).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "outfits": [{
                    "slot": 1,
                    "outfit": {
                        "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseFemale",
                        "eyes": {"color": {"r": 0.5, "g": 0.5, "b": 0.5, "a": 1.0}},
                        "hair": {"color": {"r": 0.5, "g": 0.5, "b": 0.5, "a": 1.0}},
                        "skin": {"color": {"r": 0.5, "g": 0.5, "b": 0.5, "a": 1.0}},
                        "wearables": [],
                        "forceRender": []
                    }
                }],
                "namesForExtraSlots": []
            })
        );
    }

    #[test]
    fn a_color_without_alpha_reads_as_opaque() {
        let color: OutfitColor =
            serde_json::from_value(serde_json::json!({"r": 0.1, "g": 0.2, "b": 0.3})).unwrap();
        assert_eq!(color.a, 1.0);
        let kept: OutfitColor =
            serde_json::from_value(serde_json::json!({"r": 0.1, "g": 0.2, "b": 0.3, "a": 0.5}))
                .unwrap();
        assert_eq!(kept.a, 0.5);
    }
}
