//! The scene listener's AoI, compacted to fit the Pulse server's 4096-byte receive buffer.

use std::collections::BTreeMap;

use bevy::math::IVec2;
use common::bounds_calc::scene_regions;
use dcl_component::proto_components::pulse;
use prost::Message as _;

/// Encoded AoI cap, leaving room for the handshake's auth chain (450-800 bytes) under 4096.
pub const MAX_AOI_BYTES: usize = 2560;

/// One hosted scene: the realm it is announced in and the parcels it covers.
#[derive(Debug, Clone)]
pub struct HostedScene {
    pub realm: String,
    pub scene: String,
    pub parcels: Vec<IVec2>,
}

/// What compaction changed, for the logs.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Compaction {
    /// The encoded size of the exact AoI, when it was over budget.
    pub exact_bytes: usize,
    /// The scenes announced as their bounding box.
    pub collapsed: Vec<Collapsed>,
    /// The realms left out of the announcement.
    pub dropped: Vec<String>,
}

impl Compaction {
    pub fn is_empty(&self) -> bool {
        self.collapsed.is_empty() && self.dropped.is_empty()
    }
}

/// A scene announced as its bounding box, and how many rects that saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collapsed {
    pub realm: String,
    pub scene: String,
    pub rects: usize,
}

/// The encoded size of a `ClientMessage` carrying `aoi` as a `SceneListenerUpdate`.
pub fn encoded_len(aoi: &[pulse::SceneListenerAoi]) -> usize {
    pulse::ClientMessage {
        message: Some(pulse::client_message::Message::SceneListenerUpdate(
            pulse::SceneListenerUpdate { aoi: aoi.to_vec() },
        )),
    }
    .encoded_len()
}

/// Per-realm AoI sorted by realm: exact within `max_bytes`, otherwise compacted.
pub fn listener_aoi(
    scenes: &[HostedScene],
    max_bytes: usize,
) -> (Vec<pulse::SceneListenerAoi>, Compaction) {
    let mut by_realm: BTreeMap<&str, Vec<IVec2>> = BTreeMap::new();
    for scene in scenes {
        by_realm
            .entry(&scene.realm)
            .or_default()
            .extend(&scene.parcels);
    }
    let exact: Vec<_> = by_realm
        .into_iter()
        .map(|(realm, mut parcels)| {
            parcels.sort_unstable_by_key(|p| (p.x, p.y));
            parcels.dedup();
            realm_aoi(realm, regions(parcels))
        })
        .collect();
    let exact_bytes = encoded_len(&exact);
    if exact_bytes <= max_bytes {
        return (exact, Compaction::default());
    }

    let mut scenes: Vec<Shape> = scenes
        .iter()
        .filter(|scene| !scene.parcels.is_empty())
        .map(Shape::new)
        .collect();
    scenes.sort_by(|a, b| (&a.realm, &a.scene).cmp(&(&b.realm, &b.scene)));
    // Most rects first; ties by realm and scene, so the same scenes always collapse.
    let mut order: Vec<usize> = (0..scenes.len()).collect();
    order.sort_by(|&a, &b| scenes[b].rects.cmp(&scenes[a].rects));

    let mut compaction = Compaction {
        exact_bytes,
        ..Default::default()
    };
    let mut aoi = exact;
    for index in order {
        if encoded_len(&aoi) <= max_bytes || scenes[index].rects <= 1 {
            break;
        }
        scenes[index].collapsed = true;
        let scene = &scenes[index];
        compaction.collapsed.push(Collapsed {
            realm: scene.realm.clone(),
            scene: scene.scene.clone(),
            rects: scene.rects,
        });
        let realm = scene.realm.clone();
        if let Some(entry) = aoi.iter_mut().find(|entry| entry.realm == realm) {
            entry.parcel_rects = realm_rects(&scenes, &realm);
        }
    }

    // Largest realms first; ties by name.
    let mut realms: Vec<(usize, String)> = aoi
        .iter()
        .map(|realm| (realm.encoded_len(), realm.realm.clone()))
        .collect();
    realms.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    for (_, realm) in realms {
        if encoded_len(&aoi) <= max_bytes {
            break;
        }
        aoi.retain(|entry| entry.realm != realm);
        compaction.dropped.push(realm);
    }

    (aoi, compaction)
}

/// A scene's parcels, its rect count and bounding box, and whether it is announced as that box.
struct Shape {
    realm: String,
    scene: String,
    parcels: Vec<IVec2>,
    rects: usize,
    bounds: pulse::ParcelRect,
    collapsed: bool,
}

impl Shape {
    fn new(scene: &HostedScene) -> Self {
        let mut parcels = scene.parcels.clone();
        parcels.sort_unstable_by_key(|p| (p.x, p.y));
        parcels.dedup();
        let min = parcels.iter().fold(IVec2::MAX, |a, b| a.min(*b));
        let max = parcels.iter().fold(IVec2::MIN, |a, b| a.max(*b));
        Self {
            realm: scene.realm.clone(),
            scene: scene.scene.clone(),
            rects: regions(parcels.clone()).len(),
            parcels,
            bounds: rect(min, max),
            collapsed: false,
        }
    }
}

/// `realm`'s rects: its other scenes' parcels merged into blocks, then each collapsed scene's box.
fn realm_rects(scenes: &[Shape], realm: &str) -> Vec<pulse::ParcelRect> {
    let in_realm = || scenes.iter().filter(move |scene| scene.realm == realm);
    let mut parcels: Vec<IVec2> = in_realm()
        .filter(|scene| !scene.collapsed)
        .flat_map(|scene| scene.parcels.iter().copied())
        .collect();
    parcels.sort_unstable_by_key(|p| (p.x, p.y));
    parcels.dedup();
    let mut rects = regions(parcels);
    rects.extend(
        in_realm()
            .filter(|scene| scene.collapsed)
            .map(|scene| scene.bounds.clone()),
    );
    rects
}

/// `parcels`, sorted, as one rect per contiguous rectangular block.
fn regions(parcels: Vec<IVec2>) -> Vec<pulse::ParcelRect> {
    scene_regions(parcels.into_iter())
        .into_iter()
        .map(|region| rect(region.min, region.max))
        .collect()
}

fn rect(min: IVec2, max: IVec2) -> pulse::ParcelRect {
    pulse::ParcelRect {
        min_x: min.x,
        min_z: min.y,
        max_x: max.x,
        max_z: max.y,
    }
}

fn realm_aoi(realm: &str, parcel_rects: Vec<pulse::ParcelRect>) -> pulse::SceneListenerAoi {
    pulse::SceneListenerAoi {
        realm: realm.to_owned(),
        parcel_rects,
    }
}

#[cfg(test)]
pub(crate) mod tests;
