use std::collections::VecDeque;

use bevy::prelude::*;
use common::structs::PrimaryUser;
use dcl_types::interface::CrdtStore;

use crate::{
    initialize_scene::SuperUserScene, renderer_context::RendererSceneContext, ContainingScene,
};

const MAX_TICKS: usize = 256;

#[derive(Clone, Copy, Default)]
pub(super) struct Batch {
    pub(super) bytes: usize,
    pub(super) updates: usize,
}

impl Batch {
    fn measure(crdt: &CrdtStore) -> Self {
        let mut batch = Self::default();
        for state in crdt.lww.values() {
            batch.updates += state.last_write.len();
            batch.bytes += state
                .last_write
                .values()
                .filter(|v| v.is_some)
                .map(|v| v.data.len())
                .sum::<usize>();
        }
        for values in crdt.go.values().flat_map(|state| state.0.values()) {
            batch.updates += values.len();
            batch.bytes += values.iter().map(|v| v.data.len()).sum::<usize>();
        }
        batch
    }
}

pub(super) struct Tick {
    pub(super) dt: f64,
    pub(super) sent: Batch,
    pub(super) received: Batch,
}

/// One send/receive pair per completed tick on the player's current ordinary parcel scene.
#[derive(Component, Default)]
pub(super) struct SceneTraffic {
    sent: Option<Batch>,
    pub(super) ticks: VecDeque<Tick>,
}

impl SceneTraffic {
    pub(super) fn record_sent(&mut self, crdt: &CrdtStore) {
        self.sent = Some(Batch::measure(crdt));
    }

    pub(super) fn record_received(&mut self, crdt: &CrdtStore, dt: f64) {
        let Some(sent) = self.sent.take() else {
            return;
        };
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        if self.ticks.len() == MAX_TICKS {
            self.ticks.pop_front();
        }
        self.ticks.push_back(Tick {
            dt,
            sent,
            received: Batch::measure(crdt),
        });
    }
}

pub(super) fn track_parcel(
    mut commands: Commands,
    containing: ContainingScene,
    player: Query<Entity, With<PrimaryUser>>,
    scenes: Query<&RendererSceneContext, Without<SuperUserScene>>,
    histories: Query<Entity, With<SceneTraffic>>,
) {
    let current = player
        .single()
        .ok()
        .and_then(|player| containing.get_parcel(player))
        .filter(|root| scenes.get(*root).is_ok_and(|scene| !scene.is_portable));
    let previous = histories.single().ok();
    if current != previous {
        if let Some(previous) = previous {
            commands.entity(previous).try_remove::<SceneTraffic>();
        }
        if let Some(current) = current {
            commands.entity(current).try_insert(SceneTraffic::default());
        }
    }
}
