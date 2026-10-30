//! Where a pass reads. The old collector polled archipelago (`/peers`,
//! `/islands`, `/hot-scenes`), comms (`/scene-participants?pointer=&realm_name=`,
//! `/scene-participants?realm_name=<world>`) and the worlds server
//! (`/live-data`) over HTTP with a client and retry ladder of its own. In v2
//! all three are modules of this process, so [`Sources`] is answered by
//! [`ModuleSources`] over their states: the same rows those routes serialize,
//! without the hop, the JSON or the retries (a module call either answers or
//! fails the pass, and the periodic job retries the pass). The tests answer
//! it with a fake.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use super::model::{HotScene, Island, Peer};
use crate::comms::archipelago;
use crate::comms::archipelago::content::Scene;
use crate::comms::archipelago::feed::IslandReport;
use crate::comms::archipelago::peers::{PeerState, to_parcel};
use crate::comms::gatekeeper;
use crate::comms::gatekeeper::admission::handlers::{ParticipantsQuery, participants};

/// The worlds presence map as `/live-data` serves it.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveData {
    pub total_users: i64,
    pub per_world: Vec<(String, i64)>,
}

#[async_trait]
pub trait Sources: Send + Sync {
    /// Archipelago's connected peers.
    async fn peers(&self) -> anyhow::Result<Vec<Peer>>;
    /// Archipelago's islands, each counting the members this replica knows.
    async fn islands(&self) -> anyhow::Result<Vec<Island>>;
    /// The Genesis scenes with peers on their parcels, busiest first.
    async fn hot_scenes(&self) -> anyhow::Result<Vec<HotScene>>;
    /// The wallets in the room of the Genesis scene at `pointer` in `realm`,
    /// as reported (the collector lowercases and deduplicates them); `None`
    /// when no scene is deployed there (the old 404, which skipped the
    /// occupancy row).
    async fn scene_participants(
        &self,
        realm: &str,
        pointer: &str,
    ) -> anyhow::Result<Option<Vec<String>>>;
    /// The wallets in a world's room; `None` when the world is unknown.
    async fn world_participants(&self, world: &str) -> anyhow::Result<Option<Vec<String>>>;
    /// The worlds presence map; `None` when this node serves none.
    async fn worlds_live_data(&self) -> anyhow::Result<Option<LiveData>>;
}

/// [`Sources`] over the in-process archipelago, comms and worlds states.
pub struct ModuleSources {
    pub archipelago: archipelago::AppState,
    pub comms: gatekeeper::SharedState,
    pub worlds: crate::worlds::core::SharedState,
}

impl ModuleSources {
    pub fn new(
        archipelago: archipelago::AppState,
        comms: gatekeeper::SharedState,
        worlds: crate::worlds::core::SharedState,
    ) -> Arc<Self> {
        Arc::new(Self {
            archipelago,
            comms,
            worlds,
        })
    }

    async fn participants(&self, query: ParticipantsQuery) -> anyhow::Result<Option<Vec<String>>> {
        match participants(&self.comms, query).await {
            Ok(addresses) => Ok(Some(addresses)),
            Err(e) if e.code == 404 => Ok(None),
            Err(e) => anyhow::bail!("scene-participants answered {}: {}", e.code, e.message),
        }
    }
}

#[async_trait]
impl Sources for ModuleSources {
    async fn peers(&self) -> anyhow::Result<Vec<Peer>> {
        Ok(self
            .archipelago
            .peers
            .peers_snapshot()
            .iter()
            .map(peer_of)
            .collect())
    }

    async fn islands(&self) -> anyhow::Result<Vec<Island>> {
        let known = self.archipelago.peers.peers_by_address();
        Ok(self
            .archipelago
            .feed
            .islands()
            .iter()
            .map(|i| island_of(i, &known))
            .collect())
    }

    async fn hot_scenes(&self) -> anyhow::Result<Vec<HotScene>> {
        let peers = self.archipelago.peers.peers_snapshot();
        let per_tile = peers_per_tile(&peers);
        let tiles: Vec<String> = per_tile.keys().cloned().collect();
        let scenes = self
            .archipelago
            .content
            .fetch_scenes(&tiles)
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "hot-scenes: the scene lookup for {} occupied tiles failed",
                    tiles.len()
                )
            })?;
        Ok(hot_scenes_of(&per_tile, scenes))
    }

    async fn scene_participants(
        &self,
        realm: &str,
        pointer: &str,
    ) -> anyhow::Result<Option<Vec<String>>> {
        self.participants(ParticipantsQuery {
            pointer: Some(pointer.to_string()),
            realm_name: Some(realm.to_string()),
            room: None,
        })
        .await
    }

    async fn world_participants(&self, world: &str) -> anyhow::Result<Option<Vec<String>>> {
        self.participants(ParticipantsQuery {
            pointer: None,
            realm_name: Some(world.to_string()),
            room: None,
        })
        .await
    }

    async fn worlds_live_data(&self) -> anyhow::Result<Option<LiveData>> {
        let per_world = self.worlds.presence.world_counts();
        let total_users = per_world.iter().map(|(_, n)| n).sum();
        Ok(Some(LiveData {
            total_users,
            per_world,
        }))
    }
}

/// The `/peers` row: `parcel` from the heartbeat position, `lastPing` in ms.
pub fn peer_of(p: &PeerState) -> Peer {
    let [px, py, pz] = p.position;
    let [x, y] = to_parcel(px, pz);
    Peer {
        address: p.address.clone(),
        parcel_x: Some(x),
        parcel_y: Some(y),
        position_x: Some(px as f64),
        position_y: Some(py as f64),
        position_z: Some(pz as f64),
        last_ping: Some(p.last_heartbeat.timestamp_millis()),
    }
}

/// The `/islands` row: a member this replica never heard a heartbeat from is
/// left out of the count, as the route leaves it out of the list.
pub fn island_of(i: &IslandReport, known: &HashMap<String, PeerState>) -> Island {
    Island {
        island_id: i.id.clone(),
        peer_count: i.peers.iter().filter(|a| known.contains_key(*a)).count() as i32,
        max_peers: Some(i.max_peers as i32),
        center_x: Some(i.center[0] as f64),
        center_y: Some(i.center[1] as f64),
        center_z: Some(i.center[2] as f64),
        radius: Some(i.radius as f64),
    }
}

/// Peers per parcel `x,y`, from the heartbeat positions.
pub fn peers_per_tile(peers: &[PeerState]) -> HashMap<String, u32> {
    let mut per_tile: HashMap<String, u32> = HashMap::new();
    for p in peers {
        let [px, _py, pz] = p.position;
        let [x, y] = to_parcel(px, pz);
        *per_tile.entry(format!("{x},{y}")).or_insert(0) += 1;
    }
    per_tile
}

pub const HOT_SCENES_LIMIT: usize = 100;

/// The `/hot-scenes` rows: each scene's headcount is the sum over its
/// parcels, busiest first, at most [`HOT_SCENES_LIMIT`].
pub fn hot_scenes_of(per_tile: &HashMap<String, u32>, scenes: Vec<Scene>) -> Vec<HotScene> {
    let mut hot: Vec<(u32, HotScene)> = scenes
        .into_iter()
        .map(|scene| {
            let users: u32 = scene
                .parcels
                .iter()
                .map(|tile| per_tile.get(tile).copied().unwrap_or(0))
                .sum();
            let row = HotScene {
                scene_id: scene.id,
                name: scene.name,
                base_x: Some(scene.base[0]),
                base_y: Some(scene.base[1]),
                users_count: Some(users as i32),
                parcel_count: scene.parcels.len() as i32,
                creator: scene.creator,
                description: scene.description,
            };
            (users, row)
        })
        .collect();
    hot.sort_by_key(|(users, _)| std::cmp::Reverse(*users));
    hot.truncate(HOT_SCENES_LIMIT);
    hot.into_iter().map(|(_, row)| row).collect()
}
