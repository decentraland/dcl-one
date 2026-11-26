//! Scene animations in the runtime debug panel, and the `/scene_animations` console command.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use serde_json::json;

use super::{Animator, Clips, FinishReports};
use crate::SceneEntity;

/// The reply lists this many clips; the counts cover every model.
const LISTED_CLIPS: usize = 32;

pub(super) struct AnimatorDiagnosticsPlugin;

impl Plugin for AnimatorDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<SceneAnimationsCommand, _>(scene_animations)
            .add_debug_section(DebugSection {
                title: "Scene animations",
                query: "/scene_animations",
                fields: &[
                    ("Animated models", "models"),
                    ("Models with an Animator", "withAnimator"),
                    ("Clips playing", "playing"),
                    ("Clips stopped by their scene", "stopped"),
                    ("Clips at their end", "finished"),
                    ("Finished clips reported to scenes", "finishReports"),
                ],
                actions: &[],
                inputs: &[],
            });
    }
}

/// Count the scene models that animate and list their clips' playback state.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_animations")]
struct SceneAnimationsCommand;

fn scene_animations(
    mut input: ConsoleCommand<SceneAnimationsCommand>,
    models: Query<(&SceneEntity, &AnimationPlayer, &Clips, Has<Animator>)>,
    reports: Res<FinishReports>,
) {
    if input.take().is_none() {
        return;
    }
    let (mut with_animator, mut playing, mut stopped, mut finished) = (0, 0, 0, 0);
    let mut listed = Vec::new();
    for (scene_ent, player, clips, has_animator) in models.iter() {
        if has_animator {
            with_animator += 1;
        }
        for (node, active) in player.playing_animations() {
            let state = if active.is_finished() {
                finished += 1;
                "finished"
            } else if active.weight() == 0.0 && active.speed() == 0.0 {
                stopped += 1;
                "stopped"
            } else {
                playing += 1;
                "playing"
            };
            if listed.len() == LISTED_CLIPS {
                continue;
            }
            // a model without an Animator plays its first clip, which has a node of its own
            let named = clips
                .named
                .iter()
                .find(|(_, (named, _))| named == node)
                .map(|(name, (_, duration))| (name.as_str(), *duration));
            listed.push(json!({
                "entity": scene_ent.id.to_string(),
                "clip": named.map(|(name, _)| name),
                "state": state,
                "weight": active.weight(),
                "speed": active.speed(),
                "seekSeconds": active.seek_time(),
                "durationSeconds": named.map(|(_, duration)| duration),
                "looping": matches!(
                    active.repeat_mode(),
                    bevy::animation::RepeatAnimation::Forever
                ),
            }));
        }
    }
    input.reply_ok(
        json!({
            "models": models.iter().count(),
            "withAnimator": with_animator,
            "playing": playing,
            "stopped": stopped,
            "finished": finished,
            "finishReports": reports.0,
            "clips": listed,
        })
        .to_string(),
    );
}
