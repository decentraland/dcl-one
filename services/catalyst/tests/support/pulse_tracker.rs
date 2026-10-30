#![allow(dead_code)]
use catalyst::comms::pulse::{
    cluster::nats::{NatsClusterFeed, NatsFeedOptions},
    cluster::{ClusterOptions, ClusterTracker},
    decentraland::common::Vector3,
    interest::SPATIAL_GRID_CELL_SIZE,
    realm_grids::RealmSpatialGrids,
    snapshot::{IdentityBoard, PeerSnapshot, SnapshotBoard},
};
use std::sync::Arc;
const CAP: usize = 8;

fn address(number: usize) -> String {
    format!("0x{number:040x}")
}

pub struct World {
    grids: RealmSpatialGrids,
    board: SnapshotBoard,
    identity: IdentityBoard,
    tracker: ClusterTracker,
    feed: Arc<NatsClusterFeed>,
}

impl World {
    pub fn new(url: String) -> Self {
        Self::with_options(NatsFeedOptions {
            url,
            capacity: 1,
            discovery_interval_ms: 0,
            ..Default::default()
        })
    }

    pub fn with_options(options: NatsFeedOptions) -> Self {
        let feed = Arc::new(NatsClusterFeed::spawn(options));
        Self {
            grids: RealmSpatialGrids::new(SPATIAL_GRID_CELL_SIZE, CAP),
            board: SnapshotBoard::new(CAP, 4),
            identity: IdentityBoard::new(CAP),
            tracker: ClusterTracker::new(ClusterOptions::default(), CAP, feed.clone()),
            feed,
        }
    }

    pub fn enable_control_positions(&self, audience: &str) {
        self.feed.enable_control_positions(audience).unwrap();
    }

    pub fn place(&mut self, peer: u32, wallet: &str, session: &str) {
        let position = Vector3 {
            x: 1.0,
            y: 0.0,
            z: 1.0,
        };
        self.board.set_active(peer);
        self.board.publish(
            peer,
            PeerSnapshot {
                global_position: position,
                realm: Some("test-realm".into()),
                ..Default::default()
            },
        );
        self.identity
            .set_with_session(peer, wallet.into(), session.into());
        self.grids.set(peer, "test-realm", position);
    }

    pub fn remove(&mut self, peer: u32) {
        self.grids.remove(peer);
        self.identity.remove(peer);
        self.board.clear_active(peer);
    }

    pub fn pass(&mut self) {
        self.tracker
            .run_pass(&self.grids, &self.board, &self.identity);
    }
}
