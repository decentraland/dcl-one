//! The platform ban as an entry gate. While a ban is known, `PlatformBanEntryGate` is set and the
//! realm change (`ipfs::change_realm`) refuses every request, so a held lobby stays held and a
//! player in a realm stays there. The HUD reads the same ban from `GetPlatformBanStream` to show
//! it. An unknown or failed check leaves the gate open.

use bevy::{
    app::{App, PostUpdate, Update},
    ecs::{
        change_detection::DetectChangesMut,
        event::EventReader,
        resource::Resource,
        schedule::{common_conditions::resource_changed, IntoScheduleConfigs},
        system::{Res, ResMut},
    },
};
use common::{rpc::RpcStreamSender, structs::PlatformBanEntryGate};
use system_bridge::{PlatformBanNotice, SystemApi};

use crate::PlatformBanStatus;

/// The ban as the HUD shows it; not banned unless a ban is known.
pub fn notice(status: &PlatformBanStatus) -> PlatformBanNotice {
    match status.ban() {
        None => PlatformBanNotice::default(),
        Some(ban) => PlatformBanNotice {
            banned: true,
            reason: ban.reason.clone(),
            custom_message: ban.custom_message.clone(),
            expires_at: ban.expires_at.clone(),
        },
    }
}

fn sync_entry_gate(status: Res<PlatformBanStatus>, mut gate: ResMut<PlatformBanEntryGate>) {
    gate.set_if_neq(PlatformBanEntryGate {
        banned: status.ban().is_some(),
    });
}

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
    let current = notice(&status);
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

pub(crate) fn register(app: &mut App) {
    app.init_resource::<PlatformBanEntryGate>()
        .init_resource::<PlatformBanStreams>()
        .add_systems(Update, publish_platform_ban)
        .add_systems(
            PostUpdate,
            sync_entry_gate.run_if(resource_changed::<PlatformBanStatus>),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlatformBanInfo, PlatformBanSource};

    fn gate_app() -> App {
        let mut app = App::new();
        app.add_event::<SystemApi>()
            .insert_resource(PlatformBanStatus::new());
        register(&mut app);
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

    fn gate(app: &App) -> bool {
        app.world().resource::<PlatformBanEntryGate>().banned
    }

    #[test]
    fn the_gate_closes_only_on_a_known_ban() {
        let mut app = gate_app();
        app.update();
        assert!(!gate(&app), "unknown is open");

        set(&mut app, |s| {
            s.platform_ban_check_failed("status 500".into(), 1.0)
        });
        assert!(!gate(&app), "a failed check is open");

        set(&mut app, |s| {
            s.apply_platform_ban(None, PlatformBanSource::BanStatusEndpoint, 2.0)
        });
        assert!(!gate(&app), "not banned is open");

        set(&mut app, |s| {
            s.apply_platform_ban(Some(ban("spam")), PlatformBanSource::BanStatusEndpoint, 3.0)
        });
        assert!(gate(&app));

        // a refusal-learnt ban survives a failed check, so the gate stays closed
        set(&mut app, |s| {
            s.apply_platform_ban(
                Some(PlatformBanInfo::from_refusal("banned")),
                PlatformBanSource::SceneAdmissionRefusal,
                4.0,
            )
        });
        set(&mut app, |s| {
            s.platform_ban_check_failed("status 500".into(), 5.0)
        });
        assert!(gate(&app));

        set(&mut app, PlatformBanStatus::reset_platform_ban);
        assert!(!gate(&app), "a wallet change reopens it");
    }

    #[test]
    fn a_stream_gets_the_notice_now_and_on_every_change() {
        let mut app = gate_app();
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
