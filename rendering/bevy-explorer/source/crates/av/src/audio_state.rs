use bevy::{asset::LoadState, prelude::*};
use bevy_console::ConsoleCommand;
use common::structs::{AudioEmitter, PrimaryUser};
use console::{reply::round, DoAddConsoleCommand};
use dcl_component::transform_and_parent::DclTranslation;
use ipfs::IpfsAssetServer;
use scene_runner::{renderer_context::RendererSceneContext, ContainingScene, SceneEntity};
use serde_json::{json, Value};

#[cfg(any(feature = "ffmpeg", feature = "html"))]
use crate::AVSinks;
use crate::{
    audio_source::{AudioSource, EmitterEnded, EmitterPlayback},
    debug_panel::{source_label, MediaPlayers, MediaRead},
    AVPlayer, AVPlayerConfig, AudioStream, VideoPlayer,
};
#[cfg(any(feature = "ffmpeg", feature = "html"))]
use dcl_component::proto_components::sdk::components::VideoState;

pub(crate) struct AudioStatePlugin;

impl Plugin for AudioStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<AudioStateCommand, _>(audio_state);
    }
}

/// The sound of the scene under the player: its `AudioSource` emitters, audio streams and video
/// players, nearest first, with what the scene asked for (`requested`, `volume.authored`) and what
/// the engine is playing (`playing`, `state`, `volume.live`, `currentTime`).
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/audio_state")]
struct AudioStateCommand {
    /// List only the entries that are sounding; `summary` still counts the whole scene.
    #[arg(long)]
    only_playing: bool,
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn audio_state(
    mut cmd: ConsoleCommand<AudioStateCommand>,
    player: Query<(Entity, &GlobalTransform), With<PrimaryUser>>,
    containing: ContainingScene,
    scenes: Query<&RendererSceneContext>,
    sources: Query<(
        &SceneEntity,
        &AudioSource,
        Option<&AudioEmitter>,
        Option<&EmitterPlayback>,
        Has<EmitterEnded>,
        Option<&GlobalTransform>,
    )>,
    clips: Res<Assets<bevy_kira_audio::AudioSource>>,
    ipfas: IpfsAssetServer,
    streams: MediaPlayers<AudioStream>,
    videos: MediaPlayers<VideoPlayer>,
) {
    let Some(Ok(command)) = cmd.take() else {
        return;
    };
    let Ok((player, player_gt)) = player.single() else {
        cmd.reply_failed("there is no player");
        return;
    };
    let Some(scene) = containing.get_parcel(player) else {
        cmd.reply_failed("No scene world found at the current parcel.");
        return;
    };
    let Ok(ctx) = scenes.get(scene) else {
        cmd.reply_failed("No scene world found at the current parcel.");
        return;
    };
    // the audio backends mute a scene's emitters unless the player stands in it
    let is_current = containing.get(player).contains(&scene);
    let listener = player_gt.translation();
    let distance =
        |gt: Option<&GlobalTransform>| gt.map(|gt| round(gt.translation().distance(listener)));
    let position = |gt: Option<&GlobalTransform>| {
        gt.map(|gt| {
            let p = DclTranslation::from_bevy_translation(gt.translation()).0;
            json!({ "x": round(p[0]), "y": round(p[1]), "z": round(p[2]) })
        })
    };

    let mut entries = Vec::new();
    let mut counts = [0usize; 3];

    for (entity, source, emitter, playback, ended, gt) in &sources {
        if entity.root != scene {
            continue;
        }
        counts[0] += 1;
        let clip = emitter.map(|emitter| &emitter.handle);
        let load = clip.and_then(|clip| ipfas.asset_server().get_load_state(clip.id()));
        let duration = clip
            .and_then(|clip| clips.get(clip.id()))
            .map(|clip| clip.sound.frames.len() as f64 / f64::from(clip.sound.sample_rate));
        let requested = source.playing();
        let state = emitter_state(
            !source.audio_clip_url.is_empty() && emitter.is_some(),
            playback.is_some(),
            ended,
            load.as_ref(),
            requested,
        );
        entries.push(json!({
            "kind": "audio_source",
            "crdtEntityId": entity.id.as_proto_u32().unwrap_or(entity.id.id as u32),
            "source": source_label(&source.audio_clip_url),
            "state": state,
            "playing": playback.is_some(),
            "requested": requested,
            "loop": source.r#loop(),
            "spatial": !source.global(),
            "volume": {
                "authored": round(source.volume.unwrap_or(1.0)),
                "live": playback.map(|p| round(p.gain)),
            },
            "pitch": round(source.pitch.unwrap_or(1.0)),
            "currentTime": playback.map(|p| round(p.position as f32)),
            "duration": duration.map(|d| round(d as f32)),
            "distance": distance(gt),
            "position": position(gt),
        }));
    }

    for read in streams.iter().filter(|read| read.entity.root == scene) {
        counts[1] += 1;
        let spatial = read.player.spatial.unwrap_or(false);
        entries.push(player_entry(
            "audio_stream",
            &read,
            spatial,
            &distance,
            &position,
        ));
    }

    for read in videos.iter().filter(|read| read.entity.root == scene) {
        counts[2] += 1;
        let spatial = read.player.spatial.unwrap_or(false);
        entries.push(player_entry(
            "video_player",
            &read,
            spatial,
            &distance,
            &position,
        ));
    }

    let total = entries.len();
    let playing = entries.iter().filter(|e| e["playing"] == true).count();
    // nearest first; an entry without a position (no transform) last
    entries.sort_by(|a, b| {
        let d = |e: &Value| e["distance"].as_f64().unwrap_or(f64::INFINITY);
        d(a).total_cmp(&d(b))
    });
    if command.only_playing {
        entries.retain(|e| e["playing"] == true);
    }

    let listener = DclTranslation::from_bevy_translation(listener).0;
    cmd.reply_ok(
        json!({
            "scene": { "hash": &*ctx.hash, "title": &ctx.title, "parcel": { "x": ctx.base.x, "y": ctx.base.y } },
            "sceneIsCurrent": is_current,
            "note": (!is_current).then_some(
                "the player is not in this scene, so the engine plays its emitters at volume 0"
            ),
            "listener": { "x": round(listener[0]), "y": round(listener[1]), "z": round(listener[2]) },
            "summary": {
                "total": total,
                "playing": playing,
                "audioSources": counts[0],
                "streams": counts[1],
                "videos": counts[2],
            },
            "onlyPlaying": command.only_playing,
            "returned": entries.len(),
            "entries": entries,
        })
        .to_string(),
    );
}

/// A stream's or a video's entry: what the scene asked of the player and, with a playback
/// backend, what its sinks last reported.
fn player_entry<T: AVPlayer>(
    kind: &str,
    read: &MediaRead<T>,
    spatial: bool,
    distance: &impl Fn(Option<&GlobalTransform>) -> Option<f64>,
    position: &impl Fn(Option<&GlobalTransform>) -> Option<Value>,
) -> Value {
    let config = read.player.config();
    #[allow(unused_mut)]
    let mut entry = json!({
        "kind": kind,
        "crdtEntityId": read.entity.id.as_proto_u32().unwrap_or(read.entity.id.id as u32),
        "source": source_label(read.player.url()),
        "state": "none",
        "playing": false,
        "requested": config.playing(),
        "loop": config.r#loop(),
        "spatial": spatial,
        "volume": { "authored": round(config.volume()), "live": null },
        "scheduled": read.scheduled,
        "distance": distance(read.transform),
        "position": position(read.transform),
    });
    #[cfg(any(feature = "ffmpeg", feature = "html"))]
    add_sink(&mut entry, read.sinks, read.scheduled);
    entry
}

/// An emitter's `state`. A clip asked to play that is loaded but not sounding has `ended` only
/// once the backend saw it play to its end; before that the backend is still starting it.
fn emitter_state(
    has_clip: bool,
    sounding: bool,
    ended: bool,
    load: Option<&LoadState>,
    requested: bool,
) -> &'static str {
    match (sounding, load) {
        _ if !has_clip => "none",
        (true, _) => "playing",
        (false, Some(LoadState::Failed(_))) => "error",
        (false, Some(LoadState::Loaded)) if requested && ended => "ended",
        (false, Some(LoadState::Loaded)) if requested => "loading",
        (false, Some(LoadState::Loaded)) => "ready",
        (false, _) => "loading",
    }
}

/// A stream's or a video's backend state: the last state the player reported to the scene, the
/// volume it was sent, and, for a video, where it is in the media.
#[cfg(any(feature = "ffmpeg", feature = "html"))]
fn add_sink<T: AVPlayer>(entry: &mut Value, sinks: Option<&AVSinks<T>>, scheduled: bool) {
    let Some(sinks) = sinks else {
        return;
    };
    let state = sinks.sent_state;
    entry["state"] = json!(state
        .as_str_name()
        .trim_start_matches("VS_")
        .to_ascii_lowercase());
    entry["playing"] = json!(scheduled && state == VideoState::VsPlaying);
    entry["volume"]["live"] = json!(sinks.sent_volume.map(round));
    if let Some(video) = sinks.video.as_ref() {
        entry["currentTime"] =
            json!((video.current_time >= 0.0).then(|| round(video.current_time as f32)));
        entry["duration"] = json!(video.length.map(|l| round(l as f32)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_emitter_reads_what_the_backend_does_not_what_the_scene_asked() {
        let loaded = Some(&LoadState::Loaded);
        assert_eq!(emitter_state(false, false, false, loaded, true), "none");
        assert_eq!(emitter_state(true, true, false, loaded, true), "playing");
        assert_eq!(emitter_state(true, false, false, loaded, false), "ready");
        // a one-shot that played to its end: the scene still says `playing`
        assert_eq!(emitter_state(true, false, true, loaded, true), "ended");
        assert_eq!(
            emitter_state(true, false, false, Some(&LoadState::Loading), true),
            "loading"
        );
        assert_eq!(emitter_state(true, false, false, None, false), "loading");
    }

    #[test]
    fn a_clip_asked_to_play_that_has_not_started_is_not_ended() {
        let loaded = Some(&LoadState::Loaded);
        // loaded and requested, the backend has not started it (or is retrying the start)
        assert_eq!(emitter_state(true, false, false, loaded, true), "loading");
        // an end seen before the scene stopped asking does not outlive the request
        assert_eq!(emitter_state(true, false, true, loaded, false), "ready");
    }
}
