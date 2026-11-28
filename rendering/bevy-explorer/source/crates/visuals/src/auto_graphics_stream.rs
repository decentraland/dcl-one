//! `GetAutoGraphicsStream`: each time Auto Graphics settles on lower graphics, the streams get
//! an `AutoGraphicsNotice`. A notice raised before a stream opened is not replayed.

use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        event::{Event, EventReader},
        resource::Resource,
        schedule::{common_conditions::on_event, Condition, IntoScheduleConfigs},
        system::ResMut,
    },
};
use common::{
    rpc::RpcStreamSender,
    structs::{AutoGraphics, BloomSetting, ShadowSetting},
};
use system_bridge::{AutoGraphicsNotice, SystemApi};

/// Auto Graphics settled on lower graphics than it has told the player about; carries what is
/// lowered.
#[derive(Event, Clone, Copy)]
pub(crate) struct AutoGraphicsRaised(pub AutoGraphics);

/// What the HUD is told about lowered graphics.
fn notice(adjusted: &AutoGraphics) -> AutoGraphicsNotice {
    AutoGraphicsNotice {
        shadows_off: adjusted.shadows == Some(ShadowSetting::Off),
        render_scale_percent: adjusted
            .render_scale
            .map(|scale| (scale * 100.0).round() as u32),
        bloom_off: adjusted.bloom == Some(BloomSetting::Off),
    }
}

/// The open streams, and the number of notices raised this session.
#[derive(Resource, Default)]
pub(crate) struct AutoGraphicsStreams {
    streams: Vec<RpcStreamSender<AutoGraphicsNotice>>,
    pub raised: u32,
}

/// Keeps new streams, and sends every open stream each notice raised.
fn publish_auto_graphics(
    mut system_api: EventReader<SystemApi>,
    mut raised: EventReader<AutoGraphicsRaised>,
    mut streams: ResMut<AutoGraphicsStreams>,
) {
    for ev in system_api.read() {
        if let SystemApi::GetAutoGraphicsStream(stream) = ev {
            streams.streams.push(stream.clone());
        }
    }
    streams.streams.retain(|stream| !stream.is_closed());
    for AutoGraphicsRaised(adjusted) in raised.read() {
        streams.raised = streams.raised.wrapping_add(1);
        let notice = notice(adjusted);
        for stream in &streams.streams {
            let _ = stream.send(notice.clone());
        }
    }
}

pub(crate) struct AutoGraphicsStreamPlugin;

impl Plugin for AutoGraphicsStreamPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<AutoGraphicsRaised>()
            .init_resource::<AutoGraphicsStreams>()
            .add_systems(
                Update,
                publish_auto_graphics
                    .run_if(on_event::<AutoGraphicsRaised>.or(on_event::<SystemApi>)),
            );
    }
}
