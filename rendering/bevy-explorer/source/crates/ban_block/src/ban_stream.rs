//! `GetPlatformBanStream`: the platform ban as a `PlatformBanNotice`, now and on every change.
//! `GetSceneBanStream`: the current scene's scene or world ban as a `SceneBanNotice`, the same
//! way.

use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        event::EventReader,
        resource::Resource,
        schedule::{
            common_conditions::{on_event, resource_changed},
            Condition, IntoScheduleConfigs,
        },
        system::{Res, ResMut},
    },
};
use common::rpc::RpcStreamSender;
use system_bridge::{PlatformBanNotice, SceneBanNotice, SceneBanScope, SystemApi};

use crate::{AdmissionView, BanScope, PlatformBanStatus, SceneAdmission};

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

/// The open scene-ban streams and the notice they last got.
#[derive(Resource, Default)]
struct SceneBanStreams {
    streams: Vec<RpcStreamSender<SceneBanNotice>>,
    sent: SceneBanNotice,
}

/// The current scene's refusal as the HUD shows it: a scene or world ban. A platform ban has
/// its own notice, and an unknown or allowed admission is no ban.
fn scene_ban_notice(admission: &SceneAdmission) -> SceneBanNotice {
    let scope = match admission.view() {
        AdmissionView::Refused {
            scope: BanScope::Scene,
            ..
        } => Some(SceneBanScope::Scene),
        AdmissionView::Refused {
            scope: BanScope::World,
            ..
        } => Some(SceneBanScope::World),
        _ => None,
    };
    SceneBanNotice { scope }
}

/// Sends a new stream the notice now, and every stream the notice when it changes.
fn publish_scene_ban(
    mut events: EventReader<SystemApi>,
    admission: Res<SceneAdmission>,
    mut streams: ResMut<SceneBanStreams>,
) {
    let current = scene_ban_notice(&admission);
    streams.streams.retain(|stream| !stream.is_closed());
    if streams.sent != current {
        for stream in &streams.streams {
            let _ = stream.send(current.clone());
        }
        streams.sent = current.clone();
    }
    for ev in events.read() {
        if let SystemApi::GetSceneBanStream(stream) = ev {
            let _ = stream.send(current.clone());
            streams.streams.push(stream.clone());
        }
    }
}

pub(crate) struct PlatformBanStreamPlugin;

impl Plugin for PlatformBanStreamPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlatformBanStreams>()
            .init_resource::<SceneBanStreams>()
            .add_systems(
                Update,
                (
                    publish_platform_ban
                        .run_if(resource_changed::<PlatformBanStatus>.or(on_event::<SystemApi>)),
                    publish_scene_ban
                        .run_if(resource_changed::<SceneAdmission>.or(on_event::<SystemApi>)),
                ),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlatformBanInfo, PlatformBanSource};

    fn stream_app() -> App {
        let mut app = App::new();
        app.add_event::<SystemApi>()
            .insert_resource(PlatformBanStatus::new())
            .init_resource::<SceneAdmission>();
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

    fn admit(app: &mut App, admission: SceneAdmission) {
        *app.world_mut().resource_mut::<SceneAdmission>() = admission;
        app.update();
    }

    fn scene_refusal() -> SceneAdmission {
        SceneAdmission::from_refusal(403, Some("User is banned from this scene"), false, "s")
    }

    #[test]
    fn a_scene_ban_stream_gets_the_scope_now_and_on_every_change() {
        let mut app = stream_app();
        let (sender, mut receiver) = RpcStreamSender::channel();
        app.world_mut()
            .send_event(SystemApi::GetSceneBanStream(sender));
        app.update();
        assert_eq!(receiver.try_recv().ok(), Some(SceneBanNotice::default()));

        admit(&mut app, scene_refusal());
        assert_eq!(
            receiver.try_recv().ok(),
            Some(SceneBanNotice {
                scope: Some(SceneBanScope::Scene)
            })
        );

        // a recheck with the same answer sends nothing
        admit(&mut app, scene_refusal());
        assert!(receiver.try_recv().is_err());

        admit(&mut app, SceneAdmission::allowed("s"));
        assert_eq!(receiver.try_recv().ok(), Some(SceneBanNotice::default()));
    }

    #[test]
    fn a_world_ban_reports_world_and_a_platform_refusal_reports_none() {
        let world =
            SceneAdmission::from_refusal(401, Some("You are banned from world x"), true, "s");
        assert_eq!(scene_ban_notice(&world).scope, Some(SceneBanScope::World));
        let platform = SceneAdmission::from_refusal(
            403,
            Some("Access denied, platform-banned user"),
            false,
            "s",
        );
        assert_eq!(scene_ban_notice(&platform).scope, None);
        assert_eq!(scene_ban_notice(&SceneAdmission::unknown()).scope, None);
    }
}
