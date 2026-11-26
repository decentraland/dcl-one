use bevy::prelude::*;
use common::{
    debug_panic,
    inputs::SystemAction,
    rpc::{RpcCall, RpcCallEvent, RpcOrigin},
    structs::MicState,
};
use dcl_component::proto_components::kernel::apis::{
    VideoTrackSourceType, VideoTracksActiveStreamsData, VideoTracksActiveStreamsResponse,
};
use input_manager::{InputManager, InputPriority};
use livestream_manager::{livekit_track_url, ActiveTransmitter, LIVEKIT_CURRENT_STREAM};
use system_bridge::SystemApi;
use tokio::{sync::mpsc, task::JoinHandle};
#[cfg(not(target_arch = "wasm32"))]
use {
    crate::livekit::LivekitAudioManager,
    common::structs::AudioSettings,
    kira::{
        manager::{AudioManager, AudioManagerSettings, DefaultBackend},
        track::TrackBuilder,
        tween::Tween,
    },
};

#[cfg(feature = "room_debug")]
use crate::livekit::room_debug::RoomDebugPlugin;
use crate::{
    global_crdt::NetworkUpdate,
    livekit::{
        mic::MicPlugin,
        participant::{plugin::LivekitParticipantPlugin, HostingParticipants, LivekitParticipant},
        room::plugin::LivekitRoomPlugin,
        runtime::LivekitRuntimePlugin,
        track::{
            plugin::LivekitTrackPlugin, Camera as LivekitSourceCamera, LivekitTrack, Publishing,
            ScreenshareVideo as LivekitSourceScreenshare, Video,
        },
        ConnectionAvailability, LivekitRuntime, LivekitTransport, StartLivekit,
    },
    voice_chat::{state::Input as VoiceInput, CallMic, VoiceInputs},
    SceneRoom, Transport, TransportType,
};

pub struct LivekitPlugin;

impl Plugin for LivekitPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(not(target_arch = "wasm32"))]
        app.init_resource::<PlayerUpdateTasks>();
        #[cfg(target_arch = "wasm32")]
        app.init_non_send_resource::<PlayerUpdateTasks>();
        app.init_state::<ConnectionAvailability>();

        #[cfg(target_arch = "wasm32")]
        {
            app.insert_resource(crate::livekit::web::take_event_receiver())
                .init_resource::<crate::livekit::web::MicrophoneStatus>()
                .add_systems(PreUpdate, crate::livekit::web::dispatch_page_events);
        }

        // a headless app has no microphone, and plays no voices
        let headless = app
            .world()
            .get_resource::<common::structs::NoRenderApp>()
            .is_some();
        if !headless {
            app.add_plugins(MicPlugin);
        }
        app.add_plugins(LivekitRuntimePlugin);
        app.add_plugins(LivekitRoomPlugin);
        app.add_plugins(super::room::diagnostics::RoomDiagnosticsPlugin);
        app.add_plugins(LivekitParticipantPlugin);
        app.add_plugins(LivekitTrackPlugin);

        app.add_systems(
            Update,
            (
                reset_identity_policy,
                start_livekit,
                verify_player_update_tasks,
            ),
        );
        // on the web the page plays voice (livekit-client), and the engine worker has no audio
        // output; a headless app plays nothing, so opens no audio device
        #[cfg(not(target_arch = "wasm32"))]
        if !headless {
            app.add_systems(Startup, build_kira_audio_manager);
            app.add_systems(
                Update,
                respond_to_audio_settings_change.run_if(
                    resource_exists::<LivekitAudioManager>
                        .and(resource_exists_and_changed::<AudioSettings>),
                ),
            );
        }

        app.add_event::<StartLivekit>();

        app.add_systems(
            Update,
            (handle_input, respond_to_system_api, respond_to_rpc_calls),
        );

        #[cfg(feature = "room_debug")]
        app.add_plugins(RoomDebugPlugin);
    }
}

fn reset_identity_policy(
    wallet: Res<wallet::Wallet>,
    mut identity: Local<Option<alloy_core::primitives::Address>>,
    mut availability: ResMut<NextState<ConnectionAvailability>>,
) {
    if wallet.is_changed() && *identity != wallet.address() {
        *identity = wallet.address();
        availability.set(ConnectionAvailability::Available);
    }
}

/// Holds [`LivekitRuntime`]s, so on the web it is non-send like them (see `runtime.rs`).
#[derive(Default, Deref, DerefMut)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Resource))]
pub(super) struct PlayerUpdateTasks(Vec<PlayerUpdateTask>);

#[cfg(not(target_arch = "wasm32"))]
pub(super) type PlayerUpdateTasksMut<'w> = ResMut<'w, PlayerUpdateTasks>;
#[cfg(target_arch = "wasm32")]
pub(super) type PlayerUpdateTasksMut<'w> = NonSendMut<'w, PlayerUpdateTasks>;

pub(super) struct PlayerUpdateTask {
    pub runtime: LivekitRuntime,
    pub task: JoinHandle<Result<(), mpsc::error::SendError<NetworkUpdate>>>,
}

fn start_livekit(
    mut commands: Commands,
    mut room_events: EventReader<StartLivekit>,
    mut existing: Query<&mut LivekitTransport>,
) {
    for ev in room_events.read() {
        if crate::adapter::livekit_credentials(&ev.address).is_err() {
            warn!("invalid or insecure LiveKit adapter");
            continue;
        }
        let Ok(mut entity) = commands.get_entity(ev.entity) else {
            continue;
        };
        if let Ok(mut transport) = existing.get_mut(ev.entity) {
            transport.address.clone_from(&ev.address);
            entity.try_remove::<super::room::Connecting>();
            entity.try_insert(super::room::Connecting);
            continue;
        }
        info!("starting livekit protocol");
        let (components, sender, control_sender) =
            super::standalone_room_components(ev.address.clone());

        // No connect-time profile announce here — `profile::mod` periodically announces the version
        // over PRIMARY (Pulse + websocket dev server), and the Pulse handshake carries the
        // connect-time version. Peers reach this transport for guest profile request/response by
        // picking a profile-capable transport at request time, not off any announcement of ours.

        entity.try_insert((
            components,
            super::RoomSender {
                sender: sender.clone(),
                control: control_sender.clone(),
            },
        ));
        if let Some(context) = ev.context {
            entity.try_insert(Transport {
                transport_type: TransportType::Livekit,
                sender,
                control: Some(control_sender),
                context,
            });
        }
    }
}

#[cfg(test)]
mod stale_start_tests {
    use super::*;

    #[derive(Resource)]
    struct Departing(Entity);

    #[test]
    fn adapter_start_after_queued_despawn_is_harmless() {
        let mut app = App::new();
        let owner = app.world_mut().spawn_empty().id();
        app.insert_resource(Departing(owner))
            .add_event::<StartLivekit>()
            .add_systems(
                Update,
                (
                    |mut commands: Commands, departing: Res<Departing>| {
                        commands.entity(departing.0).despawn();
                    },
                    start_livekit,
                )
                    .chain_ignore_deferred(),
            );
        app.world_mut().send_event(StartLivekit {
            entity: owner,
            address: "wss://example.org?access_token=test".into(),
            context: None,
        });
        app.update();
        assert!(app.world().get_entity(owner).is_err());
    }
}

fn verify_player_update_tasks(mut player_update_tasks: PlayerUpdateTasksMut) {
    let mut done = vec![];
    for (
        i,
        PlayerUpdateTask {
            runtime,
            ref mut task,
        },
    ) in player_update_tasks.iter_mut().enumerate()
    {
        if task.is_finished() {
            done.push(i);
            let res = runtime.block_on(task);
            match res {
                Ok(res) => {
                    if let Err(err) = res {
                        debug_panic!("Failed to send PlayerUpdate due to {err}.");
                    }
                }
                Err(err) => {
                    debug_panic!("Failed to pull PlayerUpdateTask due to '{err}'.");
                }
            }
        }
    }

    while let Some(i) = done.pop() {
        player_update_tasks.remove(i);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn build_kira_audio_manager(mut commands: Commands, audio_settings: Option<Res<AudioSettings>>) {
    let (scene, voice) = audio_settings
        .map(|s| (s.scene(), s.voice_chat()))
        .unwrap_or((1.0, 1.0));
    let mut manager = match AudioManager::new(AudioManagerSettings::<DefaultBackend>::default()) {
        Ok(manager) => manager,
        Err(err) => {
            debug_panic!("Failed to livekit build AudioManager due to '{err}'.");
        }
    };
    let scene_track = manager.add_sub_track(TrackBuilder::new().volume(scene as f64));
    let voice_track = manager.add_sub_track(TrackBuilder::new().volume(voice as f64));
    match (scene_track, voice_track) {
        (Ok(scene_track), Ok(voice_track)) => {
            debug!("Livekit AudioManager built.");
            commands.insert_resource(LivekitAudioManager {
                manager,
                scene_track,
                voice_track,
            });
        }
        (Err(err), _) | (_, Err(err)) => {
            debug_panic!("Failed to build the livekit audio tracks due to '{err}'.");
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn respond_to_audio_settings_change(
    mut livekit_audio_manager: ResMut<LivekitAudioManager>,
    audio_settings: Res<AudioSettings>,
) {
    livekit_audio_manager
        .scene_track
        .set_volume(audio_settings.scene() as f64, Tween::default());
    livekit_audio_manager
        .voice_track
        .set_volume(audio_settings.voice_chat() as f64, Tween::default());
}

/// In a call, a mic key press held at least this long is push-to-talk; a shorter one leaves the
/// mic open.
const CALL_MIC_HOLD_SECS: f64 = 0.5;

/// A mic key press, from key-down to its release.
enum MicKeyPress {
    /// The scene blocks nearby voice: the press opened nothing and its release closes nothing.
    Refused,
    Nearby,
    Call {
        pressed_at: f64,
        /// The call mic was open at the press.
        was_open: bool,
    },
}

impl MicKeyPress {
    /// Nearby, the release closes the mic. In a call, it closes it after a hold, or when it was
    /// open at the press; once the call has ended it does nothing.
    fn release(
        self,
        released_at: f64,
        mic_state: &mut MicState,
        call_voice_inputs: Option<&mut VoiceInputs>,
    ) {
        match self {
            MicKeyPress::Refused => (),
            MicKeyPress::Nearby => mic_state.enabled = false,
            MicKeyPress::Call {
                pressed_at,
                was_open,
            } => {
                let closes = was_open || released_at - pressed_at >= CALL_MIC_HOLD_SECS;
                if let Some(voice_inputs) = call_voice_inputs.filter(|_| closes) {
                    voice_inputs.0.push_back(VoiceInput::SetMic(false));
                }
            }
        }
    }
}

/// The microphone key drives the nearby mic, or the call mic while a call is connected: a press
/// opens the mic and its release closes it, except that a short press in a call leaves it open.
fn handle_input(
    input_manager: InputManager,
    time: Res<Time<Real>>,
    mut mic_state: ResMut<MicState>,
    mut press: Local<Option<MicKeyPress>>,
    call_mic: Option<Res<CallMic>>,
    mut voice_inputs: Option<ResMut<VoiceInputs>>,
) {
    let now = time.elapsed_secs_f64();
    let call_mic_open = match (&call_mic, &voice_inputs) {
        (Some(call_mic), Some(_)) if call_mic.in_call => Some(call_mic.enabled),
        _ => None,
    };
    let in_call = call_mic_open.is_some();
    // a press lasts while any binding of the action is held; the highest priority reads the
    // bindings whatever has reserved the keyboard
    let released = input_manager.just_up(SystemAction::Microphone)
        && !input_manager.is_button_down(SystemAction::Microphone, InputPriority::BindInput);
    let held = if released { press.take() } else { None };
    let release_pending = released && held.is_none();
    if let Some(held) = held {
        held.release(
            now,
            &mut mic_state,
            voice_inputs.as_deref_mut().filter(|_| in_call),
        );
    }
    if press.is_some() || !input_manager.just_down(SystemAction::Microphone, InputPriority::None) {
        return;
    }
    let pressed = match call_mic_open {
        Some(was_open) => {
            if let Some(voice_inputs) = voice_inputs.as_deref_mut().filter(|_| !was_open) {
                voice_inputs.0.push_back(VoiceInput::SetMic(true));
            }
            MicKeyPress::Call {
                pressed_at: now,
                was_open,
            }
        }
        None if mic_state.scene_blocked() => {
            mic_state.refuse();
            MicKeyPress::Refused
        }
        None => {
            mic_state.enabled = true;
            MicKeyPress::Nearby
        }
    };
    // a tap inside one frame: its release came with the press
    if release_pending {
        pressed.release(
            now,
            &mut mic_state,
            voice_inputs.as_deref_mut().filter(|_| in_call),
        );
    } else {
        *press = Some(pressed);
    }
}

fn respond_to_system_api(
    mut events: EventReader<SystemApi>,
    mut mic: ResMut<MicState>,
    focus: super::mic::MicrophoneFocus,
) {
    for ev in events.read() {
        match ev {
            SystemApi::GetMicState(rpc_result_sender) => {
                rpc_result_sender.send(mic.gated_by_client_window_focus(focus.get()));
            }
            SystemApi::SetMicEnabled(enabled) => {
                mic.request_enabled(*enabled);
            }
            _ => (),
        }
    }
}

fn respond_to_rpc_calls(
    mut rpc_calls: EventReader<RpcCallEvent>,
    participants: Query<(&LivekitParticipant, &Publishing)>,
    tracks: Query<
        (
            &LivekitTrack,
            AnyOf<(&LivekitSourceCamera, &LivekitSourceScreenshare)>,
            Has<ActiveTransmitter>,
        ),
        With<Video>,
    >,
    scene_rooms: Query<(&HostingParticipants, &SceneRoom)>,
) {
    for RpcCallEvent { origin, call } in rpc_calls.read() {
        #[expect(clippy::single_match, reason = "may be expanded later")]
        match call {
            RpcCall::ActiveVideoStreams { response } => {
                let RpcOrigin::Scene { entity: _, hash } = origin else {
                    response.send(VideoTracksActiveStreamsResponse { streams: vec![] });
                    continue;
                };
                let Some((hosting_participants, _)) = scene_rooms
                    .iter()
                    .find(|(_, scene_room)| scene_room.0.as_str() == &**hash)
                else {
                    debug!("No mapping between RpcCall origin and a transport.");
                    continue;
                };

                let mut streams = vec![];
                for (participant, publishing) in
                    participants.iter_many(hosting_participants.collection())
                {
                    for (track, source, has_active_streamer) in
                        tracks.iter_many(publishing.collection())
                    {
                        let identity = participant.identity().0;
                        let track_sid = livekit_track_url(&identity, track.sid());

                        if has_active_streamer {
                            streams.push(VideoTracksActiveStreamsData {
                                identity: participant.identity().0.clone(),
                                track_sid: LIVEKIT_CURRENT_STREAM.to_owned(),
                                source_type: match source {
                                    (Some(_), _) => VideoTrackSourceType::VtstCamera as i32,
                                    (_, Some(_)) => VideoTrackSourceType::VtstScreenShare as i32,
                                    _ => VideoTrackSourceType::VtstUnknown as i32,
                                },
                            });
                        }

                        streams.push(VideoTracksActiveStreamsData {
                            identity: participant.identity().0,
                            track_sid,
                            source_type: match source {
                                (Some(_), _) => VideoTrackSourceType::VtstCamera as i32,
                                (_, Some(_)) => VideoTrackSourceType::VtstScreenShare as i32,
                                _ => VideoTrackSourceType::VtstUnknown as i32,
                            },
                        });
                    }
                }
                response.send(VideoTracksActiveStreamsResponse { streams });
            }
            _ => (),
        }
    }
}

#[cfg(test)]
mod mic_key_tests {
    use std::time::Duration;

    use common::inputs::{Action, InputIdentifier, InputMap};
    use input_manager::{CumulativeAxisData, InputPriorities};

    use super::*;

    /// The time between two frames.
    const FRAME_SECS: f64 = 1.0 / 60.0;
    /// A press shorter than the call mic's hold threshold.
    const TAP_SECS: f64 = CALL_MIC_HOLD_SECS / 2.0;
    /// A press longer than the call mic's hold threshold.
    const HOLD_SECS: f64 = CALL_MIC_HOLD_SECS * 2.0;
    /// A key the tests bind to the mic action beside the default one.
    const SECOND_MIC_KEY: KeyCode = KeyCode::F9;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<InputMap>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CumulativeAxisData>()
            .init_resource::<InputPriorities>()
            .init_resource::<Time<Real>>()
            .init_resource::<MicState>()
            .init_resource::<CallMic>()
            .init_resource::<VoiceInputs>()
            .add_systems(Update, handle_input);
        app
    }

    fn mic_key(app: &App) -> KeyCode {
        app.world().resource::<InputMap>().inputs[&Action::System(SystemAction::Microphone)]
            .iter()
            .find_map(|input| match input {
                InputIdentifier::Key(key) => Some(*key),
                _ => None,
            })
            .unwrap()
    }

    /// One frame, `secs` after the last: the `down` keys go down, then the `up` keys come up.
    fn keys_frame(app: &mut App, secs: f64, down: &[KeyCode], up: &[KeyCode]) {
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_secs_f64(secs));
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.clear();
        for &key in down {
            keys.press(key);
        }
        for &key in up {
            keys.release(key);
        }
        app.update();
    }

    /// One frame, `secs` after the last: the mic key goes down, comes up, or both.
    fn frame(app: &mut App, secs: f64, down: bool, up: bool) {
        let key = mic_key(app);
        let keys = |on: bool| if on { vec![key] } else { vec![] };
        keys_frame(app, secs, &keys(down), &keys(up));
    }

    fn set_call(app: &mut App, in_call: bool, enabled: bool) {
        let mut call_mic = app.world_mut().resource_mut::<CallMic>();
        call_mic.in_call = in_call;
        call_mic.enabled = enabled;
    }

    /// The call mic switches the key queued since the last call.
    fn call_mic_switches(app: &mut App) -> Vec<bool> {
        app.world_mut()
            .resource_mut::<VoiceInputs>()
            .0
            .drain(..)
            .filter_map(|input| match input {
                VoiceInput::SetMic(enabled) => Some(enabled),
                _ => None,
            })
            .collect()
    }

    fn nearby_open(app: &App) -> bool {
        app.world().resource::<MicState>().enabled
    }

    #[test]
    fn nearby_press_opens_and_release_closes() {
        let mut app = app();
        frame(&mut app, FRAME_SECS, true, false);
        assert!(nearby_open(&app));
        frame(&mut app, HOLD_SECS, false, false);
        assert!(nearby_open(&app));
        frame(&mut app, FRAME_SECS, false, true);
        assert!(!nearby_open(&app));
        assert!(call_mic_switches(&mut app).is_empty());
    }

    #[test]
    fn nearby_tap_inside_one_frame_leaves_the_mic_closed() {
        let mut app = app();
        frame(&mut app, FRAME_SECS, true, true);
        assert!(!nearby_open(&app));
        frame(&mut app, FRAME_SECS, false, false);
        assert!(!nearby_open(&app));
    }

    #[test]
    fn nearby_press_with_the_mic_open_keeps_it_open_until_release() {
        let mut app = app();
        app.world_mut().resource_mut::<MicState>().enabled = true;
        frame(&mut app, FRAME_SECS, true, false);
        assert!(nearby_open(&app));
        frame(&mut app, FRAME_SECS, false, true);
        assert!(!nearby_open(&app));
    }

    #[test]
    fn scene_blocked_press_refuses_once_and_its_release_does_nothing() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<MicState>()
            .set_scene_disabled(true);
        frame(&mut app, FRAME_SECS, true, false);
        frame(&mut app, FRAME_SECS, false, true);
        let mic_state = app.world().resource::<MicState>();
        assert_eq!(mic_state.refusals, 1);
        assert!(!mic_state.enabled);
        assert!(call_mic_switches(&mut app).is_empty());
    }

    #[test]
    fn call_tap_from_closed_leaves_the_mic_open() {
        let mut app = app();
        set_call(&mut app, true, false);
        frame(&mut app, FRAME_SECS, true, false);
        assert_eq!(call_mic_switches(&mut app), [true]);
        set_call(&mut app, true, true);
        frame(&mut app, TAP_SECS, false, true);
        assert!(call_mic_switches(&mut app).is_empty());
    }

    #[test]
    fn call_tap_inside_one_frame_leaves_the_mic_open() {
        let mut app = app();
        set_call(&mut app, true, false);
        frame(&mut app, FRAME_SECS, true, true);
        assert_eq!(call_mic_switches(&mut app), [true]);
    }

    #[test]
    fn call_hold_from_closed_is_push_to_talk() {
        let mut app = app();
        set_call(&mut app, true, false);
        frame(&mut app, FRAME_SECS, true, false);
        assert_eq!(call_mic_switches(&mut app), [true]);
        set_call(&mut app, true, true);
        frame(&mut app, HOLD_SECS, false, true);
        assert_eq!(call_mic_switches(&mut app), [false]);
    }

    #[test]
    fn call_press_with_the_mic_open_closes_it_on_release() {
        let mut app = app();
        set_call(&mut app, true, true);
        frame(&mut app, FRAME_SECS, true, false);
        assert!(call_mic_switches(&mut app).is_empty());
        frame(&mut app, FRAME_SECS, false, true);
        assert_eq!(call_mic_switches(&mut app), [false]);
    }

    #[test]
    fn call_ending_during_the_press_makes_the_release_do_nothing() {
        let mut app = app();
        set_call(&mut app, true, false);
        frame(&mut app, FRAME_SECS, true, false);
        assert_eq!(call_mic_switches(&mut app), [true]);
        set_call(&mut app, false, false);
        frame(&mut app, HOLD_SECS, false, true);
        assert!(call_mic_switches(&mut app).is_empty());
        assert!(!nearby_open(&app));
    }

    #[test]
    fn call_starting_during_a_nearby_press_closes_the_nearby_mic_on_release() {
        let mut app = app();
        frame(&mut app, FRAME_SECS, true, false);
        assert!(nearby_open(&app));
        set_call(&mut app, true, false);
        frame(&mut app, FRAME_SECS, false, true);
        assert!(!nearby_open(&app));
        assert!(call_mic_switches(&mut app).is_empty());
    }

    #[test]
    fn a_second_mic_binding_neither_restarts_nor_ends_a_held_press() {
        let mut app = app();
        let mic_key = mic_key(&app);
        app.world_mut()
            .resource_mut::<InputMap>()
            .inputs
            .get_mut(&Action::System(SystemAction::Microphone))
            .unwrap()
            .push(InputIdentifier::Key(SECOND_MIC_KEY));
        set_call(&mut app, true, false);
        keys_frame(&mut app, FRAME_SECS, &[mic_key], &[]);
        assert_eq!(call_mic_switches(&mut app), [true]);
        set_call(&mut app, true, true);
        keys_frame(&mut app, FRAME_SECS, &[SECOND_MIC_KEY], &[]);
        keys_frame(&mut app, HOLD_SECS, &[], &[SECOND_MIC_KEY]);
        assert!(call_mic_switches(&mut app).is_empty());
        keys_frame(&mut app, FRAME_SECS, &[], &[mic_key]);
        assert_eq!(call_mic_switches(&mut app), [false]);
    }
}
