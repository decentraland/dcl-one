//! `GetPlatformBanStream`: the platform ban as a `PlatformBanNotice`, now and on every change.

use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        event::EventReader,
        resource::Resource,
        system::{Res, ResMut},
    },
};
use common::rpc::RpcStreamSender;
use system_bridge::{PlatformBanNotice, SystemApi};

use crate::PlatformBanStatus;

/// The open streams and the notice they last got.
#[derive(Resource, Default)]
struct PlatformBanStreams {
    streams: Vec<RpcStreamSender<PlatformBanNotice>>,
    sent: PlatformBanNotice,
}

/// Sends a new stream the notice now, and every stream the notice when it changes.
fn publish_platform_ban(
    mut events: EventReader<SystemApi>,
    status: Res<PlatformBanStatus>,
    mut streams: ResMut<PlatformBanStreams>,
) {
    let current = PlatformBanNotice::from(&*status);
    streams.streams.retain(|stream| !stream.is_closed());
    if streams.sent != current {
        for stream in &streams.streams {
            let _ = stream.send(current.clone());
        }
        streams.sent = current.clone();
    }
    for ev in events.read() {
        if let SystemApi::GetPlatformBanStream(stream) = ev {
            let _ = stream.send(current.clone());
            streams.streams.push(stream.clone());
        }
    }
}

pub(crate) struct PlatformBanStreamPlugin;

impl Plugin for PlatformBanStreamPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlatformBanStreams>()
            .add_systems(Update, publish_platform_ban);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlatformBanInfo, PlatformBanSource};

    fn stream_app() -> App {
        let mut app = App::new();
        app.add_event::<SystemApi>()
            .insert_resource(PlatformBanStatus::new());
        app.add_plugins(PlatformBanStreamPlugin);
        app
    }

    fn ban(reason: &str) -> PlatformBanInfo {
        PlatformBanInfo {
            reason: reason.to_owned(),
            expires_at: Some("2026-12-01T00:00:00.000Z".to_owned()),
            ..PlatformBanInfo::from_refusal("")
        }
    }

    fn set(app: &mut App, f: impl FnOnce(&mut PlatformBanStatus)) {
        f(&mut app.world_mut().resource_mut::<PlatformBanStatus>());
        app.update();
    }

    #[test]
    fn a_stream_gets_the_notice_now_and_on_every_change() {
        let mut app = stream_app();
        let (sender, mut receiver) = RpcStreamSender::channel();
        app.world_mut()
            .send_event(SystemApi::GetPlatformBanStream(sender));
        app.update();
        assert_eq!(receiver.try_recv().ok(), Some(PlatformBanNotice::default()));

        set(&mut app, |s| {
            s.apply_platform_ban(
                Some(PlatformBanInfo {
                    custom_message: Some("go away".into()),
                    ..ban("spam")
                }),
                PlatformBanSource::BanStatusEndpoint,
                1.0,
            )
        });
        assert_eq!(
            receiver.try_recv().ok(),
            Some(PlatformBanNotice {
                banned: true,
                reason: "spam".into(),
                custom_message: Some("go away".into()),
                expires_at: Some("2026-12-01T00:00:00.000Z".into()),
            })
        );

        // a re-check with the same answer sends nothing
        set(&mut app, |s| {
            s.apply_platform_ban(
                Some(PlatformBanInfo {
                    custom_message: Some("go away".into()),
                    ..ban("spam")
                }),
                PlatformBanSource::BanStatusEndpoint,
                2.0,
            )
        });
        assert!(receiver.try_recv().is_err());

        set(&mut app, PlatformBanStatus::reset_platform_ban);
        assert_eq!(receiver.try_recv().ok(), Some(PlatformBanNotice::default()));
    }
}
