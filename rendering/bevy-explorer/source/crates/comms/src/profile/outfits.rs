//! The local player's saved outfits: the catalyst `outfits` entity on `<address>:outfits`, signed
//! and deployed here for the bridge scene, and each slot's picture, kept by the engine on this
//! device (scenes get no storage of their own).

use std::{collections::VecDeque, sync::Arc};

use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use common::{
    rpc::RpcResultSender,
    util::{TaskCompat, TaskExt},
};
use ipfs::IpfsAssetServer;
use system_bridge::{OutfitThumbnailData, OutfitsData, SystemApi};
use wallet::Wallet;

use super::{deploy_entity, CurrentUserProfile};

/// The catalyst accepts slots 0 to 9.
pub const MAX_OUTFIT_SLOT: u32 = 9;
/// Slots past this one need a name in `namesForExtraSlots`.
const MAX_FREE_OUTFIT_SLOT: u32 = 4;
/// A slot's picture is a PNG data URL of the backpack preview.
const THUMBNAIL_PREFIX: &str = "data:image/png;base64,";
const MAX_THUMBNAIL_BYTES: usize = 1024 * 1024;

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
    thumbnails: VecDeque<ThumbnailOp>,
    running: Option<Task<()>>,
}

impl OutfitsState {
    pub fn deploys_in_flight(&self) -> usize {
        self.deploys.len()
    }
}

enum ThumbnailOp {
    Read {
        account: String,
        sender: RpcResultSender<Vec<OutfitThumbnailData>>,
    },
    Write {
        account: String,
        slot: u32,
        thumbnail: Option<String>,
    },
}

impl ThumbnailOp {
    async fn run(self) {
        match self {
            ThumbnailOp::Read { account, sender } => {
                let thumbnails = platform::outfit_thumbnails_read(&account)
                    .await
                    .unwrap_or_else(|e| {
                        warn!("failed to read outfit thumbnails: {e}");
                        Vec::new()
                    });
                sender.send(
                    thumbnails
                        .into_iter()
                        .map(|t| OutfitThumbnailData {
                            slot: t.slot,
                            thumbnail: t.thumbnail,
                        })
                        .collect(),
                );
            }
            ThumbnailOp::Write {
                account,
                slot,
                thumbnail,
            } => {
                if let Err(e) =
                    platform::outfit_thumbnail_write(&account, slot, thumbnail.as_deref()).await
                {
                    warn!("failed to store outfit thumbnail {slot}: {e}");
                }
            }
        }
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

fn check_thumbnail(slot: u32, thumbnail: Option<&str>) -> bool {
    slot <= MAX_OUTFIT_SLOT
        && thumbnail
            .is_none_or(|t| t.starts_with(THUMBNAIL_PREFIX) && t.len() <= MAX_THUMBNAIL_BYTES)
}

pub fn process_outfits(
    mut requests: EventReader<SystemApi>,
    mut state: ResMut<OutfitsState>,
    current: Res<CurrentUserProfile>,
    wallet: Res<Wallet>,
    ipfas: IpfsAssetServer,
) {
    for request in requests.read() {
        match request {
            SystemApi::DeployOutfits(outfits, sender) => {
                let Some(account) = web3_account(&wallet, &current) else {
                    sender.send(Err("only a web3 account deploys outfits".to_owned()));
                    continue;
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
            SystemApi::GetOutfitThumbnails(sender) => match web3_account(&wallet, &current) {
                Some(account) => state.thumbnails.push_back(ThumbnailOp::Read {
                    account,
                    sender: sender.clone(),
                }),
                None => sender.send(Vec::new()),
            },
            SystemApi::SetOutfitThumbnail(slot, thumbnail) => {
                let Some(account) = web3_account(&wallet, &current) else {
                    continue;
                };
                if !check_thumbnail(*slot, thumbnail.as_deref()) {
                    warn!("ignoring an invalid outfit thumbnail for slot {slot}");
                    continue;
                }
                state.thumbnails.push_back(ThumbnailOp::Write {
                    account,
                    slot: *slot,
                    thumbnail: thumbnail.clone(),
                });
            }
            _ => (),
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

    // one store operation at a time, in the order asked: a write then a read sees the write
    if state
        .running
        .as_mut()
        .is_some_and(|t| t.complete().is_some())
    {
        state.running = None;
    }
    if state.running.is_none() {
        if let Some(op) = state.thumbnails.pop_front() {
            state.running = Some(IoTaskPool::get().spawn_compat(op.run()));
        }
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
                        "eyes": {"color": {"r": 0.5, "g": 0.5, "b": 0.5}},
                        "hair": {"color": {"r": 0.5, "g": 0.5, "b": 0.5}},
                        "skin": {"color": {"r": 0.5, "g": 0.5, "b": 0.5}},
                        "wearables": [],
                        "forceRender": []
                    }
                }],
                "namesForExtraSlots": []
            })
        );
    }

    #[test]
    fn only_png_data_urls_within_the_cap_are_kept() {
        assert!(check_thumbnail(0, Some("data:image/png;base64,AAAA")));
        assert!(check_thumbnail(0, None));
        assert!(!check_thumbnail(0, Some("file:///etc/passwd")));
        assert!(!check_thumbnail(10, None));
        let big = format!("{THUMBNAIL_PREFIX}{}", "A".repeat(MAX_THUMBNAIL_BYTES));
        assert!(!check_thumbnail(0, Some(&big)));
    }
}
