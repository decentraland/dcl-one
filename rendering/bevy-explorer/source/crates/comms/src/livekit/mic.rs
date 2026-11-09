#[cfg(not(target_arch = "wasm32"))]
use std::borrow::Cow;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

use bevy::{ecs::relationship::Relationship, prelude::*};
use common::structs::{MicState, SystemAudio};
use tokio::task::JoinHandle;
#[cfg(target_arch = "wasm32")]
use {
    bevy::render::view::RenderLayers,
    common::{structs::AudioSettings, util::AsH160, util::VolumePanning},
};
#[cfg(not(target_arch = "wasm32"))]
use {
    cpal::{
        traits::{DeviceTrait, HostTrait, StreamTrait},
        Device,
    },
    livekit::{
        options::TrackPublishOptions,
        participant::Participant,
        track::{LocalAudioTrack, LocalTrack, TrackSource},
        webrtc::{
            audio_source::native::NativeAudioSource,
            prelude::{AudioFrame, AudioSourceOptions, RtcAudioSource},
        },
    },
    tokio::sync::broadcast,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::global_crdt::{LocalAudioFrame, LocalAudioSource};
use crate::livekit::{
    participant::{HostedBy, LivekitParticipant, Local as LivekitLocalParticipant},
    LivekitRuntimeRes,
};
#[cfg(target_arch = "wasm32")]
use crate::{
    global_crdt::{ForeignAudioSource, ForeignPlayer, NearbyPanning, VoicePan},
    livekit::{
        track::{Audio, LivekitTrack, Publishing, Subscribed},
        web::{
            prompt_microphone_permission, setup_microphone_permission, AudioCaptureOptions,
            LocalAudioTrack, LocalTrack, MicrophoneStatus, Participant, TrackPublishOptions,
            TrackSource,
        },
    },
};

#[path = "mic_debug.rs"]
mod debug;

pub struct MicPlugin;

impl Plugin for MicPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(not(target_arch = "wasm32"))]
        app.init_non_send_resource::<MicStream>();

        debug::register(app);
        app.init_state::<MicrophoneAvailability>();
        app.init_state::<MicrophonePermission>();
        app.init_state::<MicrophoneState>();
        app.init_state::<MicrophoneTransmissionState>();
        app.init_resource::<MicrophoneDrain>();

        #[cfg(target_arch = "wasm32")]
        app.add_systems(Startup, setup_microphone_permission);
        app.add_systems(
            Update,
            (
                #[cfg(target_arch = "wasm32")]
                poll_microphone_permission,
                verify_availability.run_if(in_state(MicrophoneAvailability::Unavailable)),
                verify_microphone_device_health.run_if(in_state(MicrophoneAvailability::Available)),
                verify_enabled.run_if(
                    in_state(MicrophoneAvailability::Available)
                        .and(in_state(MicrophoneState::Disabled)),
                ),
                verify_disabled.run_if(in_state(MicrophoneState::Enabled)),
                // Revocation runs even when permission or device availability was lost.
                reconcile_publications,
            )
                .chain()
                .after(crate::voice_room::update_voice_room),
        );
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(OnEnter(MicrophoneState::Enabled), build_cpal_stream);
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(OnEnter(MicrophoneState::Disabled), drop_cpal_stream);
        #[cfg(target_arch = "wasm32")]
        app.add_systems(
            OnEnter(MicrophoneState::Enabled),
            prompt_microphone_permission.run_if(in_state(MicrophonePermission::Prompt)),
        );
        #[cfg(target_arch = "wasm32")]
        app.add_systems(
            OnEnter(MicrophonePermission::Prompt),
            prompt_microphone_permission.run_if(in_state(MicrophoneState::Enabled)),
        );
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(OnEnter(MicrophoneAvailability::Available), setup_microphone);
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(
            OnExit(MicrophoneAvailability::Available),
            tear_down_microphone,
        );

        app.add_systems(Startup, add_ping_systems);
        app.add_systems(Update, transmission_state);

        #[cfg(not(target_arch = "wasm32"))]
        app.insert_state(MicrophonePermission::Granted);

        #[cfg(target_arch = "wasm32")]
        app.add_systems(Update, locate_foreign_streams);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, States)]
enum MicrophoneAvailability {
    #[default]
    Unavailable,
    Available,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, States)]
enum MicrophonePermission {
    #[default]
    Denied,
    #[cfg(target_arch = "wasm32")]
    Prompt,
    Granted,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, States)]
enum MicrophoneState {
    #[default]
    Disabled,
    Enabled,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, States)]
enum MicrophoneTransmissionState {
    #[default]
    Disabled,
    Enabled,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Deref, DerefMut)]
struct MicrophoneDevice(Device);

/// New routes wait until every revoked route has acknowledged stopping its source.
#[derive(Resource, Default, Clone)]
pub(crate) struct MicrophoneDrain(Arc<AtomicUsize>);

impl MicrophoneDrain {
    pub(crate) fn pending(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
struct PublicationControl {
    track: Option<LocalAudioTrack>,
    revoked: bool,
    complete: bool,
    draining: bool,
    transmitting: bool,
}

/// Dropping a participant or changing its route revokes its async work, including work which
/// has not created a track yet. The task continues only to stop/unpublish its own track.
#[derive(Component)]
pub(crate) struct MicrophonePublication {
    state: Arc<Mutex<PublicationControl>>,
    revoked: Arc<AtomicBool>,
    cancel: tokio::sync::watch::Sender<bool>,
    drain: MicrophoneDrain,
    task: JoinHandle<()>,
}

impl MicrophonePublication {
    pub(crate) fn revoke(&self) {
        self.revoked.store(true, Ordering::SeqCst);
        let mut state = self.state.lock().unwrap_or_else(|err| err.into_inner());
        state.revoked = true;
        state.transmitting = false;
        if let Some(track) = &state.track {
            mute_local_track(track, true);
        }
        if !state.complete && !state.draining {
            state.draining = true;
            self.drain.0.fetch_add(1, Ordering::SeqCst);
        }
        let _ = self.cancel.send(true);
    }
}

impl Drop for MicrophonePublication {
    fn drop(&mut self) {
        self.revoke();
    }
}

fn complete_publication(state: &Mutex<PublicationControl>, drain: &MicrophoneDrain, stopped: bool) {
    let mut state = state.lock().unwrap_or_else(|err| err.into_inner());
    state.complete = true;
    state.transmitting = false;
    // Missing browser acknowledgment fails closed, even after the owner is gone.
    if state.draining && stopped {
        state.draining = false;
        drain.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn mute_local_track(track: &LocalAudioTrack, muted: bool) {
    #[cfg(not(target_arch = "wasm32"))]
    if muted {
        track.mute();
    } else {
        track.unmute();
    }
    #[cfg(target_arch = "wasm32")]
    track.set_enabled(!muted);
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default, Deref, DerefMut)]
struct MicStream(Option<cpal::Stream>);

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct CaptureFormat {
    sample_rate: u32,
    channels: u32,
}

fn add_ping_systems(world: &mut World) {
    // This stops the ping being played during startup
    world.resource_scope::<Schedules, ()>(|_, mut schedules| {
        schedules.add_systems(OnEnter(MicrophoneTransmissionState::Enabled), ping_enabled);
        schedules.add_systems(
            OnEnter(MicrophoneTransmissionState::Disabled),
            ping_disabled,
        );
    });
}

fn transmission_state(
    tracks: Query<&MicrophonePublication>,
    microphone_transmission_state: Res<State<MicrophoneTransmissionState>>,
    mut next_state: ResMut<NextState<MicrophoneTransmissionState>>,
) {
    let transmitting = tracks.iter().any(|publication| {
        let state = publication
            .state
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        state.transmitting
    });
    if !transmitting && microphone_transmission_state.get() == &MicrophoneTransmissionState::Enabled
    {
        next_state.set(MicrophoneTransmissionState::Disabled);
    } else if transmitting
        && microphone_transmission_state.get() == &MicrophoneTransmissionState::Disabled
    {
        next_state.set(MicrophoneTransmissionState::Enabled);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn verify_availability(mut commands: Commands, mut mic_state: ResMut<MicState>) {
    let default_host = cpal::default_host();
    let maybe_device = default_host.default_input_device();

    if let Some(device) = maybe_device {
        debug!(
            "Default microphone '{}' set as input device.",
            device
                .name()
                .expect("Shouldn't became unavailable in such a short span.")
        );
        mic_state.available = true;
        commands.set_state(MicrophoneAvailability::Available);
    }
}

#[cfg(target_arch = "wasm32")]
fn verify_availability(
    mut commands: Commands,
    mut mic_state: ResMut<MicState>,
    microphone_status: Res<MicrophoneStatus>,
) {
    // Check if microphone is available in the browser
    let current_available = microphone_status.available;

    // Only update availability if it changed
    if current_available {
        debug!("Microphone became available.");
        mic_state.available = true;
        commands.set_state(MicrophoneAvailability::Available);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn verify_microphone_device_health(
    mut commands: Commands,
    microphone_device: Res<MicrophoneDevice>,
    mut mic_state: ResMut<MicState>,
) {
    if let Err(err) = microphone_device.name() {
        debug!("Microphone device became unavailable due to '{err}'.");
        mic_state.available = false;
        commands.set_state(MicrophoneAvailability::Unavailable);
    }
}

#[cfg(target_arch = "wasm32")]
fn verify_microphone_device_health(
    mut commands: Commands,
    mut mic_state: ResMut<MicState>,
    microphone_status: Res<MicrophoneStatus>,
) {
    // Check if microphone is available in the browser
    let current_available = microphone_status.available;

    if !current_available {
        debug!("Microphone became unavailable.");
        mic_state.available = false;
        commands.set_state(MicrophoneAvailability::Unavailable);
    }
}

#[cfg(target_arch = "wasm32")]
fn poll_microphone_permission(
    mut commands: Commands,
    microphone_permission: Res<State<MicrophonePermission>>,
    microphone_status: Res<MicrophoneStatus>,
) {
    match microphone_status.permission.as_str() {
        "granted" => {
            if *microphone_permission.get() != MicrophonePermission::Granted {
                debug!("Granted microphone permission.");
                commands.set_state(MicrophonePermission::Granted);
            }
        }
        "prompt" => {
            if *microphone_permission.get() != MicrophonePermission::Prompt {
                debug!("Microphone permission needs to be prompted.");
                commands.set_state(MicrophonePermission::Prompt);
            }
        }
        "denied" => {
            if *microphone_permission.get() != MicrophonePermission::Denied {
                debug!("Denied microphone permission.");
                commands.set_state(MicrophonePermission::Denied);
            }
        }
        other => panic!("Unknown microphone permission '{}'.", other),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn setup_microphone(mut commands: Commands) {
    let default_host = cpal::default_host();
    let maybe_device = default_host.default_input_device();

    if let Some(device) = maybe_device {
        commands.insert_resource(MicrophoneDevice(device));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn tear_down_microphone(mut commands: Commands) {
    commands.remove_resource::<MicrophoneDevice>();
}

fn verify_enabled(mut commands: Commands, mic_state: Res<MicState>) {
    if mic_state.enabled {
        debug!("Microphone is now enabled.");
        commands.set_state(MicrophoneState::Enabled);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn build_cpal_stream(
    mut commands: Commands,
    microphone_device: Res<MicrophoneDevice>,
    local_audio_source: Res<LocalAudioSource>,
    mut mic_stream: NonSendMut<MicStream>,
) {
    let Ok(config) = microphone_device
        .default_input_config()
        .inspect_err(|err| error!("{err}"))
    else {
        // Do not change state until `MicState::enabled` changes to prevent
        // log spam
        return;
    };
    let sender = local_audio_source.sender.clone();

    let num_channels = config.channels() as u32;
    let sample_rate = config.sample_rate().0;

    let new_stream = microphone_device
        .build_input_stream(
            &config.into(),
            move |data_f32: &[f32], _: &cpal::InputCallbackInfo| {
                let mut data_uninit = Arc::new_uninit_slice(data_f32.len());
                let data_slice = Arc::get_mut(&mut data_uninit).unwrap();
                for (src, dest) in data_f32.iter().zip(data_slice.iter_mut()) {
                    dest.write((*src * i16::MAX as f32).round() as i16);
                }
                // SAFETY: we have initialized all 'len' elements
                let data = unsafe { data_uninit.assume_init() };
                if sender
                    .send(LocalAudioFrame {
                        data: data.to_owned(),
                        sample_rate,
                        num_channels,
                        samples_per_channel: data.len() as u32 / num_channels,
                    })
                    .is_err()
                {
                    warn!("mic channel closed?");
                }
            },
            |err: cpal::StreamError| {
                warn!("mic error: {err}");
            },
            None,
        )
        .unwrap();

    if let Err(err) = new_stream.play() {
        error!("{err}");
    } else {
        debug!("Cpal stream built.");
        commands.set_state(MicrophoneState::Enabled);
        **mic_stream = Some(new_stream);
        commands.insert_resource(CaptureFormat {
            sample_rate,
            channels: num_channels,
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn drop_cpal_stream(mut commands: Commands, mut mic_stream: NonSendMut<MicStream>) {
    **mic_stream = None;
    commands.remove_resource::<CaptureFormat>();
    commands.set_state(MicrophoneState::Disabled);
}

fn verify_disabled(mut commands: Commands, mic_state: Res<MicState>) {
    if !mic_state.enabled {
        debug!("Microphone is now disabled.");
        commands.set_state(MicrophoneState::Disabled);
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn reconcile_publications(
    mut commands: Commands,
    participants: Query<
        (
            Entity,
            &LivekitParticipant,
            &HostedBy,
            Option<&MicrophonePublication>,
        ),
        With<LivekitLocalParticipant>,
    >,
    rooms: Query<(
        Has<crate::Transport>,
        Has<crate::private_chat::PrivateChatRoom>,
        Has<super::room::Connected>,
    )>,
    route: Res<crate::voice_room::ActiveVoiceRoom>,
    mic: Res<MicState>,
    permission: Res<State<MicrophonePermission>>,
    availability: Res<State<MicrophoneAvailability>>,
    drain: Res<MicrophoneDrain>,
    livekit_runtime: LivekitRuntimeRes,
    #[cfg(not(target_arch = "wasm32"))] source: Res<LocalAudioSource>,
    #[cfg(not(target_arch = "wasm32"))] format: Option<Res<CaptureFormat>>,
) {
    let enabled = mic.enabled
        && *permission.get() == MicrophonePermission::Granted
        && *availability.get() == MicrophoneAvailability::Available;
    let eligible = |room: Entity| {
        rooms
            .get(room)
            .is_ok_and(|(proximity, private, connected)| {
                enabled && connected && route.routing.allows(room, proximity, private)
            })
    };

    // Revoke all old routes before considering any new one. Removing the component runs Drop
    // at the next command flush; avoid starting a replacement in this same update.
    let mut revoking = false;
    for (entity, _, room, publication) in &participants {
        if let Some(publication) = publication {
            if !eligible(room.get())
                || publication.revoked.load(Ordering::SeqCst)
                || publication.task.is_finished()
            {
                commands.entity(entity).remove::<MicrophonePublication>();
                revoking = true;
            }
        }
    }
    if revoking || drain.pending() != 0 {
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    let Some(format) = format
    else {
        return;
    };

    for (entity, participant, room, publication) in &participants {
        if publication.is_some() || !eligible(room.get()) {
            continue;
        }
        let Participant::Local(participant) = &**participant else {
            continue;
        };
        let participant = participant.clone();
        let state = Arc::new(Mutex::new(PublicationControl::default()));
        let task_state = state.clone();
        let revoked = Arc::new(AtomicBool::new(false));
        let task_revoked = revoked.clone();
        let task_drain = drain.clone();
        let (cancel, mut cancellation) = tokio::sync::watch::channel(false);
        #[cfg(not(target_arch = "wasm32"))]
        let receiver = source.subscribe();
        #[cfg(not(target_arch = "wasm32"))]
        let (sample_rate, channels) = (format.sample_rate, format.channels);

        let task = livekit_runtime.spawn(async move {
            #[cfg(not(target_arch = "wasm32"))]
            let (track, capture) =
                build_audio_local_track(receiver, sample_rate, channels, task_revoked.clone())
                    .await;
            #[cfg(target_arch = "wasm32")]
            let track = build_audio_local_track().await;
            // Both backends create tracks muted. Publishing itself may outlive cancellation;
            // a canceled publication is never unmuted.
            {
                let mut state = task_state.lock().unwrap_or_else(|err| err.into_inner());
                state.track = Some(track.clone());
            }
            let mut published = false;
            if !task_revoked.load(Ordering::SeqCst) {
                published = participant
                    .publish_track(
                        LocalTrack::Audio(track.clone()),
                        TrackPublishOptions {
                            source: TrackSource::Microphone,
                            ..Default::default()
                        },
                    )
                    .await
                    .is_ok();
                if !published {
                    warn!("microphone publication failed");
                }
            }
            if published {
                {
                    let mut state = task_state.lock().unwrap_or_else(|err| err.into_inner());
                    if !state.revoked {
                        mute_local_track(&track, false);
                        state.transmitting = true;
                    }
                }
                if !task_revoked.load(Ordering::SeqCst) {
                    let _ = cancellation.changed().await;
                }
            }

            // Stop the source before releasing the drain barrier. On web this is an acknowledged
            // page operation, so no later route races an asynchronous old media operation.
            mute_local_track(&track, true);
            #[cfg(not(target_arch = "wasm32"))]
            capture.abort();
            #[cfg(target_arch = "wasm32")]
            let stopped = track.stop(!published).await.is_ok();
            #[cfg(not(target_arch = "wasm32"))]
            let stopped = true;
            if published {
                #[cfg(not(target_arch = "wasm32"))]
                let unpublished = participant.unpublish_track(&track.sid()).await;
                #[cfg(target_arch = "wasm32")]
                let unpublished = participant.unpublish_track(&LocalTrack::Audio(track)).await;
                if unpublished.is_err() {
                    warn!("microphone unpublish failed after stopping capture");
                }
            }
            complete_publication(&task_state, &task_drain, stopped);
        });
        commands.entity(entity).insert(MicrophonePublication {
            state,
            revoked,
            cancel,
            drain: drain.clone(),
            task,
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn locate_foreign_streams(
    mut streams: Query<(
        &GlobalTransform,
        Option<&RenderLayers>,
        &ForeignAudioSource,
        &ForeignPlayer,
        &mut VoicePan,
    )>,
    participants: Query<(&LivekitParticipant, &Publishing, &HostedBy)>,
    proximity_rooms: Query<(), With<crate::Transport>>,
    tracks: Query<&LivekitTrack, (With<Subscribed>, With<Audio>)>,
    pan: VolumePanning,
    settings: Res<AudioSettings>,
    panning_settings: Res<NearbyPanning>,
    time: Res<Time>,
) {
    for (emitter_transform, render_layers, source, player, mut voice_pan) in streams.iter_mut() {
        if source.current_transport.is_some() {
            let Some(publishing) = participants
                .iter()
                .filter_map(|(participant, publishing, room)| {
                    if proximity_rooms.contains(room.get())
                        && participant
                            .identity()
                            .as_str()
                            .as_h160()
                            .filter(|h160| *h160 == player.address)
                            .is_some()
                    {
                        Some(publishing.collection().to_vec())
                    } else {
                        None
                    }
                })
                .reduce(|mut accumulator, mut next| {
                    accumulator.append(&mut next);
                    accumulator
                })
            else {
                error!("No participant with address {}.", player.address);
                continue;
            };

            let (volume, panning) =
                pan.volume_and_panning(emitter_transform.translation(), render_layers);
            let volume = volume * settings.voice();
            let panning = voice_pan.update(panning, &panning_settings, time.delta_secs());
            for livekit_track in tracks.iter_many(publishing) {
                if let Some(track) = livekit_track.track() {
                    track.pan_and_volume(panning, volume);
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn build_audio_local_track(
    mic_receiver: broadcast::Receiver<LocalAudioFrame>,
    sample_rate: u32,
    num_channels: u32,
    revoked: Arc<AtomicBool>,
) -> (LocalAudioTrack, JoinHandle<()>) {
    let new_source = NativeAudioSource::new(
        AudioSourceOptions {
            echo_cancellation: true,
            noise_suppression: true,
            auto_gain_control: true,
        },
        sample_rate,
        num_channels,
        50,
    );
    let local_audio_track =
        LocalAudioTrack::create_audio_track("mic", RtcAudioSource::Native(new_source.clone()));
    local_audio_track.mute();

    let local_audio_track_clone = local_audio_track.clone();
    let capture = tokio::task::spawn(async move {
        let mut mic_receiver = mic_receiver;
        while let Ok(frame) = mic_receiver.recv().await {
            if revoked.load(Ordering::SeqCst) || (frame.sample_rate == 0 && frame.num_channels == 0)
            {
                // Termination frame
                break;
            }
            if let Err(e) = new_source
                .capture_frame(&AudioFrame {
                    data: Cow::Borrowed(&frame.data),
                    sample_rate: frame.sample_rate,
                    num_channels: frame.num_channels,
                    samples_per_channel: frame.samples_per_channel,
                })
                .await
            {
                warn!("failed to capture from mic: {e}");
            };
        }
        debug!(
            "Mic worker for local audio track {} closed.",
            local_audio_track_clone.sid()
        );
    });

    (local_audio_track, capture)
}

#[cfg(target_arch = "wasm32")]
async fn build_audio_local_track() -> LocalAudioTrack {
    LocalAudioTrack::new(AudioCaptureOptions {
        echo_cancellation: Some(true),
        noise_suppression: Some(true),
        auto_gain_control: Some(true),
        ..Default::default()
    })
    .await
    .unwrap_or_else(|err| {
        // as before the page host: the publish of the unusable track fails and is logged
        error!("Failed to create local audio track due to '{err}'.");
        LocalAudioTrack::unavailable()
    })
}

fn ping_enabled(mut commands: Commands) {
    commands.send_event(SystemAudio(
        "embedded://sounds/ui/voice_chat_mic_on.wav".to_owned(),
    ));
}

fn ping_disabled(mut commands: Commands) {
    commands.send_event(SystemAudio(
        "embedded://sounds/ui/voice_chat_mic_off.wav".to_owned(),
    ));
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod routing_tests {
    use super::*;

    fn pending(
        runtime: &tokio::runtime::Runtime,
        drain: &MicrophoneDrain,
    ) -> MicrophonePublication {
        let (cancel, _) = tokio::sync::watch::channel(false);
        MicrophonePublication {
            state: Arc::new(Mutex::new(PublicationControl::default())),
            revoked: Arc::new(AtomicBool::new(false)),
            cancel,
            drain: drain.clone(),
            task: runtime.spawn(std::future::pending()),
        }
    }

    #[test]
    fn cancellation_before_creation_blocks_new_route_until_stop_acknowledged() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let drain = MicrophoneDrain::default();
        let publication = pending(&runtime, &drain);
        let state = publication.state.clone();
        let revoked = publication.revoked.clone();
        publication.revoke();
        publication.revoke();
        assert_eq!(drain.pending(), 1);
        drop(publication);
        assert_eq!(drain.pending(), 1);
        assert!(revoked.load(Ordering::SeqCst));
        // A late completion retains revocation; it can never unmute the old route.
        assert!(state.lock().unwrap().revoked);
        complete_publication(&state, &drain, true);
        assert_eq!(drain.pending(), 0);
    }

    #[test]
    fn failed_stop_acknowledgment_keeps_routing_closed() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let drain = MicrophoneDrain::default();
        let publication = pending(&runtime, &drain);
        let state = publication.state.clone();
        drop(publication);
        complete_publication(&state, &drain, false);
        assert_eq!(drain.pending(), 1);
    }

    #[test]
    fn completed_publication_does_not_leave_a_phantom_drain() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let drain = MicrophoneDrain::default();
        let publication = pending(&runtime, &drain);
        complete_publication(&publication.state, &drain, true);
        drop(publication);
        assert_eq!(drain.pending(), 0);
    }
}
