//! The primary player's parcel changes, as the scene runner reports them ([`PlayerParcelChanged`]):
//! `Explorer Move To Parcel` and the reported position on every new parcel, and the node's
//! `move_to_parcel` per login and realm, with the scene there, once in world and with a usable
//! auth chain ([`ChainRetry`]).

use crate::data_definition::{
    SegmentEvent, SegmentEventExplorerMoveToParcel, SegmentEventMoveToParcel,
};
use crate::segment_system::{ChainRetry, SegmentConfig, SegmentMetricsEvents};
use bevy::prelude::*;
use common::structs::{OutOfWorld, PlayerParcelChanged, PrimaryUser};
use wallet::Wallet;

/// The last parcel a `move_to_parcel` was sent for, per login and realm.
#[derive(Default)]
pub(crate) struct ParcelTracker(Option<(String, String, IVec2)>);

impl ParcelTracker {
    /// `Some(old)` when the player is somewhere new: a new parcel, or the same parcel under
    /// another login or realm, whose previous parcel is `None`.
    pub(crate) fn moved(&self, address: &str, realm: &str, parcel: IVec2) -> Option<Option<IVec2>> {
        match &self.0 {
            Some((a, r, p)) if a == address && r == realm => (*p != parcel).then_some(Some(*p)),
            _ => Some(None),
        }
    }

    /// The move to `parcel` was sent.
    pub(crate) fn sent(&mut self, address: &str, realm: &str, parcel: IVec2) {
        self.0 = Some((address.to_owned(), realm.to_owned(), parcel));
    }
}

/// The player's parcel, the scene there and whether the player was in world there, as last
/// reported.
type ReportedParcel = (IVec2, Option<Option<String>>, bool);

/// A move made without a usable chain goes out from the last parcel sent once there is one.
pub(crate) fn report_parcel_changes(
    in_world: Query<(), (With<PrimaryUser>, Without<OutOfWorld>)>,
    mut changes: EventReader<PlayerParcelChanged>,
    mut config: ResMut<SegmentConfig>,
    mut metrics: ResMut<SegmentMetricsEvents>,
    wallet: Option<Res<Wallet>>,
    mut state: Local<(Option<ReportedParcel>, ParcelTracker, ChainRetry)>,
) {
    let (reported, tracker, retry) = &mut *state;
    retry.observe(wallet.as_ref(), config.is_changed());
    for change in changes.read() {
        if change.old != Some(change.parcel) {
            // the spawn is not a move from anywhere
            let old = change.old.map_or_else(
                || "no-position".to_owned(),
                |old| format!("{},{}", old.x, old.y),
            );
            let next = format!("{},{}", change.parcel.x, change.parcel.y);
            metrics.add_event(
                SegmentEvent::ExplorerMoveToParcel(
                    next.clone(),
                    SegmentEventExplorerMoveToParcel { old_parcel: old },
                ),
                config.common(),
            );
            config.update_position(next);
        }
        *reported = Some((change.parcel, change.scene.clone(), change.in_world));
    }

    // the parcel must have been reported in world: a player placed at the spawn point was out of
    // world at the loading-screen parcel. A realm change holds the player out of world at the old
    // realm's parcel before it is reported.
    let Some((parcel, scene, true)) = reported.as_ref() else {
        return;
    };
    if in_world.single().is_err() {
        return;
    }
    let (address, realm) = (&config.common().dcl_eth_address, &config.common().realm);
    let Some(old) = tracker.moved(address, realm, *parcel) else {
        return;
    };
    let Some(common) = retry.client_event_common(&config, wallet.as_deref()) else {
        return;
    };
    tracker.sent(address, realm, *parcel);
    metrics.add_event(
        SegmentEvent::MoveToParcel(SegmentEventMoveToParcel::new(old, *parcel, scene.clone())),
        &common,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment_system::tests::signed_in;
    use chrono::Utc;
    use common::rpc::{RpcStreamReceiver, RpcStreamSender};
    use system_bridge::AnalyticsEvent;

    fn app(config: SegmentConfig) -> (App, Entity, RpcStreamReceiver<AnalyticsEvent>) {
        let mut app = App::new();
        let (sender, receiver) = RpcStreamSender::channel();
        let mut metrics = SegmentMetricsEvents::default();
        metrics.add_listener(sender);
        app.insert_resource(metrics)
            .insert_resource(config)
            .add_event::<PlayerParcelChanged>()
            .add_systems(Update, report_parcel_changes);
        let player = app.world_mut().spawn(PrimaryUser::default()).id();
        (app, player, receiver)
    }

    fn change(
        app: &mut App,
        old: Option<IVec2>,
        parcel: IVec2,
        scene: Option<Option<&str>>,
        in_world: bool,
    ) {
        app.world_mut().send_event(PlayerParcelChanged {
            old,
            parcel,
            scene: scene.map(|scene| scene.map(str::to_owned)),
            in_world,
        });
        app.update();
    }

    fn sent(receiver: &mut RpcStreamReceiver<AnalyticsEvent>) -> Vec<(String, serde_json::Value)> {
        std::iter::from_fn(|| receiver.try_recv().ok())
            .map(|got| (got.event, got.properties))
            .collect()
    }

    fn moves(
        receiver: &mut RpcStreamReceiver<AnalyticsEvent>,
    ) -> Vec<(String, String, Option<String>)> {
        sent(receiver)
            .into_iter()
            .filter(|(event, _)| event == "move_to_parcel")
            .map(|(_, properties)| {
                (
                    properties["old_parcel"].as_str().unwrap().to_owned(),
                    properties["new_parcel"].as_str().unwrap().to_owned(),
                    properties["scene_hash"].as_str().map(str::to_owned),
                )
            })
            .collect()
    }

    // The run's first parcel is the spawn, a move from nowhere; the next is a move from it. A
    // change of the scene alone, or entering the world, is no move.
    #[test]
    fn the_spawn_parcel_is_not_a_move_from_anywhere() {
        let config = SegmentConfig::new("user".into(), "s".into(), "1.0".into(), None, false);
        let (mut app, _, mut receiver) = app(config);
        change(&mut app, None, IVec2::ZERO, None, false);
        change(&mut app, Some(IVec2::ZERO), IVec2::X, None, false);
        change(&mut app, Some(IVec2::X), IVec2::X, Some(None), false);
        change(&mut app, Some(IVec2::X), IVec2::X, Some(None), true);

        let from: Vec<_> = sent(&mut receiver)
            .into_iter()
            .map(|(event, properties)| {
                assert_eq!(event, "Explorer Move To Parcel");
                properties["old_parcel"].as_str().unwrap().to_owned()
            })
            .collect();
        assert_eq!(from, ["no-position", "0,0"]);
        assert_eq!(
            app.world().resource::<SegmentConfig>().common().position,
            "1,0"
        );
    }

    // `move_to_parcel` waits for a usable chain and goes from the last parcel sent; out of world
    // nothing goes, and a realm change at the same parcel is a move from nowhere, with the scene
    // the new realm lists there.
    #[test]
    fn move_to_parcel_goes_per_login_and_realm_once_in_world() {
        let (wallet, config) = signed_in(Utc::now().timestamp_millis(), false);
        let (mut app, player, mut receiver) = app(config);
        app.insert_resource(Wallet::default());

        change(&mut app, None, IVec2::ZERO, Some(Some("genesis")), true);
        change(&mut app, Some(IVec2::ZERO), IVec2::X, Some(None), true);
        assert_eq!(moves(&mut receiver), []);

        app.insert_resource(wallet);
        app.update();
        app.update();
        assert_eq!(
            moves(&mut receiver),
            [("(NaN, NaN)".to_owned(), "(1, 0)".to_owned(), None)]
        );

        // the realm changes before the scene runner reports the player out of world
        app.world_mut()
            .entity_mut(player)
            .insert(OutOfWorld::default());
        app.world_mut()
            .resource_mut::<SegmentConfig>()
            .update_realm("https://worlds.example/world/a.dcl.eth/".into());
        app.update();
        change(
            &mut app,
            Some(IVec2::X),
            IVec2::X,
            Some(Some("world")),
            false,
        );
        assert_eq!(moves(&mut receiver), []);

        app.world_mut().entity_mut(player).remove::<OutOfWorld>();
        app.update();
        change(
            &mut app,
            Some(IVec2::X),
            IVec2::X,
            Some(Some("world")),
            true,
        );
        app.update();
        assert_eq!(
            moves(&mut receiver),
            [(
                "(NaN, NaN)".to_owned(),
                "(1, 0)".to_owned(),
                Some("world".to_owned())
            )]
        );
    }

    // A player let out of the loading screen is placed at a spawn point on another parcel. The
    // frame it is in world, the last report can still be the loading-screen parcel, made out of
    // world: no move goes for it, only the move to the spawn parcel once reported.
    #[test]
    fn a_move_goes_for_the_spawn_parcel_not_the_loading_screen_parcel() {
        let (wallet, config) = signed_in(Utc::now().timestamp_millis(), false);
        let (mut app, player, mut receiver) = app(config);
        app.insert_resource(wallet);
        app.world_mut()
            .entity_mut(player)
            .insert(OutOfWorld::default());

        change(&mut app, None, IVec2::ZERO, Some(Some("loading")), false);
        app.world_mut().entity_mut(player).remove::<OutOfWorld>();
        app.update();
        assert_eq!(moves(&mut receiver), []);

        change(
            &mut app,
            Some(IVec2::ZERO),
            IVec2::X,
            Some(Some("spawn")),
            true,
        );
        assert_eq!(
            moves(&mut receiver),
            [(
                "(NaN, NaN)".to_owned(),
                "(1, 0)".to_owned(),
                Some("spawn".to_owned())
            )]
        );
    }

    // Without a usable chain a pending move is not tried again every frame, only once the wallet
    // changes: a chain that became usable unseen waits for that change.
    #[test]
    fn a_pending_move_is_tried_again_when_the_wallet_changes() {
        let root = wallet::local_guest::new_root_key();
        let (ephemeral, delegates) =
            wallet::local_guest::sign_identity(&root, Utc::now().timestamp_millis()).unwrap();
        let mut config = SegmentConfig::new("user".into(), "s".into(), "1.0".into(), None, false);
        config.update_identity(format!("{:#x}", root.address()), false);
        let (mut app, _, mut receiver) = app(config);
        let mut wallet = Wallet::default();
        app.insert_resource(wallet.clone());
        change(&mut app, None, IVec2::ZERO, Some(None), true);
        app.update();
        assert_eq!(moves(&mut receiver), []);

        // the shared wallet signs in without the resource being marked changed
        wallet.finalize(root.address(), ephemeral, delegates, false);
        app.update();
        app.update();
        assert_eq!(moves(&mut receiver), []);

        app.world_mut().resource_mut::<Wallet>().set_changed();
        app.update();
        assert_eq!(moves(&mut receiver).len(), 1);
    }
}
