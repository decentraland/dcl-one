//! `GetAutoGraphicsStream`: each time Auto Graphics settles on lower graphics, the streams get
//! an `AutoGraphicsNotice`. A notice raised before a stream opened is not replayed.

use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        event::EventReader,
        resource::Resource,
        system::{Local, Res, ResMut},
    },
};
use common::{
    rpc::RpcStreamSender,
    structs::{AutoGraphics, AutoGraphicsNotices, BloomSetting, ShadowSetting},
};

use crate::{AutoGraphicsNotice, SystemApi};

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

/// The open streams.
#[derive(Resource, Default)]
struct AutoGraphicsStreams(Vec<RpcStreamSender<AutoGraphicsNotice>>);

/// Keeps new streams, and sends every open stream the notice when the count has moved.
fn publish_auto_graphics(
    mut events: EventReader<SystemApi>,
    notices: Res<AutoGraphicsNotices>,
    mut streams: ResMut<AutoGraphicsStreams>,
    mut seen: Local<Option<u32>>,
) {
    for ev in events.read() {
        if let SystemApi::GetAutoGraphicsStream(stream) = ev {
            streams.0.push(stream.clone());
        }
    }
    // the first run takes the current count, so earlier notices are not replayed
    let last = seen.replace(notices.count);
    streams.0.retain(|stream| !stream.is_closed());
    if last.is_some_and(|last| last != notices.count) {
        let notice = notice(&notices.adjusted);
        for stream in &streams.0 {
            let _ = stream.send(notice.clone());
        }
    }
}

pub(crate) struct AutoGraphicsStreamPlugin;

impl Plugin for AutoGraphicsStreamPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AutoGraphicsNotices>()
            .init_resource::<AutoGraphicsStreams>()
            .add_systems(Update, publish_auto_graphics);
    }
}
