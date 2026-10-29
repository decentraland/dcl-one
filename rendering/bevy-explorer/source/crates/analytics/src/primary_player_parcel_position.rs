// Write a system that fetch the primary player parcel position and send it to the server

use crate::data_definition::{SegmentEvent, SegmentEventExplorerMoveToParcel};
use crate::segment_system::{SegmentConfig, SegmentMetricsEvents};
use bevy::prelude::*;
use common::structs::PrimaryUser;

// TODO: already defined in scene_runner
const PARCEL_SIZE: f32 = 16.0;

pub(crate) fn parcel_of(translation: Vec3) -> IVec2 {
    ((translation.xz() * Vec2::new(1.0, -1.0)) / PARCEL_SIZE)
        .floor()
        .as_ivec2()
}

pub fn primary_player_parcel_position_system(
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    mut last_parcel_position: Local<Option<IVec2>>,
    mut metrics: ResMut<SegmentMetricsEvents>,
    mut segment_config: ResMut<SegmentConfig>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let parcel_position = parcel_of(player.translation());
    if *last_parcel_position != Some(parcel_position) {
        // the spawn is not a move from anywhere
        let current = match *last_parcel_position {
            Some(last) => format!("{},{}", last.x, last.y),
            None => "no-position".to_owned(),
        };
        let next = format!("{},{}", parcel_position.x, parcel_position.y);
        metrics.add_event(
            SegmentEvent::ExplorerMoveToParcel(
                next.clone(),
                SegmentEventExplorerMoveToParcel {
                    old_parcel: current,
                },
            ),
            segment_config.common(),
        );
        segment_config.update_position(next);

        *last_parcel_position = Some(parcel_position);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::rpc::RpcStreamSender;

    // The run's first parcel is the spawn, a move from nowhere; the next is a move from it.
    #[test]
    fn the_spawn_parcel_is_not_a_move_from_anywhere() {
        let mut app = App::new();
        let (sender, mut receiver) = RpcStreamSender::channel();
        let mut metrics = SegmentMetricsEvents::default();
        metrics.add_listener(sender);
        app.insert_resource(metrics)
            .insert_resource(SegmentConfig::new(
                "user".into(),
                "s".into(),
                "1.0".into(),
                None,
                false,
            ))
            .add_systems(Update, primary_player_parcel_position_system);
        let player = app
            .world_mut()
            .spawn((
                PrimaryUser::default(),
                GlobalTransform::from_xyz(8.0, 0.0, -8.0),
            ))
            .id();
        app.update();
        *app.world_mut().get_mut::<GlobalTransform>(player).unwrap() =
            GlobalTransform::from_xyz(24.0, 0.0, -8.0);
        app.update();

        let from: Vec<String> = std::iter::from_fn(|| receiver.try_recv().ok())
            .map(|got| got.properties["old_parcel"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(from, ["no-position", "0,0"]);
        assert_eq!(
            app.world().resource::<SegmentConfig>().common().position,
            "1,0"
        );
    }
}
